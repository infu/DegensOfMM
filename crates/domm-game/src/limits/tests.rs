use candid::Principal;

use crate::ai::{
    AI_MAX_ACTORS_PER_UPDATE, AI_MAX_CANDIDATES_PER_ACTOR, AI_MAX_CHUNKS_LOADED_PER_ACTOR,
    AI_MAX_EMITTED_COMMANDS_PER_UPDATE, AI_MAX_PATH_NODES_PER_ACTOR,
};
use crate::api::{FixtureApiBackend, GameViewRequest};
use crate::battle::{BATTLE_MAX_ROUNDS, BattleCommandBudget};
use crate::champion::build_first_playable_champion_state;
use crate::cleanup::CleanupBudget;
use crate::command::{
    CommandActor, CommandCoreError, GameCommandPayload, GameEventDraft, RecoveryBudget,
    SessionCommandJournal,
};
use crate::content::{FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH};
use crate::economy::{
    EconomyError, ResourceCapMode, ResourceDelta, ResourceLedgerEntryRecord,
    build_first_playable_economy_state,
};
use crate::fixtures::first_playable_fixture;
use crate::map::build_first_playable_map_state;
use crate::movement::{
    MovementError, MovementSyncBudget, build_first_playable_movement_state, preview_move_path,
};

use super::*;

#[test]
fn hard_limit_constants_match_v1_budget_contract() {
    assert_eq!(MAX_ACTIVE_SESSIONS_PER_CANISTER, 100);
    assert_eq!(MAX_PARTICIPANTS_PER_SESSION, 2);
    assert_eq!(MAX_CHAMPIONS_PER_PARTICIPANT, 3);
    assert_eq!(MAX_TOWNS_PER_SESSION, 6);
    assert_eq!(MAX_MAP_WIDTH, 48);
    assert_eq!(MAX_MAP_HEIGHT, 48);
    assert_eq!(MAX_MAP_CHUNKS_PER_SESSION, 9);
    assert_eq!(MAX_DYNAMIC_OBJECTS_PER_SESSION, 200);
    assert_eq!(MAX_ACTIVE_BATTLES_PER_SESSION, 2);
    assert_eq!(MAX_STACKS_PER_BATTLE_SIDE, 7);
    assert_eq!(MAX_BATTLE_OBSTACLES, 16);
    assert_eq!(MAX_BATTLE_ROUNDS, 20);
    assert_eq!(MAX_AI_ACTORS_PER_UPDATE, 2);
    assert_eq!(MAX_AI_CANDIDATES_PER_ACTOR, 16);
    assert_eq!(MAX_AI_PATH_NODES_PER_ACTOR, 128);
    assert_eq!(MAX_AI_CHUNKS_LOADED_PER_ACTOR, 8);
    assert_eq!(MAX_AI_EMITTED_COMMANDS_PER_UPDATE, 2);
    assert_eq!(MAX_EVENTS_PER_TURN, 100);
    assert_eq!(MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION, 2_000);
    assert_eq!(MAX_EVENTS_RETAINED_PER_ACTIVE_SESSION, 5_000);
    assert_eq!(MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION, 3_000);
    assert_eq!(MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN, 6);
    assert_eq!(MAX_MOVEMENT_MICROSTEPS_PER_SYNC, 384);
    assert_eq!(MAX_BATTLE_STARTS_FROM_MOVEMENT, 2);
    assert_eq!(MAX_OBJECT_INTERACTIONS_FROM_MOVEMENT, 6);
    assert_eq!(MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE, 2);
    assert_eq!(MAX_CLEANUP_ROWS_PER_UPDATE, 100);
    assert_eq!(MAX_FINISHED_SESSIONS_CLEANED_PER_UPDATE, 1);
    assert_eq!(DEFAULT_LIST_LIMIT, 50);
    assert_eq!(MAX_LIST_LIMIT, 200);
    assert_eq!(MAX_VIEWPORT_CHUNKS_PER_REQUEST, 9);
    assert_eq!(MAX_RECENT_EVENTS_IN_GAME_VIEW, 50);
    assert_eq!(MAX_COMMAND_PAYLOAD_JSON_BYTES, 4_096);
    assert_eq!(MAX_COMMAND_RESULT_JSON_BYTES, 4_096);
    assert_eq!(MAX_EVENT_PAYLOAD_JSON_BYTES, 4_096);
    assert_eq!(MAX_MOVEMENT_INTENT_PATH_JSON_BYTES, 2_048);

    assert_eq!(FIRST_PLAYABLE_MAP_WIDTH, MAX_MAP_WIDTH);
    assert_eq!(FIRST_PLAYABLE_MAP_HEIGHT, MAX_MAP_HEIGHT);
    assert_eq!(BATTLE_MAX_ROUNDS, MAX_BATTLE_ROUNDS);
    assert_eq!(
        BattleCommandBudget::default().max_timeout_actions,
        MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE
    );
    assert_eq!(
        CleanupBudget::default().max_rows,
        MAX_CLEANUP_ROWS_PER_UPDATE
    );
    assert_eq!(
        CleanupBudget::default().max_finished_sessions,
        MAX_FINISHED_SESSIONS_CLEANED_PER_UPDATE
    );
    assert_eq!(
        MovementSyncBudget::default().max_microsteps,
        MAX_MOVEMENT_MICROSTEPS_PER_SYNC
    );
    assert_eq!(
        MovementSyncBudget::default().max_commands_inspected,
        RECOVERY_COMMANDS_INSPECTED_PER_UPDATE
    );
    assert_eq!(
        RecoveryBudget::default().advance_commands,
        RECOVERY_COMMANDS_ADVANCED_PER_UPDATE as usize
    );
    assert_eq!(AI_MAX_ACTORS_PER_UPDATE, MAX_AI_ACTORS_PER_UPDATE);
    assert_eq!(AI_MAX_CANDIDATES_PER_ACTOR, MAX_AI_CANDIDATES_PER_ACTOR);
    assert_eq!(AI_MAX_PATH_NODES_PER_ACTOR, MAX_AI_PATH_NODES_PER_ACTOR);
    assert_eq!(
        AI_MAX_CHUNKS_LOADED_PER_ACTOR,
        MAX_AI_CHUNKS_LOADED_PER_ACTOR
    );
    assert_eq!(
        AI_MAX_EMITTED_COMMANDS_PER_UPDATE,
        MAX_AI_EMITTED_COMMANDS_PER_UPDATE
    );
}

