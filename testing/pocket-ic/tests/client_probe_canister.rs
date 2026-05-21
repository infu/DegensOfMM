use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use candid::{CandidType, Principal};
use canic_testkit::pic::{StandaloneCanisterFixture, install_prebuilt_canister_with_cycles};
use domm_client_probe::{PlayableWebClient, ProbeError, WebClientBackend, WebClientState};
use domm_degens_canister::{
    DiagnosticBenchmarkCallPage, DiagnosticStorageSnapshot, EndpointKind, REQUIRED_GAME_ENDPOINTS,
};
use domm_game::{
    ApiError, ApiEventPage, ApiMetrics, ApiTownView, BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH,
    BattleActionInput, BattleSummary, BattleView, BuildPreview, ChampionView, CommandResponse,
    CommandStatus, CommandStatusView, ContentManifestResponse, FIRST_PLAYABLE_MAP_HEIGHT,
    FIRST_PLAYABLE_MAP_WIDTH, FIRST_PLAYABLE_RULESET_ID, FixtureApiBackend, GameView,
    GameViewRequest, LegalBattleAction, LobbyCommandResponse, LobbyCommandResult, MAX_CHUNK_LIMIT,
    MapChunkPage, MatchHistoryPage, MoveCoord, ObjectViewPage, PageInfo, ParticipantSummary,
    ParticipantView, PlayableMatchView, RecruitPreview, RecruitTarget, RenderTimeMeta, SessionView,
    SetupProgressView, first_playable_content_manifest, first_playable_fixture,
    opening_viewport_for_slot,
};
use serde::{Deserialize, Serialize};

const GATE_M_ENTITIES: &[&str] = &[
    "GameSession",
    "GameParticipant",
    "PlayerAccount",
    "LobbyCommand",
    "GameCommand",
    "CommandEffect",
    "GameEvent",
    "Champion",
    "Town",
    "TownBuilding",
    "TownGarrisonStack",
    "PlayerMatchSummary",
    "Battle",
    "BattleStack",
    "BattleOccupancy",
    "BattleObstacle",
    "MovementIntent",
    "MovementSnapshot",
    "ResourceLedgerEntry",
    "ResourceLedgerTurnSummary",
    "ParticipantObjectVisit",
    "WorldObject",
    "NeutralArmy",
    "MapOccupancy",
    "SkirmishSettingsState",
    "ProceduralMapState",
    "NavalRouteState",
    "SiegeRuleState",
];

#[test]
fn gate_m_web_client_probe_runs_against_pocket_ic_canister_adapter() {
    let fixture = first_playable_fixture();
    let mut backend = CanisterWebClientBackend::new(install_degens_canister_fixture());
    backend.reset_benchmark_canister();
    backend.set_benchmark_scenario("diagnostics");
    let initial_storage = backend.diagnostic_snapshot(GATE_M_ENTITIES);
    assert_eq!(initial_storage.total_rows, 0);
    let fixture_opening = fixture_opening_game_view(&fixture);

    backend.set_benchmark_scenario("gate_m_web_client_probe");
    let mut client =
        PlayableWebClient::new(backend, fixture.clone(), fixture.principals.player_one);
    client
        .play_first_playable_walkthrough()
        .expect("web client should play the canister-backed first match path");

    let state = client.state().clone();
    let view = client.view_model();
    assert_canister_client_state(&state);
    assert_eq!(view.screen, "result");
    assert!(view.rematch_available);
    assert!(!view.match_history.is_empty());
    assert!(
        view.match_result
            .as_deref()
            .is_some_and(|result| result.contains("finished") || result.contains("win"))
    );

    backend = client.into_backend();
    let canister_opening = backend
        .opening_game_view
        .as_ref()
        .expect("adapter should retain an opening canister DTO sample");
    assert_representative_dtos_match(&fixture_opening, canister_opening);

    backend.set_benchmark_scenario("diagnostics");
    let final_storage = backend.diagnostic_snapshot(GATE_M_ENTITIES);
    assert_eq!(row_count(&final_storage, "GameSession"), 1);
    assert_eq!(row_count(&final_storage, "GameParticipant"), 2);
    assert_eq!(row_count(&final_storage, "PlayerAccount"), 2);
    assert_eq!(row_count(&final_storage, "PlayerMatchSummary"), 2);
    assert!(row_count(&final_storage, "GameCommand") > 0);
    assert!(row_count(&final_storage, "CommandEffect") > 0);
    assert!(row_count(&final_storage, "GameEvent") > 0);
    assert!(row_count(&final_storage, "MovementSnapshot") > 0);
    assert!(row_count(&final_storage, "TownBuilding") > 0);
    assert!(row_count(&final_storage, "TownGarrisonStack") > 0);
    assert!(row_count(&final_storage, "Battle") >= 3);
    assert!(row_count(&final_storage, "BattleStack") > 0);
    assert!(row_count(&final_storage, "BattleObstacle") > 0);
    assert!(row_count(&final_storage, "SkirmishSettingsState") > 0);
    assert!(row_count(&final_storage, "ProceduralMapState") > 0);
    assert!(row_count(&final_storage, "NavalRouteState") > 0);
    assert!(row_count(&final_storage, "SiegeRuleState") > 0);
    assert!(final_storage.total_rows > initial_storage.total_rows);
    assert!(final_storage.stable_memory_pages >= initial_storage.stable_memory_pages);

    let probe_metrics = backend.probe_metrics();
    let api_metrics = WebClientBackend::metrics(&backend);
    assert!(probe_metrics.update_calls > 0);
    assert!(probe_metrics.query_calls > 0);
    assert!(probe_metrics.total_response_bytes > 0);
    assert!(probe_metrics.max_response_bytes > 0);
    assert!(api_metrics.command_response_count > 0);
    assert!(api_metrics.lobby_response_count > 0);
    assert!(api_metrics.api_event_count > 0);
    eprintln!(
        "Gate M canister client metrics: updates={} queries={} events={} total_rows={} row_growth={} stable_pages_start={} stable_pages_final={} response_bytes_total={} max_response_bytes={} max_response_method={}",
        probe_metrics.update_calls,
        probe_metrics.query_calls,
        probe_metrics.observed_event_count,
        final_storage.total_rows,
        final_storage
            .total_rows
            .saturating_sub(initial_storage.total_rows),
        initial_storage.stable_memory_pages,
        final_storage.stable_memory_pages,
        probe_metrics.total_response_bytes,
        probe_metrics.max_response_bytes,
        probe_metrics.max_response_method
    );
    backend.write_benchmark_artifacts("gate_m_web_client_probe", &initial_storage, &final_storage);
}

struct CanisterWebClientBackend {
    fixture: StandaloneCanisterFixture,
    session_id: Option<String>,
    player_one: Option<Principal>,
    player_two: Option<Principal>,
    player_one_participant_id: Option<String>,
    first_battle_id: Option<String>,
    include_battle_in_game_view: bool,
    opening_game_view: Option<GameView>,
    current_turn: u32,
    server_now_ms: u64,
    advanced_time_nonces: BTreeSet<String>,
    town_view_cache: BTreeMap<String, ApiTownView>,
    metrics: RefCell<CanisterProbeMetrics>,
}

impl CanisterWebClientBackend {
    fn new(fixture: StandaloneCanisterFixture) -> Self {
        Self {
            fixture,
            session_id: None,
            player_one: None,
            player_two: None,
            player_one_participant_id: None,
            first_battle_id: None,
            include_battle_in_game_view: false,
            opening_game_view: None,
            current_turn: 1,
            server_now_ms: 0,
            advanced_time_nonces: BTreeSet::new(),
            town_view_cache: BTreeMap::new(),
            metrics: RefCell::new(CanisterProbeMetrics::default()),
        }
    }

    fn probe_metrics(&self) -> CanisterProbeMetrics {
        self.metrics.borrow().clone()
    }

    fn reset_benchmark_canister(&self) {
        self.metrics
            .borrow_mut()
            .reset_benchmark_canister(&self.fixture);
    }

    fn set_benchmark_scenario(&self, scenario_id: &str) {
        self.metrics
            .borrow_mut()
            .set_benchmark_scenario(scenario_id);
    }

    fn write_benchmark_artifacts(
        &self,
        benchmark_name: &str,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
    ) {
        self.metrics.borrow_mut().write_benchmark_artifacts(
            &self.fixture,
            benchmark_name,
            initial_storage,
            final_storage,
        );
    }

    fn advance_time_without_timers_ms(&mut self, millis: u64) {
        self.advance_clock_ms(millis);
    }

    fn advance_time_for_timers_ms(&mut self, millis: u64) {
        self.advance_clock_ms(millis);
        self.fixture.pic().tick_n(5);
    }

    fn advance_clock_ms(&mut self, millis: u64) {
        self.fixture
            .pic()
            .advance_time(Duration::from_millis(millis));
        self.server_now_ms = self.server_now_ms.saturating_add(millis);
    }

    fn advance_to_time_ms(&mut self, now_ms: u64) {
        let millis = now_ms.saturating_sub(self.server_now_ms);
        if millis > 0 {
            self.advance_clock_ms(millis);
        }
    }

    fn advance_once_for_nonce_by_ms(&mut self, method: &str, client_nonce: &str, millis: u64) {
        if self
            .advanced_time_nonces
            .insert(format!("{method}:{client_nonce}"))
        {
            self.advance_clock_ms(millis);
        }
    }

    fn default_game_view_request(&self) -> GameViewRequest {
        GameViewRequest {
            viewport: opening_viewport_for_slot(0),
            chunk_cursor: None,
            chunk_limit: MAX_CHUNK_LIMIT,
            object_cursor: None,
            object_limit: 128,
            events_after_seq: 0,
            event_limit: 25,
            include_battle: self.include_battle_in_game_view,
        }
    }

    #[track_caller]
    fn query_result<T>(
        &self,
        caller: Principal,
        method: &str,
        args: impl candid::utils::ArgumentEncoder,
    ) -> Result<T, ProbeError>
    where
        T: CandidType + for<'de> serde::Deserialize<'de>,
    {
        let callsite = std::panic::Location::caller();
        let before = self.metrics.borrow().benchmark_sample(&self.fixture);
        let started = Instant::now();
        let response = self
            .fixture
            .pic()
            .query_call_as(self.fixture.canister_id(), caller, method, args)
            .map_err(|error| {
                ProbeError::Api(ApiError::new(
                    "pocket_ic_query_failed",
                    format!("{method} query failed at {callsite}: {error:?}"),
                    false,
                ))
            })?;
        let elapsed = started.elapsed();
        let after = self.metrics.borrow().benchmark_sample(&self.fixture);
        let mut metrics = self.metrics.borrow_mut();
        metrics.record_query(method, &response);
        let query_instruction_delta = metrics.pull_query_instruction(&self.fixture, method);
        metrics.record_benchmark_call(
            method,
            "query",
            &response,
            elapsed,
            before,
            after,
            query_instruction_delta,
            Vec::new(),
        );
        response.map_err(ProbeError::from)
    }

