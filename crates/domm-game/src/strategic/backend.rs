use candid::Principal;

use crate::aftermath::{
    AftermathError, AftermathState, MatchSessionRecord, build_first_playable_aftermath_state,
};
use crate::champion::{
    ChampionState, ChampionView, ChampionViewResult, build_first_playable_champion_state,
};
use crate::driver::{ActiveMatchView, HeadlessBackend, PlayerView, SessionView};
use crate::economy::{EconomyState, build_first_playable_economy_state};
use crate::fixtures::{ScenarioFixture, TURN_DURATION_MS};
use crate::lifecycle::{LifecycleBackend, MatchHistoryEntry, ParticipantView};
use crate::map::{
    FirstPlayableMapState, MapChunkPage, ObjectViewPage, Viewport, build_first_playable_map_state,
};
use crate::movement::{
    MoveCoord, MovementPreview, MovementState, MovementSyncBudget, MovementSyncOutcome,
    build_first_playable_movement_state, submit_move_intent, sync_session_turn,
};
use crate::neutral::{
    NeutralState, apply_neutral_encounters_from_movement, build_first_playable_neutral_state,
};
use crate::town::{RecruitTarget, TownState, build_first_playable_town_state};
use crate::world_object::{
    WorldObjectState, apply_movement_object_interactions, build_first_playable_world_object_state,
};

use super::driver::StrategicBackend;
use super::types::{StrategicCall, StrategicCommandReceipt, StrategicError, StrategicGameView};

#[derive(Clone, Debug)]
pub struct StrategicFixtureBackend {
    fixture: ScenarioFixture,
    lifecycle: LifecycleBackend,
    map: FirstPlayableMapState,
    champions: ChampionState,
    movement: MovementState,
    economy: EconomyState,
    town: TownState,
    objects: WorldObjectState,
    neutral: NeutralState,
    last_movement_outcome: Option<MovementSyncOutcome>,
    calls: Vec<StrategicCall>,
    command_count: u32,
    event_count: u32,
    query_count: u32,
    now_ms: u64,
}

