use candid::Principal;

use crate::battle::{
    BattleCommandBudget, BattleState, BattleView, battle_view_for_participant,
    build_first_playable_battle_state, submit_battle_action as submit_battle_action_inner,
    sync_battle as sync_battle_inner,
};
use crate::champion::{ChampionView, ChampionViewResult};
use crate::command::{CommandPhase, CommandStatus, CommandStatusView};
use crate::content::get_content_manifest;
use crate::driver::{HeadlessBackend, PlayerView, SessionView};
use crate::fixtures::{ScenarioFixture, first_playable_fixture};
use crate::lifecycle::ParticipantView;
use crate::limits::{MAX_COMMAND_PAYLOAD_JSON_BYTES, MAX_LIST_LIMIT};
use crate::map::{MapChunkPage, ObjectViewPage, Viewport};
use crate::movement::{MoveCoord, MovementPreview};
use crate::strategic::{StrategicBackend, StrategicFixtureBackend};
use crate::town::{BuildPreview, RecruitPreview, RecruitTarget};

use super::codec::{
    changed, duplicate_nonce_error, escape_json, fallback_command_id, lobby_status_view,
    map_update_error, nonce_u64, payload_hash,
};
use super::types::{
    ApiError, ApiEventPage, ApiEventView, ApiMetrics, ApiTownView, BattleActionInput,
    ChangedSubject, CommandResponse, CommandResult, ContentManifestResponse, EventPageInfo,
    GameView, GameViewRequest, LobbyCommandResponse, LobbyCommandResult, MatchHistoryPage,
    PageInfo,
};
use super::view::{
    MAX_CHUNK_LIMIT, MAX_OBJECT_LIMIT, api_event_from_command_event, build_game_view,
    deliver_event_to_audience, map_api_error, opening_viewport_for_slot, participant_audience_key,
};

const API_EVENT_SEQ_START: u64 = 10_000;

#[derive(Clone, Debug)]
pub struct FixtureApiBackend {
    fixture: ScenarioFixture,
    strategic: StrategicFixtureBackend,
    battle_state: Option<BattleState>,
    command_responses: Vec<CommandResponse>,
    lobby_responses: Vec<LobbyCommandResponse>,
    api_events: Vec<ApiEventView>,
    next_api_event_seq: u64,
    server_now_ms: u64,
}

impl Default for FixtureApiBackend {
    fn default() -> Self {
        Self::new(first_playable_fixture())
    }
}