#[test]
fn command_payload_result_event_and_retention_caps_fail_closed() {
    let actor = CommandActor::player("player:one", "participant:one", None);
    let oversized_payload = "x".repeat(MAX_COMMAND_PAYLOAD_JSON_BYTES + 1);
    let mut journal = SessionCommandJournal::new("session:limits", 0);

    let error = journal
        .submit_command(GameCommandPayload::from_json(
            "session:limits",
            actor.clone(),
            1,
            1,
            "test",
            oversized_payload,
        ))
        .expect_err("oversized command payload should fail");
    assert!(matches!(
        error,
        CommandCoreError::PayloadTooLarge {
            field,
            max_bytes: MAX_COMMAND_PAYLOAD_JSON_BYTES,
            actual_bytes,
        } if field == "command.payload_json" && actual_bytes == MAX_COMMAND_PAYLOAD_JSON_BYTES + 1
    ));

    let command = journal
        .submit_command(GameCommandPayload::from_json(
            "session:limits",
            actor.clone(),
            1,
            2,
            "test",
            "{}",
        ))
        .expect("small command should submit")
        .command;
    let error = journal
        .mark_command_applied(
            &command.id,
            Some("x".repeat(MAX_COMMAND_RESULT_JSON_BYTES + 1)),
        )
        .expect_err("oversized command result should fail");
    assert!(matches!(
        error,
        CommandCoreError::PayloadTooLarge {
            field,
            max_bytes: MAX_COMMAND_RESULT_JSON_BYTES,
            actual_bytes,
        } if field == "command.result_json" && actual_bytes == MAX_COMMAND_RESULT_JSON_BYTES + 1
    ));

    let error = journal
        .append_event(GameEventDraft::public(
            "session:limits",
            None,
            1,
            "event:oversized",
            "test_event",
            "x".repeat(MAX_EVENT_PAYLOAD_JSON_BYTES + 1),
        ))
        .expect_err("oversized event payload should fail");
    assert!(matches!(
        error,
        CommandCoreError::PayloadTooLarge {
            field,
            max_bytes: MAX_EVENT_PAYLOAD_JSON_BYTES,
            actual_bytes,
        } if field == "event.payload_json" && actual_bytes == MAX_EVENT_PAYLOAD_JSON_BYTES + 1
    ));

    for index in 0..MAX_EVENTS_PER_TURN {
        journal
            .append_event(GameEventDraft::public(
                "session:limits",
                None,
                1,
                format!("event:{index}"),
                "test_event",
                "{}",
            ))
            .expect("event within per-turn cap should append");
    }
    let error = journal
        .append_event(GameEventDraft::public(
            "session:limits",
            None,
            1,
            "event:over-turn-cap",
            "test_event",
            "{}",
        ))
        .expect_err("event above per-turn cap should fail");
    assert!(matches!(
        error,
        CommandCoreError::EventsPerTurnLimitExceeded {
            turn_number: 1,
            max_events: MAX_EVENTS_PER_TURN,
        }
    ));

    let mut full_journal = SessionCommandJournal::new("session:retention", 0);
    for nonce in 0..MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION {
        full_journal
            .submit_command(GameCommandPayload::from_json(
                "session:retention",
                actor.clone(),
                1,
                nonce as u64,
                "test",
                "{}",
            ))
            .expect("command within retention cap should submit");
    }
    let error = full_journal
        .submit_command(GameCommandPayload::from_json(
            "session:retention",
            actor,
            1,
            MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION as u64,
            "test",
            "{}",
        ))
        .expect_err("command above retention cap should fail");
    assert!(matches!(
        error,
        CommandCoreError::CommandRetentionLimitExceeded {
            max_commands: MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION,
        }
    ));
}