impl StrategicFixtureBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            lifecycle: LifecycleBackend::new(fixture.clone()),
            map: build_first_playable_map_state(),
            champions: build_first_playable_champion_state(),
            movement: build_first_playable_movement_state(),
            economy: build_first_playable_economy_state(),
            town: build_first_playable_town_state(),
            objects: build_first_playable_world_object_state(),
            neutral: build_first_playable_neutral_state(),
            last_movement_outcome: None,
            calls: Vec::new(),
            command_count: 0,
            event_count: 0,
            query_count: 0,
            now_ms: 0,
            fixture,
        }
    }

    #[must_use]
    pub fn calls(&self) -> &[StrategicCall] {
        &self.calls
    }

    #[must_use]
    pub fn command_count(&self) -> u32 {
        self.command_count
    }

    #[must_use]
    pub fn event_count(&self) -> u32 {
        self.event_count
    }

    #[must_use]
    pub fn query_count(&self) -> u32 {
        self.query_count
    }

    pub fn export_aftermath_state(&self) -> Result<AftermathState, AftermathError> {
        let mut state = build_first_playable_aftermath_state()?;
        state.session = MatchSessionRecord {
            session_id: self.fixture.ids.session_id.clone(),
            state: "active".to_string(),
            current_turn: self.movement.current_turn,
            max_turns: state.session.max_turns,
            winner_participant_id: None,
            finish_reason: None,
            last_command_id: Some("command:strategic:export-aftermath".to_string()),
        };
        state.champions = self.champions.clone();
        state.town = self.town.clone();
        state.economy = self.economy.clone();
        state.map = self.map.clone();
        state.neutral = self.neutral.clone();
        Ok(state)
    }

    pub fn get_my_player_public(&self, caller: Principal) -> Result<PlayerView, StrategicError> {
        Ok(self.lifecycle.get_my_player(caller)?)
    }

    pub fn get_session_public(&self, session_id: &str) -> Result<SessionView, StrategicError> {
        Ok(self.lifecycle.get_session(session_id)?)
    }

    pub fn get_my_participant_public(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ParticipantView, StrategicError> {
        Ok(self.lifecycle.get_my_participant(caller, session_id)?)
    }

    pub fn get_match_history_public(
        &self,
        caller: Principal,
        cursor: usize,
        limit: usize,
    ) -> Result<Vec<MatchHistoryEntry>, StrategicError> {
        Ok(self.lifecycle.get_match_history(caller, cursor, limit)?)
    }

    pub fn get_events_public(
        &self,
        caller: Principal,
        session_id: &str,
        events_after_seq: u64,
        limit: usize,
    ) -> Result<crate::command::EventPage, StrategicError> {
        Ok(self
            .lifecycle
            .get_events(caller, session_id, events_after_seq, limit)?)
    }

    pub fn visible_map_chunks_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<MapChunkPage, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self
            .map
            .map_chunk_views(&participant_id, viewport, cursor, limit))
    }

    pub fn visible_objects_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<ObjectViewPage, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self
            .map
            .object_views(&participant_id, viewport, cursor, limit))
    }

    pub fn my_champions_public(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<Vec<ChampionView>, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self
            .champions
            .champions
            .iter()
            .filter(|champion| champion.participant_id == participant_id)
            .filter_map(|champion| {
                match self.champions.champion_view_for(
                    &participant_id,
                    &champion.champion_id,
                    true,
                    self.movement.current_turn,
                ) {
                    ChampionViewResult::Visible(view) => Some(view),
                    ChampionViewResult::Hidden { .. } => None,
                }
            })
            .collect())
    }

    pub fn champion_view_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
    ) -> Result<ChampionViewResult, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let visible = self
            .champions
            .champions
            .iter()
            .find(|champion| champion.champion_id == champion_id)
            .is_some_and(|champion| {
                self.map
                    .is_visible_at(&participant_id, champion.x, champion.y)
            });
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self.champions.champion_view_for(
            &participant_id,
            champion_id,
            visible,
            self.movement.current_turn,
        ))
    }

    pub fn preview_move_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        now_ms: u64,
    ) -> Result<MovementPreview, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(crate::movement::preview_move_path(
            &self.movement,
            &self.map,
            &self.champions,
            &participant_id,
            champion_id,
            path,
            now_ms,
        )?)
    }

    pub fn preview_build_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
    ) -> Result<crate::town::BuildPreview, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self.town.preview_build_town_structure(
            &self.economy,
            &participant_id,
            town_id,
            building_slug,
            turn_number,
        )?)
    }

    pub fn preview_recruit_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        turn_number: u32,
    ) -> Result<crate::town::RecruitPreview, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        self.record_query(StrategicCall::InspectView { caller });
        Ok(self.town.preview_recruit_units(
            &self.economy,
            &participant_id,
            town_id,
            unit_slug,
            quantity,
            target,
            turn_number,
        )?)
    }

    #[must_use]
    pub fn current_turn(&self) -> u32 {
        self.movement.current_turn
    }

    #[must_use]
    pub fn turn_started_at(&self) -> u64 {
        u64::from(self.movement.current_turn.saturating_sub(1)) * TURN_DURATION_MS
    }

    fn participant_for_caller(&self, caller: Principal) -> Result<&str, StrategicError> {
        if caller == self.fixture.principals.player_one {
            Ok(&self.fixture.ids.participant_one_id)
        } else if caller == self.fixture.principals.player_two {
            Ok(&self.fixture.ids.participant_two_id)
        } else {
            Err(StrategicError::UnknownCaller)
        }
    }

    fn command_id(&self, kind: &str, client_nonce: u64) -> String {
        format!(
            "command:strategic:{}:{kind}:{client_nonce}",
            self.fixture.ids.session_id
        )
    }

    fn receipt(&self, command_kind: &str, command_id: String) -> StrategicCommandReceipt {
        StrategicCommandReceipt {
            command_kind: command_kind.to_string(),
            command_id,
            current_turn: self.movement.current_turn,
            command_count: self.command_count,
            event_count: self.event_count,
        }
    }

    fn record_update(&mut self, call: StrategicCall, event_delta: u32) {
        self.calls.push(call);
        self.command_count = self.command_count.saturating_add(1);
        self.event_count = self.event_count.saturating_add(event_delta.max(1));
    }

    fn record_query(&mut self, call: StrategicCall) {
        self.calls.push(call);
        self.query_count = self.query_count.saturating_add(1);
    }

    fn ensure_active_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<(), StrategicError> {
        self.lifecycle.get_game_view(caller, session_id)?;
        Ok(())
    }
}

