use candid::Principal;
use domm_game::{
    ApiError, ApiEventPage, BattleActionInput, CommandResponse, FIRST_PLAYABLE_RULESET_SLUG,
    FIRST_PLAYABLE_RULESET_VERSION, GameView, GameViewRequest, LobbyCommandResponse,
    LobbyCommandResult, MoveCoord, RecruitTarget, ScenarioFixture, TURN_DURATION_MS,
};

use crate::render::render_opening_viewport;
use crate::types::{ClientOpeningViewport, ProbeError, RenderedViewport};

use super::service::WebClientBackend;
use super::state::WebClientState;
use super::view_model::WebClientViewModel;

pub struct PlayableWebClient<B> {
    backend: B,
    fixture: ScenarioFixture,
    caller: Principal,
    state: WebClientState,
    nonces: ClientNonceBook,
}

impl<B> PlayableWebClient<B> {
    #[must_use]
    pub fn new(backend: B, fixture: ScenarioFixture, caller: Principal) -> Self {
        Self {
            backend,
            fixture,
            caller,
            state: WebClientState::new(),
            nonces: ClientNonceBook::new("web"),
        }
    }

    #[must_use]
    pub fn state(&self) -> &WebClientState {
        &self.state
    }

    #[must_use]
    pub fn view_model(&self) -> WebClientViewModel {
        WebClientViewModel::from_state(&self.state)
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: WebClientBackend> PlayableWebClient<B> {
    pub fn play_first_playable_walkthrough(&mut self) -> Result<(), ProbeError> {
        self.start_match()?;
        self.submit_pickup_move_with_retry()?;
        self.sync_turn_with_retry(TURN_DURATION_MS, "pickup")?;
        self.refresh()?;
        self.state.mark_complete("pickup");

        self.sync_until_turn(3)?;
        self.build_training_yard()?;
        self.sync_until_turn(8)?;
        self.recruit_levies()?;
        self.trigger_neutral_battle()?;
        self.open_battle_panel()?;
        self.submit_defend_and_sync_battle()?;
        self.finish_match_result()?;
        Ok(())
    }

    pub fn start_match(&mut self) -> Result<(), ProbeError> {
        let manifest = self
            .backend
            .get_content_manifest(FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION)?;
        self.state.content_manifest_hash = Some(manifest.manifest.computed_content_hash());

        let nonces = self.fixture.command_nonces.clone();
        let player_one = self.fixture.principals.player_one;
        let player_two = self.fixture.principals.player_two;
        let scenario_seed = self.fixture.scenario_seed.clone();

        let player_one_response =
            self.backend
                .register_player(player_one, "Misery One", &nonces.register_player_one);
        self.record_lobby(&player_one_response, false)?;
        let player_two_response =
            self.backend
                .register_player(player_two, "Mayhem Two", &nonces.register_player_two);
        self.record_lobby(&player_two_response, false)?;

        let created =
            self.backend
                .create_session(player_one, &nonces.create_session, &scenario_seed);
        self.record_lobby(&created, false)?;
        let session_id =
            session_id_from_lobby(&created).unwrap_or_else(|| self.fixture.ids.session_id.clone());
        self.state.session_id = Some(session_id.clone());

        let joined = self
            .backend
            .join_session(player_two, &session_id, &nonces.join_session);
        self.record_lobby(&joined, false)?;
        let ready_one =
            self.backend
                .mark_ready(player_one, &session_id, &nonces.mark_ready_player_one);
        self.record_lobby(&ready_one, false)?;
        let ready_two =
            self.backend
                .mark_ready(player_two, &session_id, &nonces.mark_ready_player_two);
        self.record_lobby(&ready_two, false)?;
        let started = self
            .backend
            .start_session(player_one, &session_id, &nonces.start_session);
        self.record_lobby(&started, false)?;

        self.state.mark_complete("lobby");
        self.refresh()?;
        self.state.mark_complete("map");
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<(), ProbeError> {
        let session_id = self.session_id()?;
        let mut game_view = self.backend.default_game_view(self.caller, &session_id)?;
        self.collect_remaining_pages(&session_id, &mut game_view)?;
        let rendered = render_game_view(&game_view)?;
        self.state.apply_game_view(game_view, rendered);
        let history = self.backend.get_match_history(self.caller, 0, 20)?;
        self.state.apply_match_history(&history.entries);
        Ok(())
    }

    fn submit_pickup_move_with_retry(&mut self) -> Result<(), ProbeError> {
        let nonce = self.nonces.next("move-pickup");
        let path = vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)];
        let first = self.submit_move("champion:west", path.clone(), &nonce, 1_000, false)?;
        let retry = self.submit_move("champion:west", path, &nonce, 1_000, true)?;
        self.record_retry(&first, &retry)?;
        Ok(())
    }

    fn trigger_neutral_battle(&mut self) -> Result<(), ProbeError> {
        let nonce = self.nonces.next("move-neutral");
        let path = vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ];
        self.submit_move(
            "champion:west",
            path,
            &nonce,
            TURN_DURATION_MS.saturating_mul(7).saturating_add(1_000),
            false,
        )?;
        let mut triggered = false;
        let mut observed = Vec::new();
        for attempt in 0_u64..24 {
            let response = self.sync_turn_with_retry(
                TURN_DURATION_MS
                    .saturating_mul(8)
                    .saturating_add(attempt.saturating_mul(1_000)),
                &format!("battle-trigger-{attempt}"),
            )?;
            let events = response
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>()
                .join(",");
            let error = response
                .error
                .as_ref()
                .map(|error| error.code.as_str())
                .unwrap_or("-");
            observed.push(format!(
                "#{attempt}:status={:?}:turn={}:events=[{}]:error={}",
                response.status, response.effective_turn, events, error
            ));
            if self.backend.has_first_fixture_battle_id()
                || response
                    .events
                    .iter()
                    .any(|event| event.event_type == "neutral_encounter_pending")
            {
                triggered = true;
                break;
            }
        }
        if !triggered && !self.backend.has_first_fixture_battle_id() {
            self.refresh()?;
            let game_view = self.state.game_view.as_ref();
            let champion = game_view.and_then(|view| view.champions.first());
            if let Some(champion) = champion {
                let followup_path = path_to_west_mine_neutral(champion.x, champion.y);
                observed.push(format!(
                    "followup_from=({},{}):path_len={}",
                    champion.x,
                    champion.y,
                    followup_path.len()
                ));
                if !followup_path.is_empty() {
                    let followup_nonce = self.nonces.next("move-neutral-followup");
                    self.submit_move(
                        "champion:west",
                        followup_path,
                        &followup_nonce,
                        TURN_DURATION_MS
                            .saturating_mul(u64::from(self.current_turn()?))
                            .saturating_add(1_000),
                        false,
                    )?;
                    for attempt in 0_u64..24 {
                        let response = self.sync_turn_with_retry(
                            TURN_DURATION_MS
                                .saturating_mul(u64::from(self.current_turn()?))
                                .saturating_add(attempt.saturating_mul(1_000)),
                            &format!("battle-trigger-followup-{attempt}"),
                        )?;
                        let events = response
                            .events
                            .iter()
                            .map(|event| event.event_type.as_str())
                            .collect::<Vec<_>>()
                            .join(",");
                        let error = response
                            .error
                            .as_ref()
                            .map(|error| error.code.as_str())
                            .unwrap_or("-");
                        observed.push(format!(
                            "#followup-{attempt}:status={:?}:turn={}:events=[{}]:error={}",
                            response.status, response.effective_turn, events, error
                        ));
                        if self.backend.has_first_fixture_battle_id()
                            || response
                                .events
                                .iter()
                                .any(|event| event.event_type == "neutral_encounter_pending")
                        {
                            triggered = true;
                            break;
                        }
                    }
                }
            } else if let Some(view) = game_view {
                let mine_objects = view
                    .objects
                    .iter()
                    .filter(|object| object.x == 12 && object.y == 22)
                    .map(|object| {
                        format!(
                            "{}:{}:{}",
                            object.subject_kind, object.subject_id_text, object.details_json
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                observed.push(format!(
                    "followup_no_champion:champions={}:objects={}:turn={}:mine=[{}]",
                    view.champions.len(),
                    view.objects.len(),
                    view.session.current_turn,
                    mine_objects
                ));
            } else {
                observed.push("followup_no_game_view".to_string());
            }
        }
        if !triggered && !self.backend.has_first_fixture_battle_id() {
            return Err(ProbeError::Api(ApiError::new(
                "neutral_encounter_missing",
                format!(
                    "movement sync did not emit a neutral encounter event: {}",
                    observed.join(" | ")
                ),
                false,
            )));
        }
        self.refresh()?;
        Ok(())
    }

    fn submit_move(
        &mut self,
        champion_id: &str,
        path: Vec<MoveCoord>,
        nonce: &str,
        now_ms: u64,
        replayed: bool,
    ) -> Result<CommandResponse, ProbeError> {
        let session_id = self.session_id()?;
        let response = self.backend.submit_move_intent(
            self.caller,
            &session_id,
            champion_id,
            path,
            nonce,
            now_ms,
        );
        self.record_command(&response, replayed)?;
        Ok(response)
    }

    fn sync_until_turn(&mut self, target_turn: u32) -> Result<(), ProbeError> {
        loop {
            let current_turn = self
                .state
                .game_view
                .as_ref()
                .ok_or(ProbeError::MissingGameView)?
                .session
                .current_turn;
            if current_turn >= target_turn {
                return Ok(());
            }
            let now_ms = u64::from(current_turn).saturating_mul(TURN_DURATION_MS);
            self.sync_turn_with_retry(now_ms, &format!("turn-{current_turn}"))?;
            self.refresh()?;
        }
    }

    fn sync_turn_with_retry(
        &mut self,
        now_ms: u64,
        key: &str,
    ) -> Result<CommandResponse, ProbeError> {
        let session_id = self.session_id()?;
        for attempt in 0..8_u64 {
            let nonce = self.nonces.next(&format!("sync-{key}-{attempt}"));
            let sync_now_ms =
                now_ms.saturating_add(attempt.saturating_mul(TURN_DURATION_MS + 1_000));
            let first =
                self.backend
                    .sync_session_turn(self.caller, &session_id, sync_now_ms, &nonce);
            if first
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                let _retry =
                    self.backend
                        .sync_session_turn(self.caller, &session_id, sync_now_ms, &nonce);
                return Ok(first);
            }
            self.record_command(&first, false)?;

            let retry =
                self.backend
                    .sync_session_turn(self.caller, &session_id, sync_now_ms, &nonce);
            self.record_command(&retry, true)?;
            self.record_retry(&first, &retry)?;
            return Ok(first);
        }

        Err(ProbeError::Api(domm_game::ApiError::new(
            "sync_session_turn_not_applied",
            "sync_session_turn retry loop did not apply or no-op",
            false,
        )))
    }

    fn build_training_yard(&mut self) -> Result<(), ProbeError> {
        let session_id = self.session_id()?;
        let turn_number = self.current_turn()?;
        let preview = self.backend.preview_build_town_structure(
            self.caller,
            &session_id,
            "town:west",
            "freehold-training-yard",
            turn_number,
        )?;
        if !preview.allowed {
            return Err(ProbeError::CommandFailed {
                command_type: "preview_build_town_structure".to_string(),
                code: preview
                    .disabled_reason
                    .unwrap_or_else(|| "build_not_available".to_string()),
            });
        }
        let nonce = self.nonces.next("build-training-yard");
        let response = self.backend.submit_build_town_structure(
            self.caller,
            &session_id,
            "town:west",
            "freehold-training-yard",
            turn_number,
            &nonce,
        );
        self.record_command(&response, false)?;
        self.backend
            .get_town_view(self.caller, &session_id, "town:west")?;
        self.refresh()?;
        self.state.mark_complete("build");
        Ok(())
    }

    fn recruit_levies(&mut self) -> Result<(), ProbeError> {
        let session_id = self.session_id()?;
        let turn_number = self.current_turn()?;
        let target = RecruitTarget::TownGarrison { slot_index: None };
        let preview = self.backend.preview_recruit_units(
            self.caller,
            &session_id,
            "town:west",
            "mudhook-levy",
            4,
            &target,
            turn_number,
        )?;
        if !preview.allowed {
            return Err(ProbeError::CommandFailed {
                command_type: "preview_recruit_units".to_string(),
                code: preview
                    .disabled_reason
                    .unwrap_or_else(|| "recruit_not_available".to_string()),
            });
        }
        let nonce = self.nonces.next("recruit-levies");
        let response = self.backend.submit_recruit_units(
            self.caller,
            &session_id,
            "town:west",
            "mudhook-levy",
            4,
            target,
            turn_number,
            &nonce,
        );
        self.record_command(&response, false)?;
        self.refresh()?;
        self.state.mark_complete("recruit");
        Ok(())
    }

    fn open_battle_panel(&mut self) -> Result<(), ProbeError> {
        let battle_id = self.backend.first_fixture_battle_id();
        self.backend.get_battle_state(
            self.caller,
            &battle_id,
            TURN_DURATION_MS.saturating_mul(8).saturating_add(1_000),
        )?;
        self.state.battle_panel_open = true;
        self.refresh()?;
        Ok(())
    }

    fn submit_defend_and_sync_battle(&mut self) -> Result<(), ProbeError> {
        let mut battle_id = String::new();
        let mut submitted = false;
        for attempt in 0..6_u64 {
            let battle = self
                .state
                .game_view
                .as_ref()
                .and_then(|view| view.battle.as_ref())
                .ok_or(ProbeError::MissingGameView)?;
            battle_id = battle.battle_id.clone();
            let active_stack_id = battle
                .active_stack_id
                .clone()
                .ok_or(ProbeError::MissingActiveBattleStack)?;
            let nonce = self.nonces.next(&format!("battle-defend-{attempt}"));
            let input = BattleActionInput {
                battle_id: battle_id.clone(),
                battle_stack_id: active_stack_id,
                action: "Defend".to_string(),
                ability_key: None,
                target_stack_id: None,
                destination: None,
            };
            let action_now_ms = TURN_DURATION_MS
                .saturating_mul(8)
                .saturating_add(attempt.saturating_mul(domm_game::BATTLE_ACTION_DEADLINE_MS + 1));
            let first = self.backend.submit_battle_action(
                self.caller,
                input.clone(),
                &nonce,
                action_now_ms,
            );
            if first.error.as_ref().is_some_and(|error| {
                matches!(
                    error.code.as_str(),
                    "battle_processing" | "battle_stack_not_active"
                )
            }) {
                let sync_nonce = self
                    .nonces
                    .next(&format!("sync-battle-processing-{attempt}"));
                let sync = self.backend.sync_battle(
                    self.caller,
                    &battle_id,
                    action_now_ms.saturating_add(domm_game::BATTLE_ACTION_DEADLINE_MS + 1),
                    &sync_nonce,
                );
                self.record_command(&sync, false)?;
                self.refresh()?;
                continue;
            }
            self.record_command(&first, false)?;

            let retry =
                self.backend
                    .submit_battle_action(self.caller, input, &nonce, action_now_ms);
            self.record_command(&retry, true)?;
            self.record_retry(&first, &retry)?;
            submitted = true;
            break;
        }
        if !submitted {
            return Err(ProbeError::Api(domm_game::ApiError::new(
                "battle_action_not_submitted",
                "battle action remained blocked by battle processing",
                false,
            )));
        }

        let sync_nonce = self.nonces.next("sync-battle");
        let sync = self.backend.sync_battle(
            self.caller,
            &battle_id,
            TURN_DURATION_MS
                .saturating_mul(8)
                .saturating_add(domm_game::BATTLE_ACTION_DEADLINE_MS),
            &sync_nonce,
        );
        self.record_command(&sync, false)?;
        self.refresh()?;
        self.state.mark_complete("battle");
        Ok(())
    }

    fn finish_match_result(&mut self) -> Result<(), ProbeError> {
        let session_id = self.session_id()?;
        let result = self
            .backend
            .finish_first_playable_match(self.caller, &session_id)?;
        self.state.apply_playable_result(&result);
        Ok(())
    }

    fn collect_remaining_pages(
        &mut self,
        session_id: &str,
        game_view: &mut GameView,
    ) -> Result<(), ProbeError> {
        let viewport = game_view.viewport.clone();
        let mut chunk_page = game_view.map_page_info.clone();
        while chunk_page.has_more {
            let page = self.backend.game_view(
                self.caller,
                session_id,
                GameViewRequest {
                    viewport: viewport.clone(),
                    chunk_cursor: chunk_page.next_cursor,
                    chunk_limit: chunk_page.limit,
                    object_cursor: None,
                    object_limit: game_view.object_page_info.limit,
                    events_after_seq: game_view.events.last().map_or(0, |event| event.event_seq),
                    event_limit: game_view.event_page_info.limit,
                    include_battle: game_view.battle_panel_requested(),
                },
            )?;
            game_view.map_chunks.extend(page.map_chunks);
            chunk_page = page.map_page_info;
        }

        let mut object_page = game_view.object_page_info.clone();
        while object_page.has_more {
            let page = self.backend.game_view(
                self.caller,
                session_id,
                GameViewRequest {
                    viewport: viewport.clone(),
                    chunk_cursor: None,
                    chunk_limit: game_view.map_page_info.limit,
                    object_cursor: object_page.next_cursor,
                    object_limit: object_page.limit,
                    events_after_seq: game_view.events.last().map_or(0, |event| event.event_seq),
                    event_limit: game_view.event_page_info.limit,
                    include_battle: game_view.battle_panel_requested(),
                },
            )?;
            game_view.objects.extend(page.objects);
            object_page = page.object_page_info;
        }
        Ok(())
    }

    fn record_lobby(
        &mut self,
        response: &LobbyCommandResponse,
        replayed: bool,
    ) -> Result<(), ProbeError> {
        self.state.record_lobby_response(response, replayed);
        if let Some(error) = response.error.as_ref() {
            return Err(ProbeError::CommandFailed {
                command_type: response.command_type.clone(),
                code: error.code.clone(),
            });
        }
        Ok(())
    }

    fn record_command(
        &mut self,
        response: &CommandResponse,
        replayed: bool,
    ) -> Result<(), ProbeError> {
        self.state.record_command_response(response, replayed);
        if let Some(status) = self.backend.get_command_status(&response.command_id) {
            self.state.last_command_status = Some(status);
        }
        if let Some(error) = response.error.as_ref() {
            return Err(ProbeError::CommandFailed {
                command_type: response.command_type.clone(),
                code: error.code.clone(),
            });
        }
        Ok(())
    }

    fn record_retry(
        &mut self,
        first: &CommandResponse,
        retry: &CommandResponse,
    ) -> Result<(), ProbeError> {
        if first.command_id != retry.command_id || first.payload_hash != retry.payload_hash {
            return Err(ProbeError::RetryDidNotReplay {
                command_type: first.command_type.clone(),
            });
        }
        self.state.retry_replays = self.state.retry_replays.saturating_add(1);
        Ok(())
    }

    fn session_id(&self) -> Result<String, ProbeError> {
        self.state
            .session_id
            .clone()
            .ok_or(ProbeError::MissingSession)
    }

    fn current_turn(&self) -> Result<u32, ProbeError> {
        Ok(self
            .state
            .game_view
            .as_ref()
            .ok_or(ProbeError::MissingGameView)?
            .session
            .current_turn)
    }
}

fn path_to_west_mine_neutral(x: u16, y: u16) -> Vec<MoveCoord> {
    let mut path = Vec::new();
    let mut current_x = x;
    let mut current_y = y;
    while current_x < 12 {
        current_x += 1;
        path.push(MoveCoord::new(current_x, current_y));
    }
    while current_x > 12 {
        current_x -= 1;
        path.push(MoveCoord::new(current_x, current_y));
    }
    while current_y > 22 {
        current_y -= 1;
        path.push(MoveCoord::new(current_x, current_y));
    }
    while current_y < 22 {
        current_y += 1;
        path.push(MoveCoord::new(current_x, current_y));
    }
    path
}

fn render_game_view(game_view: &GameView) -> Result<RenderedViewport, ProbeError> {
    render_opening_viewport(&ClientOpeningViewport {
        game_view: game_view.clone(),
        viewport: game_view.viewport.clone(),
        chunks: game_view.map_chunks.clone(),
        objects: game_view.objects.clone(),
        events: ApiEventPage {
            events: game_view.events.clone(),
            page_info: game_view.event_page_info.clone(),
        },
        sync_required: game_view.render_time.sync_required,
    })
}

fn session_id_from_lobby(response: &LobbyCommandResponse) -> Option<String> {
    match &response.result {
        LobbyCommandResult::Session(session) => Some(session.session_id.clone()),
        _ => None,
    }
}

trait BattlePanelRequest {
    fn battle_panel_requested(&self) -> bool;
}

impl BattlePanelRequest for GameView {
    fn battle_panel_requested(&self) -> bool {
        self.battle.is_some() || self.battle_summary.is_some()
    }
}

struct ClientNonceBook {
    prefix: String,
    next_index: u64,
}

impl ClientNonceBook {
    fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            next_index: 0,
        }
    }

    fn next(&mut self, intent: &str) -> String {
        self.next_index = self.next_index.saturating_add(1);
        format!("{}:{intent}:{}", self.prefix, self.next_index)
    }
}