#[test]
fn resource_ledger_retention_cap_is_enforced() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = fixture.ids.participant_one_id;

    for index in 0..MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION {
        state.ledger_entries.push(ResourceLedgerEntryRecord {
            id: format!("ledger:existing:{index}"),
            session_id: state.session_id.clone(),
            participant_id: participant_id.clone(),
            command_id: format!("command:existing:{index}"),
            ledger_key: format!("gold:existing:{index}"),
            turn_number: 1,
            resource_key: "gold".to_string(),
            delta: 0,
            balance_after: 10_000,
            reason: "existing".to_string(),
            status: "applied".to_string(),
        });
    }

    let error = state
        .apply_resource_deltas(
            "command:over-ledger-cap",
            1,
            vec![ResourceDelta {
                participant_id,
                resource_key: "wood".to_string(),
                delta: 1,
                reason: "test_reward".to_string(),
                effect_key: "effect:over-ledger-cap".to_string(),
                phase: "apply".to_string(),
            }],
            ResourceCapMode::RejectOnOverflow,
        )
        .expect_err("ledger write above retention cap should fail");

    assert!(matches!(
        error,
        EconomyError::ResourceLedgerRetentionLimitExceeded {
            max_rows: MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION,
        }
    ));
}

#[test]
fn api_query_limits_reject_oversized_requests_and_clamp_event_pages() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();

    let mut request = GameViewRequest::opening_for_slot(0);
    request.chunk_limit = MAX_VIEWPORT_CHUNKS_PER_REQUEST + 1;
    let error = backend
        .get_game_view(fixture.principals.player_one, &session.session_id, request)
        .expect_err("oversized chunk limit should fail");
    assert_eq!(error.code, "viewport_chunk_limit_exceeded");

    let viewport = GameViewRequest::opening_for_slot(0).viewport;
    let error = backend
        .get_visible_map_chunks(
            fixture.principals.player_one,
            &session.session_id,
            &viewport,
            None,
            MAX_VIEWPORT_CHUNKS_PER_REQUEST + 1,
        )
        .expect_err("direct chunk query should reject oversized limit");
    assert_eq!(error.code, "viewport_chunk_limit_exceeded");

    let mut request = GameViewRequest::opening_for_slot(0);
    request.event_limit = MAX_RECENT_EVENTS_IN_GAME_VIEW + 1;
    let error = backend
        .get_game_view(fixture.principals.player_one, &session.session_id, request)
        .expect_err("oversized event limit should fail");
    assert_eq!(error.code, "event_limit_exceeded");

    let audience = format!("participant:{}", fixture.ids.participant_one_id);
    let page = backend.get_events_after(&session.session_id, &audience, 0, MAX_LIST_LIMIT + 1);
    assert_eq!(page.page_info.limit, MAX_LIST_LIMIT);
}