    fn update_result<T>(
        &self,
        caller: Principal,
        method: &str,
        args: impl candid::utils::ArgumentEncoder,
    ) -> Result<T, ProbeError>
    where
        T: CandidType + for<'de> serde::Deserialize<'de>,
    {
        let before = self.metrics.borrow().benchmark_sample(&self.fixture);
        let started = Instant::now();
        let response = self
            .fixture
            .pic()
            .update_call_as(self.fixture.canister_id(), caller, method, args)
            .map_err(|error| {
                ProbeError::Api(ApiError::new(
                    "pocket_ic_update_failed",
                    format!("{method} update failed: {error:?}"),
                    false,
                ))
            })?;
        let elapsed = started.elapsed();
        let after = self.metrics.borrow().benchmark_sample(&self.fixture);
        let mut metrics = self.metrics.borrow_mut();
        metrics.record_update(method, &response);
        let update_measurement = metrics.pull_update_measurement(&self.fixture, method);
        metrics.record_benchmark_call(
            method,
            "update",
            &response,
            elapsed,
            before,
            after,
            update_measurement
                .as_ref()
                .map(|measurement| measurement.instruction_delta),
            update_measurement
                .map(|measurement| measurement.repo_ops)
                .unwrap_or_default(),
        );
        response.map_err(ProbeError::from)
    }

    fn update_lobby_response(
        &mut self,
        caller: Principal,
        method: &str,
        args: impl candid::utils::ArgumentEncoder,
    ) -> Result<LobbyCommandResponse, ProbeError> {
        let response = self.update_result::<LobbyCommandResponse>(caller, method, args)?;
        self.metrics.borrow_mut().observe_lobby_response(&response);
        self.current_turn = self
            .current_turn
            .max(response.effective_turn)
            .max(response.durable_turn)
            .max(1);
        Ok(response)
    }

    fn update_command_response(
        &mut self,
        caller: Principal,
        method: &str,
        args: impl candid::utils::ArgumentEncoder,
    ) -> Result<CommandResponse, ProbeError> {
        let response = self.update_result::<CommandResponse>(caller, method, args)?;
        self.current_turn = self
            .current_turn
            .max(response.effective_turn)
            .max(response.durable_turn)
            .max(1);
        self.town_view_cache.clear();
        self.observe_command_response(&response);
        Ok(response)
    }

    fn observe_command_response(&mut self, response: &CommandResponse) {
        self.metrics.borrow_mut().observe_command_response(response);
        if self.first_battle_id.is_none() {
            self.first_battle_id =
                battle_id_from_events(response, "neutral_encounter_pending").ok();
        }
    }

    fn observe_game_view(&self, view: &GameView) {
        self.metrics
            .borrow_mut()
            .observe_event_count(view.events.len() as u32);
    }

    fn observe_event_page(&self, page: &ApiEventPage) {
        self.metrics.borrow_mut().observe_event_page(page);
    }