impl HeadlessBackend for StrategicFixtureBackend {
    type Error = StrategicError;

    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, Self::Error> {
        let view = self
            .lifecycle
            .register_player(caller, display_name, client_nonce)?;
        self.record_update(StrategicCall::RegisterPlayer { caller }, 1);
        Ok(view)
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, Self::Error> {
        let view = self
            .lifecycle
            .create_session(caller, client_nonce, scenario_seed)?;
        self.record_update(StrategicCall::CreateSession { caller }, 1);
        Ok(view)
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        let view = self
            .lifecycle
            .join_session(caller, session_id, client_nonce)?;
        self.record_update(StrategicCall::JoinSession { caller }, 1);
        Ok(view)
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        let view = self
            .lifecycle
            .mark_ready(caller, session_id, client_nonce)?;
        self.record_update(StrategicCall::MarkReady { caller }, 1);
        Ok(view)
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        let view = self
            .lifecycle
            .start_session(caller, session_id, client_nonce)?;
        self.record_update(StrategicCall::StartSession { caller }, 1);
        Ok(view)
    }

    fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, Self::Error> {
        let view = self.lifecycle.get_game_view(caller, session_id)?;
        self.record_query(StrategicCall::InspectView { caller });
        Ok(view)
    }
}

impl StrategicBackend for StrategicFixtureBackend {
    fn inspect_strategic_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<StrategicGameView, StrategicError> {
        self.lifecycle.get_game_view(caller, session_id)?;
        self.record_query(StrategicCall::InspectView { caller });
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let champion = self.champions.champion("champion:west")?;
        let resources = self.economy.participant(&participant_id)?.balances.clone();
        let viewport = self.map.opening_viewport_snapshot(&participant_id);
        let built_buildings = self
            .town
            .buildings
            .iter()
            .filter(|building| building.town_id == "town:west")
            .map(|building| building.building_slug.clone())
            .collect::<Vec<_>>();
        let recruit_pool_available = self
            .town
            .recruit_pools
            .iter()
            .find(|pool| pool.town_id == "town:west" && pool.unit_slug == "mudhook-levy")
            .map_or(0, |pool| pool.available);
        let town_garrison_quantity = self
            .town
            .garrison_stacks
            .iter()
            .filter(|stack| stack.owner_id == "town:west")
            .map(|stack| stack.quantity)
            .sum();
        let pending_battle_key = self
            .neutral
            .encounters
            .last()
            .map(|encounter| encounter.battle_key.clone());
        let sync_required = self.movement.time_view(self.now_ms).sync_required;
        let approximate_query_bytes = approximate_query_bytes(
            viewport.chunks.len(),
            viewport.objects.len(),
            built_buildings.len(),
            self.neutral.encounters.len(),
        );

        Ok(StrategicGameView {
            session_id: session_id.to_string(),
            participant_id,
            current_turn: self.movement.current_turn,
            sync_required,
            champion_id: champion.champion_id.clone(),
            champion_status: champion.status.clone(),
            champion_x: champion.x,
            champion_y: champion.y,
            resources,
            built_buildings,
            recruit_pool_available,
            town_garrison_quantity,
            visible_chunk_count: viewport.chunks.len() as u32,
            visible_object_count: viewport.objects.len() as u32,
            object_command_count: self.objects.commands.len() as u32,
            movement_snapshot_count: self.movement.snapshots.len() as u32,
            neutral_encounter_count: self.neutral.encounters.len() as u32,
            pending_battle_key,
            command_count: self.command_count,
            event_count: self.event_count,
            query_count: self.query_count,
            approximate_query_bytes,
        })
    }

    fn submit_move_intent_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: u64,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        submit_move_intent(
            &mut self.movement,
            &self.map,
            &self.champions,
            &participant_id,
            champion_id,
            path.clone(),
            client_nonce,
            now_ms,
        )?;
        self.now_ms = now_ms;
        self.record_update(
            StrategicCall::SubmitMoveIntent {
                caller,
                champion_id: champion_id.to_string(),
                path,
            },
            1,
        );
        Ok(self.receipt("submit_move_intent", self.command_id("move", client_nonce)))
    }