impl FixtureApiBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            strategic: StrategicFixtureBackend::new(fixture.clone()),
            fixture,
            battle_state: None,
            command_responses: Vec::new(),
            lobby_responses: Vec::new(),
            api_events: Vec::new(),
            next_api_event_seq: API_EVENT_SEQ_START,
            server_now_ms: 0,
        }
    }

    #[must_use]
    pub fn fixture(&self) -> &ScenarioFixture {
        &self.fixture
    }

    #[must_use]
    pub fn metrics(&self) -> ApiMetrics {
        ApiMetrics {
            command_response_count: self.command_responses.len() as u32,
            lobby_response_count: self.lobby_responses.len() as u32,
            api_event_count: self.api_events.len() as u32,
            strategic_command_count: self.strategic.command_count(),
            strategic_event_count: self.strategic.event_count(),
            strategic_query_count: self.strategic.query_count(),
        }
    }

    pub fn start_first_playable_session(&mut self) -> SessionView {
        let player_one = self.fixture.principals.player_one;
        let player_two = self.fixture.principals.player_two;
        let nonces = self.fixture.command_nonces.clone();
        let scenario_seed = self.fixture.scenario_seed.clone();
        self.register_player(player_one, "Misery One", &nonces.register_player_one);
        self.register_player(player_two, "Mayhem Two", &nonces.register_player_two);
        let created = self.create_session(player_one, &nonces.create_session, &scenario_seed);
        let session_id = match created.result {
            LobbyCommandResult::Session(session) => session.session_id,
            _ => self.fixture.ids.session_id.clone(),
        };
        self.join_session(player_two, &session_id, &nonces.join_session);
        self.mark_ready(player_one, &session_id, &nonces.mark_ready_player_one);
        self.mark_ready(player_two, &session_id, &nonces.mark_ready_player_two);
        let started = self.start_session(player_one, &session_id, &nonces.start_session);
        match started.result {
            LobbyCommandResult::Session(session) => session,
            _ => self
                .get_session(&session_id)
                .expect("first playable session should be available after start"),
        }
    }

    pub fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        let command_type = "register_player";
        let payload = format!(r#"{{"display_name":"{}"}}"#, escape_json(display_name));
        let payload_hash = payload_hash(command_type, &caller.to_text(), client_nonce, &payload);
        if let Some(response) =
            self.replayed_or_mismatched_lobby(caller, command_type, client_nonce, &payload_hash)
        {
            return response;
        }
        if let Some(error) = payload_limit_error("lobby_command.payload_json", &payload) {
            return self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                error,
            );
        }

        let result = self
            .strategic
            .register_player(caller, display_name, client_nonce);
        match result {
            Ok(player) => {
                let event = self.append_api_event(
                    None,
                    command_type,
                    "player_registered",
                    Some("player"),
                    Some(&player.player_id),
                    &format!(r#"{{"player_id":"{}"}}"#, player.player_id),
                    "public",
                );
                self.success_lobby_response(
                    caller,
                    command_type,
                    client_nonce,
                    payload_hash,
                    vec![event],
                    vec![changed("player", &player.player_id, "upsert")],
                    LobbyCommandResult::Player(player),
                )
            }
            Err(error) => self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                map_update_error("register_player_failed", error),
            ),
        }
    }

    pub fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> LobbyCommandResponse {
        let command_type = "create_session";
        let payload = format!(r#"{{"scenario_seed":"{}"}}"#, escape_json(scenario_seed));
        let payload_hash = payload_hash(command_type, &caller.to_text(), client_nonce, &payload);
        if let Some(response) =
            self.replayed_or_mismatched_lobby(caller, command_type, client_nonce, &payload_hash)
        {
            return response;
        }
        if let Some(error) = payload_limit_error("lobby_command.payload_json", &payload) {
            return self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                error,
            );
        }

        match self
            .strategic
            .create_session(caller, client_nonce, scenario_seed)
        {
            Ok(session) => {
                let event = self.append_api_event(
                    Some(&session.session_id),
                    command_type,
                    "session_created",
                    Some("session"),
                    Some(&session.session_id),
                    &format!(r#"{{"session_id":"{}"}}"#, session.session_id),
                    "public",
                );
                self.success_lobby_response(
                    caller,
                    command_type,
                    client_nonce,
                    payload_hash,
                    vec![event],
                    vec![changed("session", &session.session_id, "upsert")],
                    LobbyCommandResult::Session(session),
                )
            }
            Err(error) => self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                map_update_error("create_session_failed", error),
            ),
        }
    }

    pub fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.apply_session_lobby_command(caller, session_id, client_nonce, "join_session", |this| {
            this.strategic
                .join_session(caller, session_id, client_nonce)
        })
    }

    pub fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.apply_session_lobby_command(caller, session_id, client_nonce, "mark_ready", |this| {
            this.strategic.mark_ready(caller, session_id, client_nonce)
        })
    }

    pub fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.apply_session_lobby_command(
            caller,
            session_id,
            client_nonce,
            "start_session",
            |this| {
                this.strategic
                    .start_session(caller, session_id, client_nonce)
            },
        )
    }

    pub fn submit_move_intent(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        let command_type = "submit_move_intent";
        let payload = format!(
            r#"{{"champion_id":"{}","path_len":{},"now_ms":{now_ms}}}"#,
            escape_json(champion_id),
            path.len()
        );
        self.apply_strategic_command(
            caller,
            session_id,
            command_type,
            client_nonce,
            payload,
            |this| {
                this.server_now_ms = now_ms;
                let receipt = this
                    .strategic
                    .submit_move_intent_public(
                        caller,
                        session_id,
                        champion_id,
                        path,
                        nonce_u64(command_type, client_nonce),
                        now_ms,
                    )
                    .map_err(|error| map_update_error("submit_move_intent_failed", error))?;
                Ok((
                    receipt.command_id.clone(),
                    CommandResult::StrategicReceipt(receipt),
                    vec![changed("champion", champion_id, "update")],
                ))
            },
        )
    }

    pub fn sync_session_turn(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        let command_type = "sync_session_turn";
        let payload = format!(r#"{{"now_ms":{now_ms}}}"#);
        self.apply_strategic_command(
            caller,
            session_id,
            command_type,
            client_nonce,
            payload,
            |this| {
                this.set_now(now_ms);
                let participant_id = this.participant_id_for_caller(caller)?.to_string();
                let receipt = this
                    .strategic
                    .sync_session_turn_public(caller, session_id, now_ms)
                    .map_err(|error| map_update_error("sync_session_turn_failed", error))?;
                this.strategic
                    .apply_movement_object_interactions_public(caller, session_id, now_ms)
                    .map_err(|error| {
                        map_update_error("movement_object_interactions_failed", error)
                    })?;
                this.strategic
                    .apply_neutral_encounters_public(caller, session_id)
                    .map_err(|error| map_update_error("neutral_encounters_failed", error))?;
                this.strategic
                    .materialize_income_public(
                        caller,
                        session_id,
                        receipt.current_turn,
                        nonce_u64("sync_session_income", client_nonce),
                    )
                    .map_err(|error| map_update_error("sync_income_failed", error))?;
                Ok((
                    receipt.command_id.clone(),
                    CommandResult::StrategicReceipt(receipt),
                    vec![
                        changed("session", session_id, "update"),
                        changed("participant", &participant_id, "resources"),
                    ],
                ))
            },
        )
    }

    pub fn submit_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        let command_type = "submit_build_town_structure";
        let payload = format!(
            r#"{{"town_id":"{}","building_slug":"{}","turn_number":{turn_number}}}"#,
            escape_json(town_id),
            escape_json(building_slug)
        );
        self.apply_strategic_command(
            caller,
            session_id,
            command_type,
            client_nonce,
            payload,
            |this| {
                let receipt = this
                    .strategic
                    .build_town_structure_public(
                        caller,
                        session_id,
                        town_id,
                        building_slug,
                        turn_number,
                        nonce_u64(command_type, client_nonce),
                    )
                    .map_err(|error| map_update_error("build_town_structure_failed", error))?;
                Ok((
                    receipt.command_id.clone(),
                    CommandResult::StrategicReceipt(receipt),
                    vec![changed("town", town_id, "update")],
                ))
            },
        )
    }

    pub fn submit_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        let command_type = "submit_recruit_units";
        let payload = format!(
            r#"{{"town_id":"{}","unit_slug":"{}","quantity":{quantity},"turn_number":{turn_number}}}"#,
            escape_json(town_id),
            escape_json(unit_slug)
        );
        self.apply_strategic_command(
            caller,
            session_id,
            command_type,
            client_nonce,
            payload,
            |this| {
                let receipt = this
                    .strategic
                    .recruit_units_public(
                        caller,
                        session_id,
                        town_id,
                        unit_slug,
                        quantity,
                        target,
                        turn_number,
                        nonce_u64(command_type, client_nonce),
                    )
                    .map_err(|error| map_update_error("recruit_units_failed", error))?;
                Ok((
                    receipt.command_id.clone(),
                    CommandResult::StrategicReceipt(receipt),
                    vec![changed("town", town_id, "update")],
                ))
            },
        )
    }

    pub fn sync_battle(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        let command_type = "sync_battle";
        let payload = format!(
            r#"{{"battle_id":"{}","now_ms":{now_ms}}}"#,
            escape_json(battle_id)
        );
        self.apply_strategic_command(
            caller,
            &self.fixture.ids.session_id.clone(),
            command_type,
            client_nonce,
            payload,
            |this| {
                this.server_now_ms = now_ms;
                let state = this.ensure_battle_state()?;
                let outcome =
                    sync_battle_inner(state, battle_id, now_ms, BattleCommandBudget::default())
                        .map_err(|error| map_api_error("battle_sync_failed", error))?;
                let command_id = format!("command:api:battle-sync:{battle_id}:{client_nonce}");
                Ok((
                    command_id,
                    CommandResult::BattleSync(outcome),
                    vec![changed("battle", battle_id, "update")],
                ))
            },
        )
    }

    pub fn submit_battle_action(
        &mut self,
        caller: Principal,
        input: BattleActionInput,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        let command_type = "submit_battle_action";
        let payload = format!(
            r#"{{"battle_id":"{}","stack":"{}","action":"{}","now_ms":{now_ms}}}"#,
            escape_json(&input.battle_id),
            escape_json(&input.battle_stack_id),
            escape_json(&input.action)
        );
        self.apply_strategic_command(
            caller,
            &self.fixture.ids.session_id.clone(),
            command_type,
            client_nonce,
            payload,
            |this| {
                this.server_now_ms = now_ms;
                let participant_id = this.participant_id_for_caller(caller)?.to_string();
                let state = this.ensure_battle_state()?;
                let receipt = submit_battle_action_inner(
                    state,
                    &input.battle_id,
                    &participant_id,
                    &input.battle_stack_id,
                    &input.action,
                    input.target_stack_id.as_deref(),
                    input.destination,
                    client_nonce,
                    now_ms,
                )
                .map_err(|error| map_api_error("battle_action_failed", error))?;
                Ok((
                    receipt.command_id.clone(),
                    CommandResult::BattleAction(receipt),
                    vec![changed("battle", &input.battle_id, "update")],
                ))
            },
        )
    }

    pub fn get_my_player(&self, caller: Principal) -> Result<PlayerView, ApiError> {
        self.strategic
            .get_my_player_public(caller)
            .map_err(|error| map_api_error("get_my_player_failed", error))
    }

    pub fn get_session(&self, session_id: &str) -> Result<SessionView, ApiError> {
        self.strategic
            .get_session_public(session_id)
            .map_err(|error| map_api_error("get_session_failed", error))
    }

    pub fn get_my_participant(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ParticipantView, ApiError> {
        self.strategic
            .get_my_participant_public(caller, session_id)
            .map_err(|error| map_api_error("get_my_participant_failed", error))
    }

    pub fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        request: GameViewRequest,
    ) -> Result<GameView, ApiError> {
        let battle = if request.include_battle {
            self.current_battle_view(caller).transpose()?
        } else {
            None
        };
        build_game_view(
            &mut self.strategic,
            caller,
            session_id,
            &request,
            self.server_now_ms,
            battle,
            &self.api_events,
        )
    }

    pub fn get_default_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<GameView, ApiError> {
        let participant = self.get_my_participant(caller, session_id)?;
        self.get_game_view(
            caller,
            session_id,
            GameViewRequest::opening_for_slot(participant.slot_index),
        )
    }

    pub fn get_visible_map_chunks(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<MapChunkPage, ApiError> {
        validate_public_limit(
            "chunk_limit",
            limit,
            MAX_CHUNK_LIMIT,
            "viewport_chunk_limit_exceeded",
        )?;
        self.strategic
            .visible_map_chunks_public(caller, session_id, viewport, cursor, limit)
            .map_err(|error| map_api_error("visible_chunks_failed", error))
    }

    pub fn get_visible_objects(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<ObjectViewPage, ApiError> {
        validate_public_limit(
            "object_limit",
            limit,
            MAX_OBJECT_LIMIT,
            "list_limit_exceeded",
        )?;
        self.strategic
            .visible_objects_public(caller, session_id, viewport, cursor, limit)
            .map_err(|error| map_api_error("visible_objects_failed", error))
    }

    pub fn get_my_champions(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<Vec<ChampionView>, ApiError> {
        self.strategic
            .my_champions_public(caller, session_id)
            .map_err(|error| map_api_error("champions_failed", error))
    }

    pub fn get_champion_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
    ) -> Result<ChampionView, ApiError> {
        match self
            .strategic
            .champion_view_public(caller, session_id, champion_id)
            .map_err(|error| map_api_error("champion_view_failed", error))?
        {
            ChampionViewResult::Visible(view) => Ok(view),
            ChampionViewResult::Hidden { visibility, .. } => Err(ApiError::new(
                "not_visible",
                format!("champion is {visibility}"),
                false,
            )),
        }
    }

    pub fn get_town_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
    ) -> Result<ApiTownView, ApiError> {
        let participant_id = self.participant_id_for_caller(caller)?.to_string();
        let participant = self.get_my_participant(caller, session_id)?;
        let viewport = opening_viewport_for_slot(participant.slot_index);
        let visible = self
            .get_visible_objects(caller, session_id, &viewport, None, 128)?
            .objects
            .into_iter()
            .any(|object| object.subject_kind == "town" && object.subject_id_text == town_id);
        let state = self
            .strategic
            .export_aftermath_state()
            .map_err(|error| map_api_error("town_view_failed", error))?;
        let town = state
            .town
            .towns
            .iter()
            .find(|town| town.town_id == town_id)
            .ok_or_else(|| ApiError::new("not_found", "town not found", false))?;
        if town.owner_participant_id != participant_id && !visible {
            return Err(ApiError::new("not_visible", "town is not visible", false));
        }
        Ok(ApiTownView {
            town: town.clone(),
            buildings: state
                .town
                .buildings
                .iter()
                .filter(|building| building.town_id == town_id)
                .cloned()
                .collect(),
            recruit_pools: state
                .town
                .recruit_pools
                .iter()
                .filter(|pool| pool.town_id == town_id)
                .cloned()
                .collect(),
            garrison_stacks: state
                .town
                .garrison_stacks
                .iter()
                .filter(|stack| stack.owner_id == town_id)
                .cloned()
                .collect(),
        })
    }

    pub fn get_battle_state(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<BattleView, ApiError> {
        self.server_now_ms = now_ms;
        let participant_id = self.participant_id_for_caller(caller)?.to_string();
        let state = self.ensure_battle_state()?;
        battle_view_for_participant(state, battle_id, &participant_id, now_ms)
            .map_err(|error| map_api_error("battle_view_failed", error))
    }

    pub fn get_content_manifest(
        &self,
        ruleset_slug: &str,
        version: u32,
    ) -> Result<ContentManifestResponse, ApiError> {
        let manifest = get_content_manifest(ruleset_slug, version).ok_or_else(|| {
            ApiError::new(
                "content_manifest_not_found",
                "content manifest was not found",
                false,
            )
        })?;
        Ok(ContentManifestResponse { manifest })
    }

    pub fn get_command_status(
        &self,
        command_id_or_client_nonce: &str,
    ) -> Option<CommandStatusView> {
        self.command_responses
            .iter()
            .find(|response| {
                response.command_id == command_id_or_client_nonce
                    || response.client_nonce == command_id_or_client_nonce
            })
            .map(CommandResponse::status_view)
            .or_else(|| {
                self.lobby_responses
                    .iter()
                    .find(|response| {
                        response.command_id == command_id_or_client_nonce
                            || response.client_nonce == command_id_or_client_nonce
                    })
                    .map(lobby_status_view)
            })
    }

    pub fn get_events_after(
        &self,
        session_id: &str,
        audience_key: &str,
        after_seq: u64,
        limit: u32,
    ) -> ApiEventPage {
        let limit = limit.clamp(1, MAX_LIST_LIMIT);
        let api_start_index = self
            .api_events
            .partition_point(|event| event.event_seq <= after_seq);
        let mut events = self
            .lifecycle_events_for_audience(session_id, audience_key, after_seq, limit)
            .into_iter()
            .chain(
                self.api_events[api_start_index..]
                    .iter()
                    .filter(|event| event.session_id == session_id && event.event_seq > after_seq)
                    .take(limit.saturating_add(1) as usize)
                    .map(|event| deliver_event_to_audience(event, audience_key)),
            )
            .collect::<Vec<_>>();
        events.sort_by_key(|event| event.event_seq);
        let has_more = events.len() > limit as usize;
        events.truncate(limit as usize);
        ApiEventPage {
            page_info: EventPageInfo {
                next_event_seq: has_more
                    .then(|| events.last().map_or(after_seq, |event| event.event_seq)),
                has_more,
                limit,
            },
            events,
        }
    }

    pub fn get_match_history(
        &self,
        caller: Principal,
        cursor: u32,
        limit: u32,
    ) -> Result<MatchHistoryPage, ApiError> {
        validate_public_limit("limit", limit, MAX_LIST_LIMIT, "list_limit_exceeded")?;
        let entries = self
            .strategic
            .get_match_history_public(caller, cursor as usize, limit as usize)
            .map_err(|error| map_api_error("match_history_failed", error))?;
        let has_more = entries.len() == limit as usize;
        Ok(MatchHistoryPage {
            entries,
            page_info: PageInfo {
                next_cursor: has_more.then_some(cursor.saturating_add(limit)),
                has_more,
                limit,
            },
        })
    }

    pub fn preview_move(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        now_ms: u64,
    ) -> Result<MovementPreview, ApiError> {
        self.strategic
            .preview_move_public(caller, session_id, champion_id, path, now_ms)
            .map_err(|error| map_api_error("preview_move_failed", error))
    }

    pub fn preview_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
    ) -> Result<BuildPreview, ApiError> {
        self.strategic
            .preview_build_public(caller, session_id, town_id, building_slug, turn_number)
            .map_err(|error| map_api_error("preview_build_failed", error))
    }

    pub fn preview_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        turn_number: u32,
    ) -> Result<RecruitPreview, ApiError> {
        self.strategic
            .preview_recruit_public(
                caller,
                session_id,
                town_id,
                unit_slug,
                quantity,
                target,
                turn_number,
            )
            .map_err(|error| map_api_error("preview_recruit_failed", error))
    }

    #[must_use]
    pub fn first_fixture_battle_id(&self) -> String {
        format!(
            "battle:{}:8:champion:west:neutral:west-mine",
            self.fixture.ids.session_id
        )
    }

    fn apply_session_lobby_command<F>(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
        command_type: &str,
        apply: F,
    ) -> LobbyCommandResponse
    where
        F: FnOnce(&mut Self) -> Result<SessionView, crate::strategic::StrategicError>,
    {
        let payload = format!(r#"{{"session_id":"{}"}}"#, escape_json(session_id));
        let payload_hash = payload_hash(command_type, &caller.to_text(), client_nonce, &payload);
        if let Some(response) =
            self.replayed_or_mismatched_lobby(caller, command_type, client_nonce, &payload_hash)
        {
            return response;
        }
        if let Some(error) = payload_limit_error("lobby_command.payload_json", &payload) {
            return self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                error,
            );
        }

        match apply(self) {
            Ok(session) => {
                let event_type = match command_type {
                    "start_session" => "session_started_api",
                    "mark_ready" => "participant_ready",
                    "join_session" => "participant_joined",
                    _ => command_type,
                };
                let event = self.append_api_event(
                    Some(&session.session_id),
                    command_type,
                    event_type,
                    Some("session"),
                    Some(&session.session_id),
                    &format!(r#"{{"session_id":"{}"}}"#, session.session_id),
                    "public",
                );
                self.success_lobby_response(
                    caller,
                    command_type,
                    client_nonce,
                    payload_hash,
                    vec![event],
                    vec![changed("session", &session.session_id, "update")],
                    LobbyCommandResult::Session(session),
                )
            }
            Err(error) => self.failed_lobby_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                map_update_error(command_type, error),
            ),
        }
    }

    fn apply_strategic_command<F>(
        &mut self,
        caller: Principal,
        session_id: &str,
        command_type: &str,
        client_nonce: &str,
        payload: String,
        apply: F,
    ) -> CommandResponse
    where
        F: FnOnce(&mut Self) -> Result<(String, CommandResult, Vec<ChangedSubject>), ApiError>,
    {
        let actor_key = self
            .participant_id_for_caller(caller)
            .map_or_else(|error| error.code, ToString::to_string);
        let payload_hash = payload_hash(command_type, &actor_key, client_nonce, &payload);
        if let Some(response) =
            self.replayed_or_mismatched_command(caller, command_type, client_nonce, &payload_hash)
        {
            return response;
        }
        if let Some(error) = payload_limit_error("command.payload_json", &payload) {
            return self.failed_command_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                fallback_command_id(session_id, command_type, client_nonce),
                error,
            );
        }

        match apply(self) {
            Ok((command_id, result, changed_subjects)) => {
                let event = self.append_api_event(
                    Some(session_id),
                    command_type,
                    command_type,
                    changed_subjects
                        .first()
                        .map(|subject| subject.subject_kind.as_str()),
                    changed_subjects
                        .first()
                        .map(|subject| subject.subject_id_text.as_str()),
                    &payload,
                    &participant_audience_key(&actor_key),
                );
                self.success_command_response(
                    caller,
                    command_type,
                    client_nonce,
                    payload_hash,
                    command_id,
                    vec![event],
                    changed_subjects,
                    result,
                )
            }
            Err(error) => self.failed_command_response(
                caller,
                command_type,
                client_nonce,
                payload_hash,
                fallback_command_id(session_id, command_type, client_nonce),
                error,
            ),
        }
    }

    fn replayed_or_mismatched_lobby(
        &self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: &str,
    ) -> Option<LobbyCommandResponse> {
        self.lobby_responses
            .iter()
            .find(|response| {
                response.actor_principal == caller && response.client_nonce == client_nonce
            })
            .map(|response| {
                if response.payload_hash == payload_hash {
                    response.clone()
                } else {
                    self.failed_lobby_response_unstored(
                        caller,
                        command_type,
                        client_nonce,
                        payload_hash.to_string(),
                        duplicate_nonce_error(client_nonce),
                    )
                }
            })
    }

    fn replayed_or_mismatched_command(
        &self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: &str,
    ) -> Option<CommandResponse> {
        self.command_responses
            .iter()
            .find(|response| {
                response.actor_principal == caller && response.client_nonce == client_nonce
            })
            .map(|response| {
                if response.payload_hash == payload_hash {
                    response.clone()
                } else {
                    self.failed_command_response_unstored(
                        caller,
                        command_type,
                        client_nonce,
                        payload_hash.to_string(),
                        fallback_command_id(
                            &self.fixture.ids.session_id,
                            command_type,
                            client_nonce,
                        ),
                        duplicate_nonce_error(client_nonce),
                    )
                }
            })
    }

    fn success_lobby_response(
        &mut self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        events: Vec<ApiEventView>,
        changed_subjects: Vec<ChangedSubject>,
        result: LobbyCommandResult,
    ) -> LobbyCommandResponse {
        let response = LobbyCommandResponse {
            command_id: fallback_command_id("lobby", command_type, client_nonce),
            command_type: command_type.to_string(),
            actor_principal: caller,
            client_nonce: client_nonce.to_string(),
            payload_hash,
            status: CommandStatus::Applied,
            phase: CommandPhase::Complete,
            retryable: false,
            effective_turn: self.strategic.current_turn(),
            durable_turn: self.strategic.current_turn(),
            events,
            changed_subjects,
            result,
            error: None,
        };
        self.lobby_responses.push(response.clone());
        response
    }

    fn failed_lobby_response(
        &mut self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        error: ApiError,
    ) -> LobbyCommandResponse {
        let response = self.failed_lobby_response_unstored(
            caller,
            command_type,
            client_nonce,
            payload_hash,
            error,
        );
        self.lobby_responses.push(response.clone());
        response
    }

    fn failed_lobby_response_unstored(
        &self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        error: ApiError,
    ) -> LobbyCommandResponse {
        LobbyCommandResponse {
            command_id: fallback_command_id("lobby", command_type, client_nonce),
            command_type: command_type.to_string(),
            actor_principal: caller,
            client_nonce: client_nonce.to_string(),
            payload_hash,
            status: CommandStatus::Failed,
            phase: CommandPhase::Failed,
            retryable: error.retryable,
            effective_turn: self.strategic.current_turn(),
            durable_turn: self.strategic.current_turn(),
            events: Vec::new(),
            changed_subjects: Vec::new(),
            result: LobbyCommandResult::None,
            error: Some(error),
        }
    }

    fn success_command_response(
        &mut self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        command_id: String,
        events: Vec<ApiEventView>,
        changed_subjects: Vec<ChangedSubject>,
        result: CommandResult,
    ) -> CommandResponse {
        let response = CommandResponse {
            command_id,
            command_type: command_type.to_string(),
            actor_principal: caller,
            actor_participant_id: self
                .participant_id_for_caller(caller)
                .ok()
                .map(ToString::to_string),
            client_nonce: client_nonce.to_string(),
            payload_hash,
            status: CommandStatus::Applied,
            phase: CommandPhase::Complete,
            retryable: false,
            effective_turn: self.strategic.current_turn(),
            durable_turn: self.strategic.current_turn(),
            events,
            changed_subjects,
            result,
            error: None,
        };
        self.command_responses.push(response.clone());
        response
    }

    fn failed_command_response(
        &mut self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        command_id: String,
        error: ApiError,
    ) -> CommandResponse {
        let response = self.failed_command_response_unstored(
            caller,
            command_type,
            client_nonce,
            payload_hash,
            command_id,
            error,
        );
        self.command_responses.push(response.clone());
        response
    }

    fn failed_command_response_unstored(
        &self,
        caller: Principal,
        command_type: &str,
        client_nonce: &str,
        payload_hash: String,
        command_id: String,
        error: ApiError,
    ) -> CommandResponse {
        CommandResponse {
            command_id,
            command_type: command_type.to_string(),
            actor_principal: caller,
            actor_participant_id: self
                .participant_id_for_caller(caller)
                .ok()
                .map(ToString::to_string),
            client_nonce: client_nonce.to_string(),
            payload_hash,
            status: CommandStatus::Failed,
            phase: CommandPhase::Failed,
            retryable: error.retryable,
            effective_turn: self.strategic.current_turn(),
            durable_turn: self.strategic.current_turn(),
            events: Vec::new(),
            changed_subjects: Vec::new(),
            result: CommandResult::None,
            error: Some(error),
        }
    }

    fn append_api_event(
        &mut self,
        session_id: Option<&str>,
        command_id: &str,
        event_type: &str,
        subject_kind: Option<&str>,
        subject_id_text: Option<&str>,
        payload: &str,
        audience_key: &str,
    ) -> ApiEventView {
        let event = ApiEventView {
            session_id: session_id
                .unwrap_or(&self.fixture.ids.session_id)
                .to_string(),
            event_seq: self.next_api_event_seq,
            event_key: format!(
                "api:{}:{}:{}:{}",
                command_id,
                event_type,
                subject_id_text.unwrap_or("none"),
                self.next_api_event_seq
            ),
            audience_key: audience_key.to_string(),
            turn_number: self.strategic.current_turn(),
            event_type: event_type.to_string(),
            subject_kind: subject_kind.map(str::to_string),
            subject_id_text: subject_id_text.map(str::to_string),
            payload: Some(payload.to_string()),
            redacted: false,
        };
        self.next_api_event_seq = self.next_api_event_seq.saturating_add(1);
        self.api_events.push(event.clone());
        let _ = command_id;
        event
    }

    fn lifecycle_events_for_audience(
        &self,
        session_id: &str,
        audience_key: &str,
        after_seq: u64,
        limit: u32,
    ) -> Vec<ApiEventView> {
        let caller =
            if audience_key == participant_audience_key(&self.fixture.ids.participant_two_id) {
                self.fixture.principals.player_two
            } else {
                self.fixture.principals.player_one
            };
        self.strategic
            .get_events_public(caller, session_id, after_seq, limit as usize)
            .map(|page| {
                page.events
                    .iter()
                    .map(api_event_from_command_event)
                    .map(|event| deliver_event_to_audience(&event, audience_key))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn current_battle_view(&mut self, caller: Principal) -> Option<Result<BattleView, ApiError>> {
        let battle_id = self
            .battle_state
            .as_ref()
            .and_then(|state| state.battles.first())
            .map(|battle| battle.battle_id.clone())?;
        Some(self.get_battle_state(caller, &battle_id, self.server_now_ms))
    }

    fn ensure_battle_state(&mut self) -> Result<&mut BattleState, ApiError> {
        if self.battle_state.is_none() {
            self.battle_state = Some(
                build_first_playable_battle_state()
                    .map_err(|error| map_api_error("battle_fixture_failed", error))?,
            );
        }
        self.battle_state.as_mut().ok_or_else(|| {
            ApiError::new(
                "battle_fixture_failed",
                "battle state was not initialized",
                true,
            )
        })
    }

    fn participant_id_for_caller(&self, caller: Principal) -> Result<&str, ApiError> {
        if caller == self.fixture.principals.player_one {
            Ok(&self.fixture.ids.participant_one_id)
        } else if caller == self.fixture.principals.player_two {
            Ok(&self.fixture.ids.participant_two_id)
        } else {
            Err(ApiError::new("unknown_caller", "unknown caller", false))
        }
    }

    fn set_now(&mut self, now_ms: u64) {
        self.server_now_ms = now_ms;
    }
}

fn payload_limit_error(field: &str, payload: &str) -> Option<ApiError> {
    let actual_bytes = payload.len();
    (actual_bytes > MAX_COMMAND_PAYLOAD_JSON_BYTES).then(|| {
        ApiError::new(
            "payload_too_large",
            format!("{field} exceeds the v1 public command payload limit"),
            false,
        )
        .with_details(format!(
            r#"{{"actual_bytes":{actual_bytes},"max_bytes":{MAX_COMMAND_PAYLOAD_JSON_BYTES}}}"#
        ))
    })
}

fn validate_public_limit(name: &str, limit: u32, max: u32, code: &str) -> Result<(), ApiError> {
    if limit == 0 {
        return Err(ApiError::new(
            "limit_must_be_positive",
            format!("{name} must be at least 1"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    if limit > max {
        return Err(ApiError::new(
            code,
            format!("{name} exceeds the v1 public query limit"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    Ok(())
}