    fn capture_first_battle_id_from_public_events(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<(), ProbeError> {
        if self.first_battle_id.is_some() {
            return Ok(());
        }
        let page = self.query_result::<ApiEventPage>(
            caller,
            "get_events_after",
            (session_id.to_string(), "public".to_string(), 0_u64, 200_u32),
        )?;
        self.observe_event_page(&page);
        self.first_battle_id =
            battle_id_from_event_views(&page.events, "neutral_encounter_pending");
        Ok(())
    }

    fn player_one_or_anonymous(&self) -> Principal {
        self.player_one.unwrap_or_else(Principal::anonymous)
    }

    fn resolve_champion_id(
        &self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
    ) -> Result<String, ProbeError> {
        if !champion_id.starts_with("champion:") {
            return Ok(champion_id.to_string());
        }
        Ok(self.owned_champion_id(caller, session_id)?.champion_id)
    }

    fn owned_champion_id(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ChampionView, ProbeError> {
        let champions = self.query_result::<Vec<ChampionView>>(
            caller,
            "get_my_champions",
            (session_id.to_string(),),
        )?;
        champions.into_iter().next().ok_or_else(|| {
            ProbeError::Api(ApiError::new(
                "missing_owned_champion",
                "caller has no champion in session",
                false,
            ))
        })
    }

    fn assert_applied(response: &CommandResponse) -> Result<(), ProbeError> {
        if let Some(error) = response.error.as_ref() {
            return Err(ProbeError::CommandFailed {
                command_type: response.command_type.clone(),
                code: error.code.clone(),
            });
        }
        if response.status != CommandStatus::Applied {
            return Err(ProbeError::CommandFailed {
                command_type: response.command_type.clone(),
                code: format!("status_{:?}", response.status),
            });
        }
        Ok(())
    }

    fn submit_move_and_sync_until_event(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        move_nonce: &str,
        sync_nonce_prefix: &str,
        now_ms: u64,
        expected_event_type: &str,
    ) -> Result<(CommandResponse, bool), ProbeError> {
        let moved = self.update_command_response(
            caller,
            "submit_move_intent",
            (
                session_id.to_string(),
                champion_id.to_string(),
                path.clone(),
                move_nonce.to_string(),
            ),
        )?;
        Self::assert_applied(&moved)?;

        let max_sync_calls = path.len().saturating_add(8).max(12);
        let sync_now_ms = now_ms.max(self.server_now_ms.saturating_add(61_000));
        self.sync_until_event(
            caller,
            session_id,
            sync_nonce_prefix,
            sync_now_ms,
            expected_event_type,
            max_sync_calls,
        )
    }

    fn sync_until_event(
        &mut self,
        caller: Principal,
        session_id: &str,
        sync_nonce_prefix: &str,
        now_ms: u64,
        expected_event_type: &str,
        max_sync_calls: usize,
    ) -> Result<(CommandResponse, bool), ProbeError> {
        let max_sync_calls = max_sync_calls.max(12);
        let mut saw_partial_sync = false;
        self.advance_to_time_ms(now_ms);
        for attempt in 0..max_sync_calls {
            let synced = self.update_command_response(
                caller,
                "sync_session_turn",
                (
                    session_id.to_string(),
                    format!("{sync_nonce_prefix}{attempt}"),
                ),
            )?;
            if expected_event_type != "session_turn_synced"
                && self.public_event_seen(caller, session_id, expected_event_type)?
            {
                return Ok((synced, saw_partial_sync));
            }
            if synced
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                self.advance_clock_ms(61_000);
                continue;
            }
            Self::assert_applied(&synced)?;
            let partial_sync = synced
                .events
                .iter()
                .any(|event| event.event_type == "movement_sync_incomplete");
            saw_partial_sync |= partial_sync;
            if synced
                .events
                .iter()
                .any(|event| event.event_type == expected_event_type)
            {
                return Ok((synced, saw_partial_sync));
            }
            if !partial_sync {
                self.advance_clock_ms(61_000);
            }
        }

        Err(ProbeError::Api(ApiError::new(
            "expected_event_missing",
            format!("sync_session_turn did not emit {expected_event_type}"),
            false,
        )))
    }

    fn public_event_seen(
        &self,
        caller: Principal,
        session_id: &str,
        event_type: &str,
    ) -> Result<bool, ProbeError> {
        let page = self.query_result::<ApiEventPage>(
            caller,
            "get_events_after",
            (session_id.to_string(), "public".to_string(), 0_u64, 200_u32),
        )?;
        self.observe_event_page(&page);
        Ok(page
            .events
            .iter()
            .any(|event| event.event_type == event_type))
    }

    fn sync_map_turn_if_due(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<(), ProbeError> {
        let synced = self.update_command_response(
            caller,
            "sync_session_turn",
            (session_id.to_string(), client_nonce.to_string()),
        )?;
        if synced
            .error
            .as_ref()
            .is_some_and(|error| error.code == "turn_not_due")
        {
            return Ok(());
        }
        Self::assert_applied(&synced)
    }

    fn resolve_battle_to_end(
        &mut self,
        caller: Principal,
        session_id: &str,
        battle_id: &str,
        nonce_prefix: &str,
    ) -> Result<BattleView, ProbeError> {
        self.resolve_battle_to_end_for_callers(
            caller,
            &[caller],
            session_id,
            battle_id,
            nonce_prefix,
        )
    }

    fn resolve_battle_to_end_for_callers(
        &mut self,
        sync_caller: Principal,
        callers: &[Principal],
        session_id: &str,
        battle_id: &str,
        nonce_prefix: &str,
    ) -> Result<BattleView, ProbeError> {
        for step in 0..160 {
            let mut actionable_view: Option<(Principal, BattleView)> = None;
            let mut fallback_view = None;
            for caller in callers.iter().copied() {
                let view = self.query_result::<BattleView>(
                    caller,
                    "get_battle_state",
                    (session_id.to_string(), battle_id.to_string()),
                )?;
                if view.state == "resolved" {
                    let synced = self.update_command_response(
                        sync_caller,
                        "sync_battle",
                        (
                            session_id.to_string(),
                            battle_id.to_string(),
                            format!("{nonce_prefix}:aftermath:{step}"),
                        ),
                    )?;
                    Self::assert_applied(&synced)?;
                    let turn_synced = self.update_command_response(
                        sync_caller,
                        "sync_session_turn",
                        (
                            session_id.to_string(),
                            format!("{nonce_prefix}:post-battle-turn:{step}"),
                        ),
                    )?;
                    if !turn_synced
                        .error
                        .as_ref()
                        .is_some_and(|error| error.code == "turn_not_due")
                    {
                        Self::assert_applied(&turn_synced)?;
                    }
                    return self.query_result::<BattleView>(
                        sync_caller,
                        "get_battle_state",
                        (session_id.to_string(), battle_id.to_string()),
                    );
                }
                if actionable_view.is_none() && !view.legal_actions_for_caller.is_empty() {
                    actionable_view = Some((caller, view));
                } else if fallback_view.is_none() {
                    fallback_view = Some(view);
                }
            }

            if let Some((caller, view)) = actionable_view {
                let input = choose_battle_action_for_goal(&view, caller == callers[0]);
                let submitted = self.update_command_response(
                    caller,
                    "submit_battle_action",
                    (
                        session_id.to_string(),
                        input,
                        format!("{nonce_prefix}:action:{step}"),
                    ),
                )?;
                if submitted
                    .error
                    .as_ref()
                    .is_some_and(|error| error.code == "battle_processing")
                {
                    let synced = self.update_command_response(
                        sync_caller,
                        "sync_battle",
                        (
                            session_id.to_string(),
                            battle_id.to_string(),
                            format!("{nonce_prefix}:processing-sync:{step}"),
                        ),
                    )?;
                    Self::assert_applied(&synced)?;
                    continue;
                }
                Self::assert_applied(&submitted)?;
                continue;
            }

            if fallback_view.is_some() {
                self.advance_time_without_timers_ms(domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
                let synced = self.update_command_response(
                    sync_caller,
                    "sync_battle",
                    (
                        session_id.to_string(),
                        battle_id.to_string(),
                        format!("{nonce_prefix}:sync:{step}"),
                    ),
                )?;
                Self::assert_applied(&synced)?;
            }
        }

        Err(ProbeError::Api(ApiError::new(
            "battle_not_resolved",
            format!("battle {battle_id} did not resolve within the test budget"),
            false,
        )))
    }

    fn finish_canister_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<PlayableMatchView, ProbeError> {
        let neutral_battle_id = self.first_battle_id.clone().ok_or_else(|| {
            ProbeError::Api(ApiError::new(
                "missing_neutral_battle",
                "web walkthrough did not capture a neutral battle id",
                false,
            ))
        })?;
        self.resolve_battle_to_end(
            caller,
            session_id,
            &neutral_battle_id,
            "nonce:gate-m:neutral-battle",
        )?;
        self.sync_map_turn_if_due(caller, session_id, "nonce:gate-m:post-neutral-turn")?;

        let west = self.owned_champion_id(caller, session_id)?;
        let west_champion_id = west.champion_id.clone();
        let player_two = self.player_two.ok_or_else(|| {
            ProbeError::Api(ApiError::new(
                "missing_second_player",
                "web walkthrough did not record the second player principal",
                false,
            ))
        })?;
        let east_champion_id = self.owned_champion_id(player_two, session_id)?.champion_id;
        let first_east_stage = ((west.x + 1)..=22)
            .map(|x| MoveCoord::new(x, west.y))
            .collect::<Vec<_>>();
        self.submit_move_and_sync_until_event(
            caller,
            session_id,
            &west_champion_id,
            first_east_stage,
            "nonce:gate-m:move:east-stage-1",
            "nonce:gate-m:sync:east-stage-1:",
            549_000_u64,
            "session_turn_synced",
        )?;

        let second_east_stage = (23..=32)
            .map(|x| MoveCoord::new(x, west.y))
            .collect::<Vec<_>>();
        self.submit_move_and_sync_until_event(
            caller,
            session_id,
            &west_champion_id,
            second_east_stage,
            "nonce:gate-m:move:east-stage-2",
            "nonce:gate-m:sync:east-stage-2:",
            610_000_u64,
            "session_turn_synced",
        )?;

        let mut champion_path = (33..=39)
            .map(|x| MoveCoord::new(x, west.y))
            .collect::<Vec<_>>();
        champion_path.push(MoveCoord::new(39, 23));
        champion_path.push(MoveCoord::new(39, 24));
        let (champion_sync, _) = self.submit_move_and_sync_until_event(
            caller,
            session_id,
            &west_champion_id,
            champion_path,
            "nonce:gate-m:move:champion",
            "nonce:gate-m:sync:champion:",
            671_000_u64,
            "champion_encounter_pending",
        )?;
        let champion_battle_id =
            battle_id_from_events(&champion_sync, "champion_encounter_pending")?;
        self.resolve_battle_to_end_for_callers(
            caller,
            &[caller, player_two],
            session_id,
            &champion_battle_id,
            "nonce:gate-m:champion-battle",
        )?;
        self.sync_map_turn_if_due(caller, session_id, "nonce:gate-m:post-champion-turn")?;

        let (town_contact_sync, _) = self.submit_move_and_sync_until_event(
            caller,
            session_id,
            &west_champion_id,
            vec![MoveCoord::new(40, 24), MoveCoord::new(41, 24)],
            "nonce:gate-m:move:town",
            "nonce:gate-m:sync:town:",
            732_000_u64,
            "town_encounter_pending",
        )?;
        let town_battle_id = battle_id_from_events(&town_contact_sync, "town_encounter_pending")?;
        self.advance_time_without_timers_ms(domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
        let town_sync = self.update_command_response(
            caller,
            "sync_battle",
            (
                session_id.to_string(),
                town_battle_id,
                "nonce:gate-m:town-battle:sync".to_string(),
            ),
        )?;
        Self::assert_applied(&town_sync)?;

        let final_view = self.game_view(caller, session_id, self.default_game_view_request())?;
        let finished =
            self.query_result::<SessionView>(caller, "get_session", (session_id.to_string(),))?;
        let west_champion = self.query_result::<ChampionView>(
            caller,
            "get_champion_view",
            (session_id.to_string(), west_champion_id),
        )?;
        let east_champion = self.query_result::<ChampionView>(
            player_two,
            "get_champion_view",
            (session_id.to_string(), east_champion_id),
        )?;
        let east_town = self.query_result::<ApiTownView>(
            caller,
            "get_town_view",
            (session_id.to_string(), "town:east".to_string()),
        )?;
        let west_history = self.get_match_history(caller, 0, 20)?;
        let final_events = self.get_events_after(session_id, "public", 0, 50);
        let final_storage = self.diagnostic_snapshot(GATE_M_ENTITIES);
        let winner_participant_id = self.player_one_participant_id.clone().filter(|winner| {
            west_history
                .entries
                .iter()
                .any(|entry| entry.session_id == session_id && entry.result == "win")
                && east_town.town.owner_participant_id == *winner
        });

        Ok(PlayableMatchView {
            session_id: session_id.to_string(),
            current_turn: final_view.session.current_turn,
            final_session_state: finished.state,
            winner_participant_id,
            champion_status: west_champion.status,
            captured_town_owner: east_town.town.owner_participant_id,
            defeated_neutral_state: if final_events
                .events
                .iter()
                .any(|event| event.event_type == "neutral_defeated")
            {
                "defeated".to_string()
            } else {
                "unknown".to_string()
            },
            defeated_champion_status: east_champion.status,
            match_summary_count: row_count(&final_storage, "PlayerMatchSummary"),
            match_history_count: west_history.entries.len() as u32,
            command_count: row_count(&final_storage, "GameCommand")
                .saturating_add(row_count(&final_storage, "LobbyCommand")),
            event_count: row_count(&final_storage, "GameEvent"),
            query_count: self.metrics.borrow().query_calls,
            max_query_bytes: self.metrics.borrow().max_response_bytes as u32,
            storage_row_count: final_storage.total_rows,
            recovery_retry_count: 0,
        })
    }

    fn diagnostic_snapshot(&self, entities: &[&str]) -> DiagnosticStorageSnapshot {
        let mut combined = DiagnosticStorageSnapshot {
            row_counts: Vec::new(),
            total_rows: 0,
            stable_memory_pages: 0,
        };

        for entity in entities {
            let snapshot = self
                .query_result::<DiagnosticStorageSnapshot>(
                    Principal::anonymous(),
                    "get_diagnostic_storage_snapshot",
                    (entity_names(&[*entity]),),
                )
                .expect("controller diagnostic storage snapshot should load");
            assert_eq!(snapshot.row_counts.len(), 1);
            combined.total_rows = combined.total_rows.saturating_add(snapshot.total_rows);
            combined.stable_memory_pages = combined
                .stable_memory_pages
                .max(snapshot.stable_memory_pages);
            combined.row_counts.extend(snapshot.row_counts);
        }

        combined
    }
}

impl WebClientBackend for CanisterWebClientBackend {
    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.update_lobby_response(
            caller,
            "register_player",
            (
                Some(display_name.to_lowercase().replace(' ', "-")),
                Some(display_name.to_string()),
                client_nonce.to_string(),
            ),
        )
        .expect("register_player should succeed through canister")
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        _scenario_seed: &str,
    ) -> LobbyCommandResponse {
        self.player_one = Some(caller);
        let response = self
            .update_lobby_response(
                caller,
                "create_session",
                (
                    "Gate M Web Client Match".to_string(),
                    FIRST_PLAYABLE_RULESET_ID.to_string(),
                    1_u64,
                    client_nonce.to_string(),
                ),
            )
            .expect("create_session should succeed through canister");
        if let LobbyCommandResult::Session(session) = &response.result {
            self.session_id = Some(session.session_id.clone());
        }
        response
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.player_two = Some(caller);
        self.update_lobby_response(
            caller,
            "join_session",
            (
                session_id.to_string(),
                "faction:ashen-ledger".to_string(),
                client_nonce.to_string(),
            ),
        )
        .expect("join_session should succeed through canister")
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        self.update_lobby_response(
            caller,
            "mark_ready",
            (session_id.to_string(), client_nonce.to_string()),
        )
        .expect("mark_ready should succeed through canister")
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        let mut response = self
            .update_lobby_response(
                caller,
                "start_session",
                (session_id.to_string(), client_nonce.to_string()),
            )
            .expect("start_session should succeed through canister");
        match &response.result {
            LobbyCommandResult::Session(session) => {
                assert!(
                    matches!(session.state.as_str(), "starting" | "active"),
                    "one-call start_session should return starting or active, got {}",
                    session.state
                );
            }
            other => panic!("start_session returned unexpected result: {other:?}"),
        }

        let mut last_state = String::new();
        let mut last_progress = None;
        for _ in 0..160 {
            let session = self
                .query_result::<SessionView>(caller, "get_session", (session_id.to_string(),))
                .expect("started session should load after start");
            if session.state == "active" {
                self.session_id = Some(session_id.to_string());
                let participant = self
                    .query_result::<ParticipantView>(
                        caller,
                        "get_my_participant",
                        (session_id.to_string(),),
                    )
                    .expect("west participant should load after start");
                self.player_one_participant_id = Some(participant.participant_id);
                response.result = LobbyCommandResult::Session(session);
                return response;
            }
            last_state = session.state;
            last_progress = self
                .query_result::<SetupProgressView>(
                    caller,
                    "get_setup_progress",
                    (session_id.to_string(),),
                )
                .ok();
            self.advance_time_for_timers_ms(5);
        }
        panic!(
            "one-call start_session timer continuation should activate session, last state {last_state}, progress {last_progress:?}"
        );
    }

    fn default_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<GameView, ProbeError> {
        let view = self.game_view(caller, session_id, self.default_game_view_request())?;
        if self.opening_game_view.is_none() && view.session.state == "active" {
            self.opening_game_view = Some(view.clone());
        }
        Ok(view)
    }

    fn game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        request: GameViewRequest,
    ) -> Result<GameView, ProbeError> {
        let session =
            self.query_result::<SessionView>(caller, "get_session", (session_id.to_string(),))?;
        let participant = self.query_result::<ParticipantView>(
            caller,
            "get_my_participant",
            (session_id.to_string(),),
        )?;
        let chunks = self.query_result::<MapChunkPage>(
            caller,
            "get_visible_map_chunks",
            (
                session_id.to_string(),
                request.viewport.clone(),
                request.chunk_cursor,
                request.chunk_limit,
            ),
        )?;
        let objects = self.query_result::<ObjectViewPage>(
            caller,
            "get_visible_objects",
            (
                session_id.to_string(),
                request.viewport.clone(),
                request.object_cursor,
                request.object_limit,
            ),
        )?;
        let champions = self.query_result::<Vec<ChampionView>>(
            caller,
            "get_my_champions",
            (session_id.to_string(),),
        )?;
        let owner_participant_id = participant.participant_id.clone();
        let towns =
            town_views_for_objects(self, caller, session_id, &owner_participant_id, &objects)?;
        let event_page = self.query_result::<ApiEventPage>(
            caller,
            "get_events_after",
            (
                session_id.to_string(),
                "public".to_string(),
                request.events_after_seq,
                request.event_limit,
            ),
        )?;
        self.observe_event_page(&event_page);

        let mut battle = None;
        let mut battle_summary = None;
        if request.include_battle
            && let Some(battle_id) = self.first_battle_id.clone()
        {
            let view = self.query_result::<BattleView>(
                caller,
                "get_battle_state",
                (session_id.to_string(), battle_id),
            )?;
            battle_summary = Some(BattleSummary::from(&view));
            battle = Some(view);
        }

        let view = GameView {
            session: domm_game::SessionSummary::from_session(session, self.current_turn.max(1)),
            participant: ParticipantSummary::from(participant),
            viewport: request.viewport,
            map_chunks: chunks.chunks,
            map_page_info: PageInfo {
                next_cursor: chunks.next_cursor,
                has_more: chunks.has_more,
                limit: request.chunk_limit,
            },
            objects: objects.objects,
            object_page_info: PageInfo {
                next_cursor: objects.next_cursor,
                has_more: objects.has_more,
                limit: request.object_limit,
            },
            champions,
            towns,
            battle,
            battle_summary,
            events: event_page.events,
            event_page_info: event_page.page_info,
            content_manifest_hash: first_playable_content_manifest().computed_content_hash(),
            render_time: RenderTimeMeta {
                server_now_ms: self.server_now_ms,
                turn_started_at_ms: u64::from(self.current_turn.saturating_sub(1))
                    .saturating_mul(domm_game::TURN_DURATION_MS),
                turn_duration_ms: domm_game::TURN_DURATION_MS,
                sync_required: false,
            },
            action_affordances: Vec::new(),
            omitted_fields: Vec::new(),
        };
        self.observe_game_view(&view);
        Ok(view)
    }

    fn submit_move_intent(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        self.advance_to_time_ms(now_ms);
        let champion_id = self
            .resolve_champion_id(caller, session_id, champion_id)
            .expect("semantic champion id should resolve through canister");
        self.update_command_response(
            caller,
            "submit_move_intent",
            (
                session_id.to_string(),
                champion_id,
                path,
                client_nonce.to_string(),
            ),
        )
        .expect("submit_move_intent should succeed through canister")
    }

    fn sync_session_turn(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        if self
            .advanced_time_nonces
            .insert(format!("sync_session_turn:{client_nonce}"))
        {
            self.advance_to_time_ms(now_ms.max(self.server_now_ms.saturating_add(61_000)));
        }
        let response = self
            .update_command_response(
                caller,
                "sync_session_turn",
                (session_id.to_string(), client_nonce.to_string()),
            )
            .expect("sync_session_turn should succeed through canister");
        self.capture_first_battle_id_from_public_events(caller, session_id)
            .expect("public events should be readable after sync_session_turn");
        response
    }

    fn preview_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        _turn_number: u32,
    ) -> Result<BuildPreview, ProbeError> {
        self.query_result::<BuildPreview>(
            caller,
            "preview_build_town_structure",
            (
                session_id.to_string(),
                town_id.to_string(),
                prefixed("building", building_slug),
            ),
        )
    }

    fn submit_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        _turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        self.update_command_response(
            caller,
            "submit_build_town_structure",
            (
                session_id.to_string(),
                town_id.to_string(),
                prefixed("building", building_slug),
                client_nonce.to_string(),
            ),
        )
        .expect("submit_build_town_structure should succeed through canister")
    }