    fn sync_session_turn_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let outcome = sync_session_turn(
            &mut self.movement,
            &mut self.map,
            &mut self.champions,
            now_ms,
            MovementSyncBudget::default(),
        )?;
        let event_delta = outcome.snapshots.len() as u32
            + outcome.battle_starts.len() as u32
            + outcome.object_stops.len() as u32
            + u32::from(outcome.advanced_turn);
        let command_id = outcome.command_id.clone();
        self.last_movement_outcome = Some(outcome);
        self.now_ms = now_ms;
        self.record_update(StrategicCall::SyncTurn { caller, now_ms }, event_delta);
        Ok(self.receipt("sync_session_turn", command_id))
    }

    fn apply_movement_object_interactions_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let movement_outcome = self
            .last_movement_outcome
            .clone()
            .ok_or(StrategicError::MissingMovementSync)?;
        let outcomes = apply_movement_object_interactions(
            &mut self.objects,
            &mut self.map,
            &mut self.economy,
            &self.champions,
            &movement_outcome,
            movement_outcome.from_turn,
            now_ms,
        )?;
        self.now_ms = now_ms;
        let command_id = format!(
            "command:strategic:movement-objects:{}",
            movement_outcome.command_id
        );
        self.record_update(
            StrategicCall::ApplyMovementObjects { caller },
            outcomes.len() as u32,
        );
        Ok(self.receipt("apply_movement_objects", command_id))
    }

    fn materialize_income_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let command_id = self.command_id("income", client_nonce);
        let outcome = self
            .economy
            .materialize_income(&participant_id, turn_number, &command_id)?;
        self.record_update(
            StrategicCall::MaterializeIncome {
                caller,
                turn_number,
            },
            outcome.ledger_rows_touched as u32 + outcome.balance_updates as u32,
        );
        Ok(self.receipt("materialize_income", command_id))
    }

    fn build_town_structure_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let command_id = self.command_id("build", client_nonce);
        let ledger_before = self.economy.ledger_entries.len();
        self.town.submit_build_town_structure(
            &mut self.economy,
            &participant_id,
            town_id,
            building_slug,
            turn_number,
            &command_id,
        )?;
        let ledger_delta = self
            .economy
            .ledger_entries
            .len()
            .saturating_sub(ledger_before) as u32;
        self.record_update(
            StrategicCall::BuildTownStructure {
                caller,
                town_id: town_id.to_string(),
                building_slug: building_slug.to_string(),
            },
            ledger_delta + 1,
        );
        Ok(self.receipt("build_town_structure", command_id))
    }

    fn recruit_units_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let command_id = self.command_id("recruit", client_nonce);
        let ledger_before = self.economy.ledger_entries.len();
        self.town.submit_recruit_units(
            &mut self.economy,
            &participant_id,
            town_id,
            unit_slug,
            quantity,
            target,
            turn_number,
            &command_id,
        )?;
        let ledger_delta = self
            .economy
            .ledger_entries
            .len()
            .saturating_sub(ledger_before) as u32;
        self.record_update(
            StrategicCall::RecruitUnits {
                caller,
                town_id: town_id.to_string(),
                unit_slug: unit_slug.to_string(),
                quantity,
            },
            ledger_delta + 1,
        );
        Ok(self.receipt("recruit_units", command_id))
    }

    fn apply_neutral_encounters_public(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<StrategicCommandReceipt, StrategicError> {
        self.ensure_active_match(caller, session_id)?;
        let movement_outcome = self
            .last_movement_outcome
            .clone()
            .ok_or(StrategicError::MissingMovementSync)?;
        let encounters = apply_neutral_encounters_from_movement(
            &mut self.neutral,
            &mut self.map,
            &self.champions,
            &movement_outcome,
        )?;
        let command_id = format!("command:strategic:neutral:{}", movement_outcome.command_id);
        self.record_update(
            StrategicCall::ApplyNeutralEncounters { caller },
            encounters.len() as u32,
        );
        Ok(self.receipt("apply_neutral_encounters", command_id))
    }
}

fn approximate_query_bytes(
    chunk_count: usize,
    object_count: usize,
    building_count: usize,
    encounter_count: usize,
) -> u32 {
    512 + (chunk_count as u32 * 768)
        + (object_count as u32 * 192)
        + (building_count as u32 * 64)
        + (encounter_count as u32 * 128)
}