#[test]
fn movement_path_caps_reject_overlong_paths_before_writes() {
    let fixture = first_playable_fixture();
    let movement = build_first_playable_movement_state();
    let map = build_first_playable_map_state();
    let champions = build_first_playable_champion_state();
    let path = vec![crate::movement::MoveCoord::new(9, 24); MAX_MOVE_PATH_STEPS_LIMIT + 1];

    let error = preview_move_path(
        &movement,
        &map,
        &champions,
        &fixture.ids.participant_one_id,
        "champion:west",
        path,
        1_000,
    )
    .expect_err("overlong movement paths should fail before writes");

    assert!(matches!(
        error,
        MovementError::PathTooLong {
            path_len,
            max_len: MAX_MOVE_PATH_STEPS_LIMIT,
        } if path_len == MAX_MOVE_PATH_STEPS_LIMIT + 1
    ));
    assert!(movement.intents.is_empty());
}

#[test]
fn api_command_payload_limit_returns_stored_failure() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let response = backend.register_player(
        fixture.principals.player_one,
        &"x".repeat(MAX_COMMAND_PAYLOAD_JSON_BYTES + 20),
        "nonce:oversized-register",
    );

    assert_eq!(response.status, crate::command::CommandStatus::Failed);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("payload_too_large")
    );
    assert_eq!(
        backend
            .get_command_status("nonce:oversized-register")
            .and_then(|status| status.error_code),
        Some("payload_too_large".to_string())
    );
}

#[test]
fn first_playable_measurement_report_stays_under_budgets() {
    let report = measure_first_playable_performance().expect("measurement should run");
    eprintln!(
        "first-playable budgets: commands={} events={} queries={} rows={} max_query_bytes={} estimated_response_bytes={}",
        report.command_count,
        report.event_count,
        report.query_count,
        report.storage_row_count,
        report.max_query_bytes,
        report.estimated_response_bytes
    );

    assert!(report.command_count > 0);
    assert!(report.event_count > 0);
    assert!(report.query_count > 0);
    assert!(report.max_query_under_budget);
    assert!(report.storage_under_active_row_budget);
    assert!(report.concerns.is_empty(), "{:?}", report.concerns);
}

#[test]
fn anonymous_principal_still_cannot_bypass_query_limits() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();
    let viewport = GameViewRequest::opening_for_slot(0).viewport;

    let error = backend
        .get_visible_objects(
            Principal::anonymous(),
            &session.session_id,
            &viewport,
            None,
            MAX_LIST_LIMIT + 1,
        )
        .expect_err("limit validation should run before auth-backed object query");

    assert_eq!(error.code, "list_limit_exceeded");
}