    fn preview_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        _turn_number: u32,
    ) -> Result<RecruitPreview, ProbeError> {
        self.query_result::<RecruitPreview>(
            caller,
            "preview_recruit_units",
            (
                session_id.to_string(),
                town_id.to_string(),
                prefixed("unit", unit_slug),
                quantity,
                target.clone(),
            ),
        )
    }

    fn submit_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        _turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        self.update_command_response(
            caller,
            "submit_recruit_units",
            (
                session_id.to_string(),
                town_id.to_string(),
                prefixed("unit", unit_slug),
                quantity,
                target,
                client_nonce.to_string(),
            ),
        )
        .expect("submit_recruit_units should succeed through canister")
    }

    fn get_town_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
    ) -> Result<ApiTownView, ProbeError> {
        let view = self.query_result::<ApiTownView>(
            caller,
            "get_town_view",
            (session_id.to_string(), town_id.to_string()),
        )?;
        self.town_view_cache
            .insert(town_id.to_string(), view.clone());
        Ok(view)
    }

    fn get_battle_state(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<BattleView, ProbeError> {
        self.advance_to_time_ms(now_ms);
        self.include_battle_in_game_view = true;
        self.first_battle_id
            .get_or_insert_with(|| battle_id.to_string());
        let session_id = self.session_id.clone().ok_or(ProbeError::MissingSession)?;
        self.query_result::<BattleView>(
            caller,
            "get_battle_state",
            (session_id, battle_id.to_string()),
        )
    }

    fn submit_battle_action(
        &mut self,
        caller: Principal,
        input: BattleActionInput,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        self.advance_to_time_ms(now_ms);
        let session_id = self
            .session_id
            .clone()
            .expect("session id should be set before battle action");
        self.update_command_response(
            caller,
            "submit_battle_action",
            (session_id, input, client_nonce.to_string()),
        )
        .expect("submit_battle_action should succeed through canister")
    }

    fn sync_battle(
        &mut self,
        caller: Principal,
        battle_id: &str,
        _now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        self.advance_once_for_nonce_by_ms(
            "sync_battle",
            client_nonce,
            domm_game::BATTLE_ACTION_DEADLINE_MS + 1,
        );
        let session_id = self
            .session_id
            .clone()
            .expect("session id should be set before battle sync");
        self.update_command_response(
            caller,
            "sync_battle",
            (session_id, battle_id.to_string(), client_nonce.to_string()),
        )
        .expect("sync_battle should succeed through canister")
    }

    fn get_content_manifest(
        &self,
        ruleset_slug: &str,
        version: u32,
    ) -> Result<ContentManifestResponse, ProbeError> {
        self.query_result::<ContentManifestResponse>(
            Principal::anonymous(),
            "get_content_manifest",
            (ruleset_slug.to_string(), version),
        )
    }

    fn get_command_status(&self, command_id_or_client_nonce: &str) -> Option<CommandStatusView> {
        let session_id = self.session_id.as_ref()?;
        let caller = self.player_one_or_anonymous();
        self.query_result::<CommandStatusView>(
            caller,
            "get_command_status",
            (session_id.clone(), command_id_or_client_nonce.to_string()),
        )
        .ok()
    }

    fn get_events_after(
        &self,
        session_id: &str,
        audience_key: &str,
        after_seq: u64,
        limit: u32,
    ) -> ApiEventPage {
        let page = self
            .query_result::<ApiEventPage>(
                self.player_one_or_anonymous(),
                "get_events_after",
                (
                    session_id.to_string(),
                    audience_key.to_string(),
                    after_seq,
                    limit,
                ),
            )
            .expect("get_events_after should succeed through canister");
        self.observe_event_page(&page);
        page
    }

    fn get_match_history(
        &self,
        caller: Principal,
        cursor: u32,
        limit: u32,
    ) -> Result<MatchHistoryPage, ProbeError> {
        self.query_result::<MatchHistoryPage>(caller, "get_match_history", (cursor, limit))
    }

    fn start_first_playable_session(&mut self) -> SessionView {
        let fixture = first_playable_fixture();
        self.register_player(
            fixture.principals.player_one,
            "Misery One",
            &fixture.command_nonces.register_player_one,
        );
        self.register_player(
            fixture.principals.player_two,
            "Mayhem Two",
            &fixture.command_nonces.register_player_two,
        );
        let created = self.create_session(
            fixture.principals.player_one,
            &fixture.command_nonces.create_session,
            &fixture.scenario_seed,
        );
        let session_id = match created.result {
            LobbyCommandResult::Session(session) => session.session_id,
            _ => fixture.ids.session_id,
        };
        self.join_session(
            fixture.principals.player_two,
            &session_id,
            &fixture.command_nonces.join_session,
        );
        self.mark_ready(
            fixture.principals.player_one,
            &session_id,
            &fixture.command_nonces.mark_ready_player_one,
        );
        self.mark_ready(
            fixture.principals.player_two,
            &session_id,
            &fixture.command_nonces.mark_ready_player_two,
        );
        let started = self.start_session(
            fixture.principals.player_one,
            &session_id,
            &fixture.command_nonces.start_session,
        );
        match started.result {
            LobbyCommandResult::Session(session) => session,
            _ => self
                .query_result::<SessionView>(
                    fixture.principals.player_one,
                    "get_session",
                    (session_id,),
                )
                .expect("started session should load from canister"),
        }
    }

    fn first_fixture_battle_id(&self) -> String {
        self.first_battle_id
            .clone()
            .expect("neutral battle id should be captured from canister events")
    }

    fn has_first_fixture_battle_id(&self) -> bool {
        self.first_battle_id.is_some()
    }

    fn finish_first_playable_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<PlayableMatchView, ProbeError> {
        self.finish_canister_match(caller, session_id)
    }

    fn metrics(&self) -> ApiMetrics {
        let metrics = self.metrics.borrow();
        ApiMetrics {
            command_response_count: metrics.command_response_count,
            lobby_response_count: metrics.lobby_response_count,
            api_event_count: metrics.observed_event_count,
            strategic_command_count: metrics.update_calls,
            strategic_event_count: metrics.total_response_bytes as u32,
            strategic_query_count: metrics.query_calls,
        }
    }
}

fn town_views_for_objects(
    backend: &mut CanisterWebClientBackend,
    caller: Principal,
    session_id: &str,
    owner_participant_id: &str,
    objects: &ObjectViewPage,
) -> Result<Vec<ApiTownView>, ProbeError> {
    let mut towns = Vec::new();
    let mut seen_subject_ids = BTreeSet::new();
    for object in objects
        .objects
        .iter()
        .filter(|object| object.subject_kind == "town" && object.visibility == "visible")
    {
        if object.owner_participant_id.as_deref() != Some(owner_participant_id) {
            continue;
        }
        if !seen_subject_ids.insert(object.subject_id_text.clone()) {
            continue;
        }
        if let Some(town) = backend
            .town_view_cache
            .get(&object.subject_id_text)
            .cloned()
        {
            towns.push(town);
        } else {
            towns.push(backend.get_town_view(caller, session_id, &object.subject_id_text)?);
        }
    }
    Ok(towns)
}

#[derive(Clone, Debug)]
struct BenchmarkStatusSample {
    cycles: u128,
    memory_size_bytes: u128,
    wasm_memory_size_bytes: u128,
    stable_memory_size_bytes: u128,
    query_calls_total: u128,
    query_instructions_total: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkCallRecord {
    sequence: u64,
    scenario_id: String,
    method: String,
    kind: String,
    ok: bool,
    error_code: Option<String>,
    response_bytes: u64,
    wall_time_micros: u64,
    instruction_delta: Option<u64>,
    cycle_cost: u128,
    memory_delta_bytes: i128,
    wasm_memory_delta_bytes: i128,
    stable_memory_delta_bytes: i128,
    memory_size_before_bytes: u128,
    memory_size_after_bytes: u128,
    repo_ops: Vec<BenchmarkRepoOpRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkRepoOpRecord {
    operation: String,
    calls: u64,
    instruction_delta: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkScenarioSummary {
    id: String,
    label: String,
    description: String,
    players: u32,
    call_count: u64,
    query_count: u64,
    update_count: u64,
    response_bytes_total: u64,
    instruction_total: u128,
    instruction_change_pct: Option<f64>,
    cycle_cost_total: u128,
    memory_delta_bytes: i128,
    memory_change_pct: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkMethodSummary {
    method: String,
    kind: String,
    call_count: u64,
    error_count: u64,
    avg_instruction_delta: Option<f64>,
    p95_instruction_delta: Option<u64>,
    instruction_change_pct: Option<f64>,
    avg_memory_delta_bytes: f64,
    memory_change_pct: Option<f64>,
    avg_cycle_cost: f64,
    avg_response_bytes: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkPhaseSummary {
    method: String,
    phase: String,
    call_count: u64,
    avg_instruction_delta: f64,
    total_instruction_delta: u128,
    avg_stable_page_delta: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkRepoOpSummary {
    operation: String,
    call_count: u64,
    avg_instruction_delta: f64,
    total_instruction_delta: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BenchmarkSummaryArtifact {
    run_id: String,
    git_sha: String,
    generated_at_ms: u64,
    benchmark_name: String,
    scenario_count: usize,
    call_count: usize,
    required_game_endpoint_count: usize,
    covered_required_endpoint_count: usize,
    missing_required_endpoints: Vec<String>,
    total_row_growth: u32,
    stable_memory_pages_start: u64,
    stable_memory_pages_final: u64,
    scenarios: Vec<BenchmarkScenarioSummary>,
    methods: Vec<BenchmarkMethodSummary>,
    phases: Vec<BenchmarkPhaseSummary>,
    repo_ops: Vec<BenchmarkRepoOpSummary>,
}

#[derive(Clone, Debug, Serialize)]
struct BenchmarkRunArtifact {
    summary: BenchmarkSummaryArtifact,
    calls: Vec<BenchmarkCallRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BenchmarkQueryLog {
    method: String,
    instruction_delta: u64,
}

#[derive(Clone, Debug)]
struct BenchmarkCanisterCallMeasurement {
    method: String,
    kind: String,
    ok: bool,
    error_code: Option<String>,
    instruction_delta: u64,
    stable_memory_pages_before: u64,
    stable_memory_pages_after: u64,
    repo_ops: Vec<BenchmarkRepoOpRecord>,
}

#[derive(Clone, Debug)]
struct BenchmarkRecorder {
    output_dir: PathBuf,
    calls: Vec<BenchmarkCallRecord>,
    next_sequence: u64,
    canister_call_cursor: u64,
    pending_canister_calls: VecDeque<BenchmarkCanisterCallMeasurement>,
    canister_log_cursor: u64,
    query_log_path: Option<PathBuf>,
    query_log_entry_cursor: usize,
}

impl BenchmarkRecorder {
    fn from_env() -> Option<Self> {
        let output_dir = env::var_os("DOMM_BENCH_OUTPUT_DIR").map(PathBuf::from)?;
        let query_log_path = env::var_os("DOMM_BENCH_QUERY_LOG_PATH").map(PathBuf::from);
        Some(Self {
            output_dir,
            calls: Vec::new(),
            next_sequence: 1,
            canister_call_cursor: 0,
            pending_canister_calls: VecDeque::new(),
            canister_log_cursor: 0,
            query_log_path,
            query_log_entry_cursor: 0,
        })
    }

    fn capture_status(fixture: &StandaloneCanisterFixture) -> BenchmarkStatusSample {
        let status = fixture
            .pic()
            .canister_status(fixture.canister_id(), None)
            .expect("benchmark canister_status should load");
        BenchmarkStatusSample {
            cycles: nat_to_u128(&status.cycles),
            memory_size_bytes: nat_to_u128(&status.memory_size),
            wasm_memory_size_bytes: nat_to_u128(&status.memory_metrics.wasm_memory_size),
            stable_memory_size_bytes: nat_to_u128(&status.memory_metrics.stable_memory_size),
            query_calls_total: nat_to_u128(&status.query_stats.num_calls_total),
            query_instructions_total: nat_to_u128(&status.query_stats.num_instructions_total),
        }
    }

    fn reset_canister_metrics(&mut self, fixture: &StandaloneCanisterFixture) {
        let response: Result<(), ApiError> = fixture
            .pic()
            .update_call_as(
                fixture.canister_id(),
                Principal::anonymous(),
                "reset_diagnostic_benchmark_metrics",
                (),
            )
            .expect("benchmark reset should decode");
        response.expect("benchmark reset should succeed");
        self.canister_call_cursor = 0;
        self.pending_canister_calls.clear();
        self.canister_log_cursor = 0;
        self.query_log_entry_cursor = 0;
    }

    fn fetch_canister_measurements(&mut self, fixture: &StandaloneCanisterFixture) {
        loop {
            let page: Result<DiagnosticBenchmarkCallPage, ApiError> =
                match fixture.pic().query_call_as(
                    fixture.canister_id(),
                    Principal::anonymous(),
                    "get_diagnostic_benchmark_metrics",
                    (Some(self.canister_call_cursor), 1024_u32),
                ) {
                    Ok(page) => page,
                    Err(_) => return,
                };
            let Ok(page) = page else {
                return;
            };
            let Some(last) = page.calls.last() else {
                return;
            };
            self.canister_call_cursor = last.sequence;

            for call in page.calls {
                self.pending_canister_calls
                    .push_back(BenchmarkCanisterCallMeasurement {
                        method: call.method,
                        kind: call.kind,
                        ok: call.ok,
                        error_code: call.error_code,
                        instruction_delta: call.instruction_delta,
                        stable_memory_pages_before: call.stable_memory_pages_before,
                        stable_memory_pages_after: call.stable_memory_pages_after,
                        repo_ops: call
                            .repo_ops
                            .into_iter()
                            .map(|op| BenchmarkRepoOpRecord {
                                operation: op.operation,
                                calls: op.calls,
                                instruction_delta: op.instruction_delta,
                            })
                            .collect(),
                    });
            }

            if page.next_cursor.is_none() {
                return;
            }
        }
    }

    fn pull_update_measurement(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        method: &str,
    ) -> Option<BenchmarkCanisterCallMeasurement> {
        self.fetch_canister_measurements(fixture);
        let position = self
            .pending_canister_calls
            .iter()
            .rposition(|call| call.method == method && call.kind == "update")?;
        self.pending_canister_calls.remove(position)
    }

    fn record_timer_measurements(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        scenario_id: &str,
    ) {
        self.fetch_canister_measurements(fixture);
        let mut index = 0;
        while index < self.pending_canister_calls.len() {
            if self.pending_canister_calls[index].kind != "timer" {
                index += 1;
                continue;
            }
            let measurement = self
                .pending_canister_calls
                .remove(index)
                .expect("pending timer measurement should exist");
            self.record_timer_measurement(scenario_id, measurement);
        }
    }

    fn record_timer_measurement(
        &mut self,
        scenario_id: &str,
        measurement: BenchmarkCanisterCallMeasurement,
    ) {
        let stable_before_bytes =
            u128::from(measurement.stable_memory_pages_before).saturating_mul(65_536);
        let stable_after_bytes =
            u128::from(measurement.stable_memory_pages_after).saturating_mul(65_536);
        self.calls.push(BenchmarkCallRecord {
            sequence: self.next_sequence,
            scenario_id: scenario_id.to_string(),
            method: measurement.method,
            kind: "timer".to_string(),
            ok: measurement.ok,
            error_code: measurement.error_code,
            response_bytes: 0,
            wall_time_micros: 0,
            instruction_delta: Some(measurement.instruction_delta),
            cycle_cost: 0,
            memory_delta_bytes: unsigned_delta(stable_after_bytes, stable_before_bytes),
            wasm_memory_delta_bytes: 0,
            stable_memory_delta_bytes: unsigned_delta(stable_after_bytes, stable_before_bytes),
            memory_size_before_bytes: stable_before_bytes,
            memory_size_after_bytes: stable_after_bytes,
            repo_ops: measurement.repo_ops,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
    }

    fn pull_query_instruction(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        method: &str,
    ) -> Option<u64> {
        if let Some(instruction_delta) = self.pull_query_instruction_from_log_file(method) {
            return Some(instruction_delta);
        }

        let records = fixture
            .pic()
            .fetch_canister_logs(fixture.canister_id(), Principal::anonymous())
            .ok()?;
        let mut matched = None;
        for record in &records {
            if record.idx <= self.canister_log_cursor {
                continue;
            }
            self.canister_log_cursor = self.canister_log_cursor.max(record.idx);
            if let Some(log) = parse_benchmark_query_log(&record.content)
                && log.method == method
            {
                matched = Some(log.instruction_delta);
            }
        }
        matched
    }

    fn pull_query_instruction_from_log_file(&mut self, method: &str) -> Option<u64> {
        let path = self.query_log_path.as_ref()?;
        let started = Instant::now();

        loop {
            if let Ok(text) = fs::read_to_string(path) {
                let logs = text
                    .lines()
                    .filter_map(|line| parse_benchmark_query_log(line.as_bytes()))
                    .collect::<Vec<_>>();
                while self.query_log_entry_cursor < logs.len() {
                    let log = &logs[self.query_log_entry_cursor];
                    self.query_log_entry_cursor = self.query_log_entry_cursor.saturating_add(1);
                    if log.method == method {
                        return Some(log.instruction_delta);
                    }
                }
            }

            if started.elapsed() >= Duration::from_secs(2) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn record_call<T: CandidType>(
        &mut self,
        scenario_id: &str,
        method: &str,
        kind: &str,
        response: &Result<T, ApiError>,
        elapsed: Duration,
        before: &BenchmarkStatusSample,
        after: &BenchmarkStatusSample,
        measured_instruction_delta: Option<u64>,
        repo_ops: Vec<BenchmarkRepoOpRecord>,
    ) {
        let instruction_delta = if measured_instruction_delta.is_some() {
            measured_instruction_delta
        } else if kind == "query" {
            let query_count_delta = after
                .query_calls_total
                .saturating_sub(before.query_calls_total);
            let query_instruction_delta = after
                .query_instructions_total
                .saturating_sub(before.query_instructions_total);
            if query_count_delta == 1 && query_instruction_delta > 0 {
                Some(u128_to_u64_saturating(query_instruction_delta))
            } else {
                None
            }
        } else {
            None
        };
        let cycle_cost = before.cycles.saturating_sub(after.cycles);
        let response_bytes = candid::encode_one(response)
            .unwrap_or_else(|error| panic!("{method} response should Candid encode: {error}"))
            .len();

        self.calls.push(BenchmarkCallRecord {
            sequence: self.next_sequence,
            scenario_id: scenario_id.to_string(),
            method: method.to_string(),
            kind: kind.to_string(),
            ok: response.is_ok(),
            error_code: response.as_ref().err().map(|error| error.code.clone()),
            response_bytes: u64::try_from(response_bytes).unwrap_or(u64::MAX),
            wall_time_micros: u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
            instruction_delta,
            cycle_cost,
            memory_delta_bytes: unsigned_delta(after.memory_size_bytes, before.memory_size_bytes),
            wasm_memory_delta_bytes: unsigned_delta(
                after.wasm_memory_size_bytes,
                before.wasm_memory_size_bytes,
            ),
            stable_memory_delta_bytes: unsigned_delta(
                after.stable_memory_size_bytes,
                before.stable_memory_size_bytes,
            ),
            memory_size_before_bytes: before.memory_size_bytes,
            memory_size_after_bytes: after.memory_size_bytes,
            repo_ops,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
    }

    fn write_artifacts(
        &self,
        benchmark_name: &str,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
    ) {
        if self.calls.is_empty() {
            return;
        }

        fs::create_dir_all(&self.output_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create benchmark output dir {}: {error}",
                self.output_dir.display()
            )
        });

        let summary = self.build_summary(benchmark_name, initial_storage, final_storage);
        let run = BenchmarkRunArtifact {
            summary: summary.clone(),
            calls: self.calls.clone(),
        };

        write_json_file(&self.output_dir.join("run.json"), &run);
        write_json_file(&self.output_dir.join("summary.json"), &summary);
        fs::write(
            self.output_dir.join("summary.md"),
            render_summary_markdown(&summary),
        )
        .unwrap_or_else(|error| panic!("failed to write benchmark summary.md: {error}"));
    }

    fn build_summary(
        &self,
        benchmark_name: &str,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
    ) -> BenchmarkSummaryArtifact {
        let scenarios = scenario_summaries(&self.calls);
        let methods = method_summaries(&self.calls);
        let phases = Vec::new();
        let repo_ops = repo_op_summaries(&self.calls);

        let covered_required = covered_required_endpoints(&self.calls);
        let missing_required_endpoints = REQUIRED_GAME_ENDPOINTS
            .iter()
            .filter(|endpoint| !covered_required.contains(endpoint.name))
            .map(|endpoint| endpoint.name.to_string())
            .collect::<Vec<_>>();

        BenchmarkSummaryArtifact {
            run_id: self
                .output_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("benchmark-run")
                .to_string(),
            git_sha: git_sha(),
            generated_at_ms: now_millis(),
            benchmark_name: benchmark_name.to_string(),
            scenario_count: scenarios.len(),
            call_count: self.calls.len(),
            required_game_endpoint_count: REQUIRED_GAME_ENDPOINTS.len(),
            covered_required_endpoint_count: covered_required.len(),
            missing_required_endpoints,
            total_row_growth: final_storage
                .total_rows
                .saturating_sub(initial_storage.total_rows),
            stable_memory_pages_start: initial_storage.stable_memory_pages,
            stable_memory_pages_final: final_storage.stable_memory_pages,
            scenarios,
            methods,
            phases,
            repo_ops,
        }
    }
}

#[derive(Clone, Debug)]
struct CanisterProbeMetrics {
    update_calls: u32,
    query_calls: u32,
    lobby_response_count: u32,
    command_response_count: u32,
    observed_event_count: u32,
    total_response_bytes: usize,
    max_response_bytes: usize,
    max_response_method: String,
    benchmark: Option<BenchmarkRecorder>,
    benchmark_scenario_id: String,
    last_benchmark_scenario_id: String,
}

impl Default for CanisterProbeMetrics {
    fn default() -> Self {
        Self {
            update_calls: 0,
            query_calls: 0,
            lobby_response_count: 0,
            command_response_count: 0,
            observed_event_count: 0,
            total_response_bytes: 0,
            max_response_bytes: 0,
            max_response_method: String::new(),
            benchmark: BenchmarkRecorder::from_env(),
            benchmark_scenario_id: "diagnostics".to_string(),
            last_benchmark_scenario_id: "diagnostics".to_string(),
        }
    }
}

impl CanisterProbeMetrics {
    fn set_benchmark_scenario(&mut self, scenario_id: &str) {
        self.benchmark_scenario_id = scenario_id.to_string();
        if scenario_id != "diagnostics" {
            self.last_benchmark_scenario_id = scenario_id.to_string();
        }
    }

    fn reset_benchmark_canister(&mut self, fixture: &StandaloneCanisterFixture) {
        if let Some(benchmark) = &mut self.benchmark {
            benchmark.reset_canister_metrics(fixture);
        }
    }

    fn benchmark_sample(
        &self,
        fixture: &StandaloneCanisterFixture,
    ) -> Option<BenchmarkStatusSample> {
        self.benchmark
            .as_ref()
            .map(|_| BenchmarkRecorder::capture_status(fixture))
    }

    fn pull_update_measurement(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        method: &str,
    ) -> Option<BenchmarkCanisterCallMeasurement> {
        self.benchmark
            .as_mut()
            .and_then(|benchmark| benchmark.pull_update_measurement(fixture, method))
    }

    fn pull_query_instruction(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        method: &str,
    ) -> Option<u64> {
        self.benchmark
            .as_mut()
            .and_then(|benchmark| benchmark.pull_query_instruction(fixture, method))
    }

    fn record_benchmark_call<T: CandidType>(
        &mut self,
        method: &str,
        kind: &str,
        response: &Result<T, ApiError>,
        elapsed: Duration,
        before: Option<BenchmarkStatusSample>,
        after: Option<BenchmarkStatusSample>,
        measured_instruction_delta: Option<u64>,
        repo_ops: Vec<BenchmarkRepoOpRecord>,
    ) {
        let Some(benchmark) = &mut self.benchmark else {
            return;
        };
        let (Some(before), Some(after)) = (before, after) else {
            return;
        };
        let scenario_id = self.benchmark_scenario_id.clone();
        benchmark.record_call(
            &scenario_id,
            method,
            kind,
            response,
            elapsed,
            &before,
            &after,
            measured_instruction_delta,
            repo_ops,
        );
    }

    fn write_benchmark_artifacts(
        &mut self,
        fixture: &StandaloneCanisterFixture,
        benchmark_name: &str,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
    ) {
        if let Some(benchmark) = &mut self.benchmark {
            let scenario_id = if self.benchmark_scenario_id == "diagnostics" {
                self.last_benchmark_scenario_id.as_str()
            } else {
                self.benchmark_scenario_id.as_str()
            };
            benchmark.record_timer_measurements(fixture, scenario_id);
            benchmark.write_artifacts(benchmark_name, initial_storage, final_storage);
        }
    }

    fn record_query<T: CandidType>(&mut self, method: &str, response: &Result<T, ApiError>) {
        self.query_calls = self.query_calls.saturating_add(1);
        self.record_response(method, response);
    }

    fn record_update<T: CandidType>(&mut self, method: &str, response: &Result<T, ApiError>) {
        self.update_calls = self.update_calls.saturating_add(1);
        self.record_response(method, response);
    }

    fn record_response<T: CandidType>(&mut self, method: &str, response: &Result<T, ApiError>) {
        let byte_len = candid::encode_one(response)
            .unwrap_or_else(|error| panic!("{method} response should Candid encode: {error}"))
            .len();
        self.total_response_bytes = self.total_response_bytes.saturating_add(byte_len);
        if byte_len > self.max_response_bytes {
            self.max_response_bytes = byte_len;
            self.max_response_method = method.to_string();
        }
    }

    fn observe_lobby_response(&mut self, response: &LobbyCommandResponse) {
        self.lobby_response_count = self.lobby_response_count.saturating_add(1);
        self.observe_event_count(response.events.len() as u32);
    }

    fn observe_command_response(&mut self, response: &CommandResponse) {
        self.command_response_count = self.command_response_count.saturating_add(1);
        self.observe_event_count(response.events.len() as u32);
    }

    fn observe_event_page(&mut self, page: &ApiEventPage) {
        self.observe_event_count(page.events.len() as u32);
    }

    fn observe_event_count(&mut self, count: u32) {
        self.observed_event_count = self.observed_event_count.saturating_add(count);
    }
}

fn nat_to_u128(value: &candid::Nat) -> u128 {
    value.0.to_string().parse::<u128>().unwrap_or(u128::MAX)
}

fn u128_to_u64_saturating(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn unsigned_delta(after: u128, before: u128) -> i128 {
    if after >= before {
        i128::try_from(after - before).unwrap_or(i128::MAX)
    } else {
        -i128::try_from(before - after).unwrap_or(i128::MAX)
    }
}

fn parse_benchmark_query_log(content: &[u8]) -> Option<BenchmarkQueryLog> {
    let text = String::from_utf8_lossy(content);
    if !text.contains("DOMM_BENCH_QUERY") {
        return None;
    }

    let fields = text
        .split_whitespace()
        .filter_map(|token| token.split_once('='))
        .collect::<BTreeMap<_, _>>();

    Some(BenchmarkQueryLog {
        method: fields.get("method")?.to_string(),
        instruction_delta: fields.get("instruction_delta")?.parse().ok()?,
    })
}

fn scenario_summaries(calls: &[BenchmarkCallRecord]) -> Vec<BenchmarkScenarioSummary> {
    let mut grouped: BTreeMap<String, Vec<&BenchmarkCallRecord>> = BTreeMap::new();
    for call in calls
        .iter()
        .filter(|call| !is_internal_benchmark_method(&call.method))
    {
        grouped
            .entry(call.scenario_id.clone())
            .or_default()
            .push(call);
    }

    grouped
        .into_iter()
        .filter(|(id, _)| id != "diagnostics")
        .map(|(id, calls)| {
            let first = calls.first().expect("scenario group should not be empty");
            let last = calls.last().expect("scenario group should not be empty");
            BenchmarkScenarioSummary {
                label: scenario_label(&id).to_string(),
                description: scenario_description(&id, &calls),
                players: 2,
                call_count: u64::try_from(calls.len()).unwrap_or(u64::MAX),
                query_count: count_kind(&calls, "query"),
                update_count: count_kind(&calls, "update"),
                response_bytes_total: calls.iter().map(|call| call.response_bytes).sum(),
                instruction_total: calls
                    .iter()
                    .filter_map(|call| call.instruction_delta)
                    .map(u128::from)
                    .sum(),
                instruction_change_pct: None,
                cycle_cost_total: calls.iter().map(|call| call.cycle_cost).sum(),
                memory_delta_bytes: unsigned_delta(
                    last.memory_size_after_bytes,
                    first.memory_size_before_bytes,
                ),
                memory_change_pct: None,
                id,
            }
        })
        .collect()
}

fn method_summaries(calls: &[BenchmarkCallRecord]) -> Vec<BenchmarkMethodSummary> {
    let mut grouped: BTreeMap<(String, String), Vec<&BenchmarkCallRecord>> = BTreeMap::new();
    for call in calls
        .iter()
        .filter(|call| !is_internal_benchmark_method(&call.method))
    {
        grouped
            .entry((call.method.clone(), call.kind.clone()))
            .or_default()
            .push(call);
    }

    grouped
        .into_iter()
        .map(|((method, kind), calls)| {
            let instruction_values = calls
                .iter()
                .filter_map(|call| call.instruction_delta)
                .collect::<Vec<_>>();
            BenchmarkMethodSummary {
                method,
                kind,
                call_count: u64::try_from(calls.len()).unwrap_or(u64::MAX),
                error_count: u64::try_from(calls.iter().filter(|call| !call.ok).count())
                    .unwrap_or(u64::MAX),
                avg_instruction_delta: average_u64(&instruction_values),
                p95_instruction_delta: percentile_u64(instruction_values, 95),
                instruction_change_pct: None,
                avg_memory_delta_bytes: average_i128(
                    &calls
                        .iter()
                        .map(|call| call.memory_delta_bytes)
                        .collect::<Vec<_>>(),
                ),
                memory_change_pct: None,
                avg_cycle_cost: average_u128(
                    &calls.iter().map(|call| call.cycle_cost).collect::<Vec<_>>(),
                ),
                avg_response_bytes: average_u64(
                    &calls
                        .iter()
                        .map(|call| call.response_bytes)
                        .collect::<Vec<_>>(),
                )
                .unwrap_or(0.0),
            }
        })
        .collect()
}

fn repo_op_summaries(calls: &[BenchmarkCallRecord]) -> Vec<BenchmarkRepoOpSummary> {
    let mut grouped: BTreeMap<String, Vec<&BenchmarkRepoOpRecord>> = BTreeMap::new();
    for call in calls
        .iter()
        .filter(|call| !is_internal_benchmark_method(&call.method))
    {
        for op in &call.repo_ops {
            grouped.entry(op.operation.clone()).or_default().push(op);
        }
    }

    grouped
        .into_iter()
        .map(|(operation, ops)| {
            let call_count = ops.iter().map(|op| op.calls).sum::<u64>();
            let total_instruction_delta = ops
                .iter()
                .map(|op| u128::from(op.instruction_delta))
                .sum::<u128>();
            BenchmarkRepoOpSummary {
                operation,
                call_count,
                avg_instruction_delta: if call_count == 0 {
                    0.0
                } else {
                    total_instruction_delta as f64 / call_count as f64
                },
                total_instruction_delta,
            }
        })
        .collect()
}

fn covered_required_endpoints(calls: &[BenchmarkCallRecord]) -> BTreeSet<&'static str> {
    REQUIRED_GAME_ENDPOINTS
        .iter()
        .filter(|endpoint| {
            calls.iter().any(|call| {
                call.method == endpoint.name
                    && matches!(
                        (&call.kind[..], endpoint.kind),
                        ("query", EndpointKind::Query) | ("update", EndpointKind::Update)
                    )
            })
        })
        .map(|endpoint| endpoint.name)
        .collect()
}

fn is_internal_benchmark_method(method: &str) -> bool {
    method.starts_with("get_diagnostic_")
        || method.starts_with("reset_diagnostic_")
        || method.starts_with("icydb_")
}

fn count_kind(calls: &[&BenchmarkCallRecord], kind: &str) -> u64 {
    u64::try_from(calls.iter().filter(|call| call.kind == kind).count()).unwrap_or(u64::MAX)
}

fn count_method(calls: &[&BenchmarkCallRecord], method: &str) -> u64 {
    u64::try_from(calls.iter().filter(|call| call.method == method).count()).unwrap_or(u64::MAX)
}

fn average_u64(values: &[u64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64)
}

fn average_u128(values: &[u128]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64
}

fn average_i128(values: &[i128]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64
}

fn percentile_u64(mut values: Vec<u64>, percentile: u32) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let index = (values.len().saturating_sub(1) * usize::try_from(percentile).unwrap_or(100)) / 100;
    values.get(index).copied()
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) {
    let json = serde_json::to_vec_pretty(value).expect("benchmark artifact should serialize");
    fs::write(path, json).unwrap_or_else(|error| {
        panic!(
            "failed to write benchmark artifact {}: {error}",
            path.display()
        )
    });
}

fn render_summary_markdown(summary: &BenchmarkSummaryArtifact) -> String {
    let mut out = String::new();
    out.push_str(&format!("# DoMM Benchmark {}\n\n", summary.run_id));
    out.push_str(&format!(
        "- Git: `{}`\n- Benchmark: `{}`\n- Calls: {}\n- Required endpoints covered: {}/{}\n- Row growth: {}\n- Stable memory pages: {} -> {}\n\n",
        summary.git_sha,
        summary.benchmark_name,
        summary.call_count,
        summary.covered_required_endpoint_count,
        summary.required_game_endpoint_count,
        summary.total_row_growth,
        summary.stable_memory_pages_start,
        summary.stable_memory_pages_final
    ));
    out.push_str("## Scenarios\n\n");
    out.push_str("| Scenario | Calls | Memory MB | Memory change | Instructions B | Instruction change | Cycles T |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for scenario in &summary.scenarios {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            scenario_summary_label(scenario),
            scenario.call_count,
            format_mb_from_bytes_i128(scenario.memory_delta_bytes),
            format_pct(scenario.memory_change_pct),
            format_billions_u128(scenario.instruction_total),
            format_pct(scenario.instruction_change_pct),
            format_trillions_u128(scenario.cycle_cost_total)
        ));
    }
    render_method_summary_table(
        &mut out,
        "Public Methods",
        summary
            .methods
            .iter()
            .filter(|method| method.kind != "timer"),
    );
    render_method_summary_table(
        &mut out,
        "System Jobs / Timers",
        summary
            .methods
            .iter()
            .filter(|method| method.kind == "timer"),
    );
    if !summary.phases.is_empty() {
        out.push_str("\n## Endpoint Phases\n\n");
        out.push_str("| Method | Phase | Calls | Avg inst B | Total inst B | Avg stable pages |\n");
        out.push_str("| --- | --- | ---: | ---: | ---: | ---: |\n");
        for phase in &summary.phases {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {:.2} |\n",
                phase.method,
                phase.phase,
                phase.call_count,
                format_billions_f64(phase.avg_instruction_delta),
                format_billions_u128(phase.total_instruction_delta),
                phase.avg_stable_page_delta
            ));
        }
    }
    if !summary.repo_ops.is_empty() {
        out.push_str("\n## Repo Operations\n\n");
        out.push_str("| Operation | Calls | Avg inst B | Total inst B |\n");
        out.push_str("| --- | ---: | ---: | ---: |\n");
        for op in &summary.repo_ops {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                op.operation,
                op.call_count,
                format_billions_f64(op.avg_instruction_delta),
                format_billions_u128(op.total_instruction_delta)
            ));
        }
    }
    if !summary.missing_required_endpoints.is_empty() {
        out.push_str("\n## Missing Required Endpoints\n\n");
        out.push_str(&summary.missing_required_endpoints.join(", "));
        out.push('\n');
    }
    out
}

fn render_method_summary_table<'a>(
    out: &mut String,
    title: &str,
    methods: impl Iterator<Item = &'a BenchmarkMethodSummary>,
) {
    let methods = methods.collect::<Vec<_>>();
    if methods.is_empty() {
        return;
    }
    out.push_str(&format!("\n## {title}\n\n"));
    out.push_str("| Method | Kind | Calls | Avg inst B | Inst change | Avg mem MB | Mem change | Avg cycles T | Avg bytes | Errors |\n");
    out.push_str("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for method in methods {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {} |\n",
            method.method,
            method.kind,
            method.call_count,
            format_billions_option(method.avg_instruction_delta),
            format_pct(method.instruction_change_pct),
            format_mb_from_bytes_f64(method.avg_memory_delta_bytes),
            format_pct(method.memory_change_pct),
            format_trillions_f64(method.avg_cycle_cost),
            method.avg_response_bytes,
            method.error_count
        ));
    }
}

fn scenario_summary_label(scenario: &BenchmarkScenarioSummary) -> String {
    if scenario.description.is_empty() {
        scenario.label.clone()
    } else {
        format!("{}: {}", scenario.label, scenario.description)
    }
}

fn scenario_label(id: &str) -> &str {
    match id {
        "gate_m_web_client_probe" => "Gate M web client probe",
        _ => id,
    }
}

fn scenario_description(id: &str, calls: &[&BenchmarkCallRecord]) -> String {
    let movement_paths = count_method(calls, "submit_move_intent");
    let battle_actions = count_method(calls, "submit_battle_action");
    let battle_syncs = count_method(calls, "sync_battle");
    let battle_reads = count_method(calls, "get_battle_state");
    let history_reads = count_method(calls, "get_match_history");
    let event_reads = count_method(calls, "get_events_after");
    let town_reads = count_method(calls, "get_town_view");
    let game_view_reads = count_method(calls, "get_game_view");
    let battle_activity = battle_actions + battle_syncs + battle_reads;

    let mut facts = vec![
        "2 players".to_string(),
        format!(
            "map {}x{}",
            FIRST_PLAYABLE_MAP_WIDTH, FIRST_PLAYABLE_MAP_HEIGHT
        ),
    ];
    if battle_activity > 0 {
        facts.push(format!(
            "battle grid {}x{}",
            BATTLE_GRID_WIDTH, BATTLE_GRID_HEIGHT
        ));
    }
    facts.push(count_label(movement_paths, "movement path"));
    facts.push(count_label(battle_actions, "battle action"));
    if battle_syncs > 0 {
        facts.push(count_label(battle_syncs, "battle sync"));
    }
    if battle_reads > 0 {
        facts.push(count_label(battle_reads, "battle read"));
    }
    if game_view_reads > 0 {
        facts.push(count_label(game_view_reads, "game view read"));
    }
    if town_reads > 0 {
        facts.push(count_label(town_reads, "town read"));
    }
    if history_reads > 0 {
        facts.push(count_label(history_reads, "history read"));
    }
    if event_reads > 0 {
        facts.push(count_label(event_reads, "event read"));
    }

    let focus = match id {
        "gate_m_web_client_probe" => {
            "canister-backed browser client route through match finish and result screen"
        }
        _ => "benchmark route",
    };

    format!("{}; {}", facts.join("; "), focus)
}

fn count_label(count: u64, label: &str) -> String {
    if count == 1 {
        format!("1 {label}")
    } else {
        format!("{count} {label}s")
    }
}

fn format_pct(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:+.1}%"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_billions_u128(value: u128) -> String {
    format_scaled(value as f64 / 1_000_000_000.0)
}

fn format_billions_option(value: Option<f64>) -> String {
    value
        .map(|value| format_scaled(value / 1_000_000_000.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_billions_f64(value: f64) -> String {
    format_scaled(value / 1_000_000_000.0)
}

fn format_trillions_u128(value: u128) -> String {
    format_scaled(value as f64 / 1_000_000_000_000.0)
}

fn format_trillions_f64(value: f64) -> String {
    format_scaled(value / 1_000_000_000_000.0)
}

fn format_mb_from_bytes_i128(value: i128) -> String {
    format_scaled(value as f64 / 1_048_576.0)
}

fn format_mb_from_bytes_f64(value: f64) -> String {
    format_scaled(value / 1_048_576.0)
}

fn format_scaled(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".to_string() } else { text }
}

fn git_sha() -> String {
    let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    else {
        return "unknown".to_string();
    };
    if !output.status.success() {
        return "unknown".to_string();
    }
    String::from_utf8(output.stdout)
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn fixture_opening_game_view(fixture: &domm_game::ScenarioFixture) -> GameView {
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();
    backend
        .get_game_view(
            fixture.principals.player_one,
            &session.session_id,
            GameViewRequest {
                viewport: opening_viewport_for_slot(0),
                chunk_cursor: None,
                chunk_limit: MAX_CHUNK_LIMIT,
                object_cursor: None,
                object_limit: 128,
                events_after_seq: 0,
                event_limit: 25,
                include_battle: false,
            },
        )
        .expect("fixture opening game view should load")
}

fn assert_canister_client_state(state: &WebClientState) {
    assert!(state.checklist_complete());
    assert!(state.rematch_available);
    assert!(state.retry_replays >= 3);
    assert!(state.match_result.is_some());
    assert!(state.match_history.entries_returned >= 1);
    let command_errors = state
        .command_log
        .iter()
        .filter(|entry| entry.error_code.is_some())
        .collect::<Vec<_>>();
    assert!(
        command_errors.is_empty(),
        "unexpected command errors: {command_errors:?}"
    );
    assert_replayed(state, "submit_move_intent");
    assert_replayed(state, "sync_session_turn");
    assert_replayed(state, "submit_battle_action");
    assert_command(state, "submit_build_town_structure");
    assert_command(state, "submit_recruit_units");
    assert_command(state, "sync_battle");
    assert!(
        state
            .event_feed
            .iter()
            .any(|event| event.event_type == "session_started")
    );
}

fn assert_representative_dtos_match(fixture: &GameView, canister: &GameView) {
    assert_eq!(
        canister.content_manifest_hash,
        fixture.content_manifest_hash
    );
    assert_eq!(canister.viewport, fixture.viewport);
    assert_eq!(canister.map_chunks.len(), fixture.map_chunks.len());
    assert!(
        canister.objects.len() >= fixture.objects.len(),
        "canister opening view should include at least the fixture object sample"
    );
    for fixture_object in &fixture.objects {
        assert!(
            canister.objects.iter().any(|object| {
                object.subject_kind == fixture_object.subject_kind
                    && object.subject_id_text == fixture_object.subject_id_text
            }),
            "canister opening view is missing fixture object {}:{}",
            fixture_object.subject_kind,
            fixture_object.subject_id_text
        );
    }
    assert_eq!(canister.champions.len(), fixture.champions.len());
    assert_eq!(canister.towns.len(), fixture.towns.len());
    assert_eq!(
        canister
            .champions
            .first()
            .and_then(|champion| champion.name.clone()),
        fixture
            .champions
            .first()
            .and_then(|champion| champion.name.clone())
    );
    assert_eq!(
        canister.towns.first().map(|town| town.town.name.clone()),
        fixture.towns.first().map(|town| town.town.name.clone())
    );
    assert_eq!(
        canister.participant.faction_slug,
        fixture.participant.faction_slug
    );
    assert_eq!(
        canister.render_time.turn_duration_ms,
        fixture.render_time.turn_duration_ms
    );
}

fn assert_command(state: &WebClientState, command_type: &str) {
    assert!(
        state
            .command_log
            .iter()
            .any(|entry| entry.command_type == command_type),
        "missing command {command_type}; log={:?}",
        state.command_log
    );
}

fn assert_replayed(state: &WebClientState, command_type: &str) {
    assert!(
        state
            .command_log
            .iter()
            .any(|entry| entry.command_type == command_type && entry.replayed),
        "missing replayed command {command_type}; log={:?}",
        state.command_log
    );
}

fn choose_battle_action_for_goal(view: &BattleView, aggressive: bool) -> BattleActionInput {
    let active_stack_id = view
        .active_stack_id
        .clone()
        .expect("active battle should have an active stack");
    if !aggressive {
        for preferred in ["Defend", "Wait"] {
            if let Some(action) = view
                .legal_actions_for_caller
                .iter()
                .find(|action| action.enabled && action.action == preferred)
            {
                return BattleActionInput {
                    battle_id: view.battle_id.clone(),
                    battle_stack_id: active_stack_id,
                    action: action.action.clone(),
                    ability_key: action.ability_key.clone(),
                    target_stack_id: None,
                    destination: None,
                };
            }
        }
    }

    for preferred in ["RangedAttack", "MeleeAttack", "Attack"] {
        if let Some(action) = view.legal_actions_for_caller.iter().find(|action| {
            action.enabled && action.action == preferred && !action.targets.is_empty()
        }) {
            return BattleActionInput {
                battle_id: view.battle_id.clone(),
                battle_stack_id: active_stack_id,
                action: action.action.clone(),
                ability_key: action.ability_key.clone(),
                target_stack_id: action.targets.first().cloned(),
                destination: None,
            };
        }
    }
    if let Some(action) = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled && action.action == "Move" && !action.path.is_empty())
    {
        return BattleActionInput {
            battle_id: view.battle_id.clone(),
            battle_stack_id: active_stack_id,
            action: "Move".to_string(),
            ability_key: None,
            target_stack_id: None,
            destination: best_move_destination(view, action),
        };
    }
    let action = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled && action.action != "CastAbility")
        .or_else(|| {
            view.legal_actions_for_caller
                .iter()
                .find(|action| action.enabled)
        })
        .expect("caller should have at least one enabled battle action");
    BattleActionInput {
        battle_id: view.battle_id.clone(),
        battle_stack_id: active_stack_id,
        action: action.action.clone(),
        ability_key: action.ability_key.clone(),
        target_stack_id: action.targets.first().cloned(),
        destination: action.path.first().copied(),
    }
}

fn best_move_destination(
    view: &BattleView,
    action: &LegalBattleAction,
) -> Option<domm_game::BattleCoord> {
    let active_stack_id = view.active_stack_id.as_deref()?;
    let active_side = view
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == active_stack_id)?
        .side
        .clone();
    action.path.iter().copied().min_by_key(|coord| {
        view.stacks
            .iter()
            .filter(|stack| {
                stack.side != active_side && stack.status == "active" && stack.quantity > 0
            })
            .map(|enemy| {
                u16::from(coord.x.abs_diff(enemy.battle_x))
                    + u16::from(coord.y.abs_diff(enemy.battle_y))
            })
            .min()
            .unwrap_or(u16::MAX)
    })
}

fn battle_id_from_events(
    response: &CommandResponse,
    event_type: &str,
) -> Result<String, ProbeError> {
    battle_id_from_event_views(&response.events, event_type).ok_or_else(|| {
        ProbeError::Api(ApiError::new(
            "missing_battle_id",
            format!("{event_type} event should include battle_id"),
            false,
        ))
    })
}

fn battle_id_from_event_views(
    events: &[domm_game::ApiEventView],
    event_type: &str,
) -> Option<String> {
    events
        .iter()
        .find(|event| event.event_type == event_type)
        .and_then(|event| event.payload.as_deref())
        .and_then(|payload| json_string_field(payload, "battle_id"))
}

fn json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = json.get(start..)?.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

fn prefixed(kind: &str, value: &str) -> String {
    let prefix = format!("{kind}:");
    if value.starts_with(&prefix) {
        value.to_string()
    } else {
        format!("{prefix}{value}")
    }
}

fn row_count(snapshot: &DiagnosticStorageSnapshot, entity: &str) -> u32 {
    snapshot
        .row_counts
        .iter()
        .find(|row| row.entity == entity)
        .unwrap_or_else(|| panic!("diagnostic row count missing {entity}"))
        .count
}

fn entity_names(entities: &[&str]) -> Vec<String> {
    entities
        .iter()
        .map(|entity| (*entity).to_string())
        .collect()
}

fn install_degens_canister_fixture() -> StandaloneCanisterFixture {
    let wasm_path = build_degens_canister();
    let wasm = fs::read(&wasm_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", wasm_path.display()));
    install_prebuilt_canister_with_cycles(
        wasm,
        candid::encode_args(()).expect("empty init args encode"),
        100_000_000_000_000,
    )
}

fn build_degens_canister() -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let canister_features = env::var("DOMM_CANISTER_FEATURES").ok();
    let target_dir = workspace_root.join(if canister_features.is_some() {
        "target/pocket-ic-client-probe-benchmark"
    } else {
        "target/pocket-ic-client-probe"
    });
    let linker_wrapper_dir = write_host_linker_wrapper(&target_dir);
    let nested_path = path_with_prefix(&linker_wrapper_dir);
    let mut args = vec![
        "build".to_string(),
        "-p".to_string(),
        "domm-degens-canister".to_string(),
        "--target".to_string(),
        "wasm32-unknown-unknown".to_string(),
        "--release".to_string(),
    ];
    if let Some(features) = canister_features {
        args.push("--features".to_string());
        args.push(features);
    }
    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(args)
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("PATH", nested_path)
        .output()
        .expect("failed to run cargo build for degens canister");
    assert!(
        output.status.success(),
        "canister wasm build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("domm_degens_canister.wasm")
}

fn write_host_linker_wrapper(target_dir: &Path) -> PathBuf {
    let wrapper_dir = target_dir.join("host-linker-wrapper");
    fs::create_dir_all(&wrapper_dir)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", wrapper_dir.display()));
    let wrapper_path = wrapper_dir.join("cc");
    let real_cc = system_cc_path();
    fs::write(
        &wrapper_path,
        format!(
            "#!/bin/sh\nexec '{}' \"$@\" -fuse-ld=bfd\n",
            shell_single_quote(&real_cc)
        ),
    )
    .unwrap_or_else(|error| panic!("failed to write {}: {error}", wrapper_path.display()));
    make_executable(&wrapper_path);
    wrapper_dir
}

fn system_cc_path() -> String {
    let output = Command::new("sh")
        .args(["-c", "command -v cc"])
        .output()
        .expect("failed to resolve system cc");
    assert!(
        output.status.success(),
        "failed to resolve system cc\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("system cc path should be UTF-8")
        .trim()
        .to_string()
}

fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn path_with_prefix(prefix: &Path) -> std::ffi::OsString {
    let mut paths = vec![prefix.to_path_buf()];
    if let Some(existing_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing_path));
    }
    env::join_paths(paths).expect("nested PATH should join")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()))
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|error| panic!("failed to chmod {}: {error}", path.display()));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
