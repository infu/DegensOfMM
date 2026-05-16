use super::assert_active_session_capacity;
use crate::aftermath::{
    AftermathState, apply_battle_aftermath, build_first_playable_aftermath_state,
    check_and_finalize_victory, resolve_neutral_battle_for_fixture,
    seed_resolved_champion_defeat_battle, seed_resolved_town_capture_battle,
};
use crate::battle::{BattleCommandRecord, BattleEventRecord};
use crate::cleanup::{
    ACTIVE_SESSION_LIMIT, CleanupBudget, CleanupCanisterSnapshot, CleanupError, CleanupPolicy,
    CleanupTarget, RAW_FINISHED_LOG_RETENTION_MS, compact_finished_session,
    should_compact_raw_finished_logs,
};
use crate::fixtures::first_playable_fixture;
use crate::map::MapOccupancyRecord;

const FINISHED_AT_MS: u64 = 1_800_000_530_000;

#[test]
fn finished_session_cleanup_retains_summaries_and_deletes_expired_raw_logs() {
    let mut state = finished_state();
    let starting_match_summaries = state.player_match_summaries.len();
    let starting_history = state.match_history.len();
    let report = compact_finished_session(
        &mut state,
        expired_target(),
        CleanupBudget::default(),
        expired_policy(),
    )
    .expect("finished cleanup should run");

    assert!(report.completed);
    assert_eq!(report.cleaned_sessions, 1);
    assert!(report.event_summaries_written >= 1);
    assert!(report.ledger_summaries_written >= 1);
    assert!(report.map_occupancy_rows_removed > 0);
    assert!(report.battle_rows_removed > 0);
    assert!(report.visibility_rows_removed > 0);
    assert!(report.raw_event_rows_removed > 0);
    assert!(report.raw_ledger_rows_removed > 0);

    assert_eq!(state.player_match_summaries.len(), starting_match_summaries);
    assert_eq!(state.match_history.len(), starting_history);
    assert!(!state.event_turn_summaries.is_empty());
    assert!(!state.economy.turn_summaries.is_empty());
    assert!(state.aftermath_events.is_empty());
    assert!(state.economy.ledger_entries.is_empty());
    assert!(state.battle.battles.is_empty());
    assert!(state.battle.stacks.is_empty());
    assert!(state.battle.occupancy.is_empty());
    assert!(state.map.occupancy_rows.is_empty());
    assert!(state.map.visibility_chunks.is_empty());
}

#[test]
fn cleanup_is_bounded_and_preserves_dependency_order_across_retries() {
    let mut state = finished_state();
    let mut reports = Vec::new();
    let budget = CleanupBudget {
        max_rows: 7,
        max_finished_sessions: 1,
    };

    for _ in 0..50 {
        let report =
            compact_finished_session(&mut state, expired_target(), budget, expired_policy())
                .expect("bounded cleanup should retry cleanly");
        assert!(report.rows_compacted <= budget.max_rows);
        let completed = report.completed;
        reports.push(report);
        if completed {
            break;
        }
    }

    assert!(
        reports.last().is_some_and(|report| report.completed),
        "cleanup should finish under repeated bounded calls"
    );
    let operations = reports
        .iter()
        .flat_map(|report| report.operations.iter().cloned())
        .collect::<Vec<_>>();
    assert_before(
        &operations,
        "write_event_turn_summary",
        "delete_aftermath_events",
    );
    assert_before(
        &operations,
        "write_resource_ledger_turn_summary",
        "delete_resource_ledger_entries",
    );
    assert_before(
        &operations,
        "delete_map_occupancy:champion",
        "delete_battle_occupancy",
    );
    assert!(state.map.occupancy_rows.is_empty());
    assert!(state.map.visibility_chunks.is_empty());
    assert!(state.battle.battles.is_empty());
}

#[test]
fn cleanup_operations_follow_schema_deletion_policy() {
    let fixture = first_playable_fixture();
    let mut state = finished_state();
    let battle_id = state.battle.battles[0].battle_id.clone();
    seed_cleanup_occupancy_marker(&mut state, "champion", "champion:cleanup-marker");
    seed_cleanup_occupancy_marker(&mut state, "town", "town:cleanup-marker");
    seed_cleanup_occupancy_marker(&mut state, "artifact", "artifact:cleanup-marker");
    seed_cleanup_occupancy_marker(&mut state, "neutral_army", "neutral:cleanup-marker");
    seed_cleanup_occupancy_marker(&mut state, "world_object", "object:cleanup-marker");
    state.battle.events.push(BattleEventRecord {
        event_seq: 99_001,
        battle_id: battle_id.clone(),
        event_key: "battle:cleanup:seeded-event".to_string(),
        command_id: "command:cleanup:applied-battle".to_string(),
        event_type: "cleanup_seeded_battle_event".to_string(),
        subject_id_text: "battle:cleanup".to_string(),
        payload: "{\"cleanup\":true}".to_string(),
    });
    state.battle.commands.push(BattleCommandRecord {
        command_id: "command:cleanup:applied-battle".to_string(),
        battle_id,
        actor_participant_id: Some(fixture.ids.participant_one_id),
        battle_stack_id: None,
        client_nonce: "nonce:cleanup:applied-battle".to_string(),
        payload_hash: "hash:cleanup:applied-battle".to_string(),
        action: "attack".to_string(),
        target_stack_id: None,
        destination: None,
        system: false,
        status: "applied".to_string(),
        created_at: FINISHED_AT_MS,
        applied_at: Some(FINISHED_AT_MS + 1),
        retryable_error: None,
    });

    let report = compact_finished_session(
        &mut state,
        expired_target(),
        CleanupBudget {
            max_rows: 500,
            max_finished_sessions: 1,
        },
        expired_policy(),
    )
    .expect("cleanup should complete");

    assert!(report.completed);
    assert_before(
        &report.operations,
        "write_event_turn_summary",
        "delete_battle_events",
    );
    assert_before(
        &report.operations,
        "write_event_turn_summary",
        "delete_aftermath_events",
    );
    assert_before(
        &report.operations,
        "write_resource_ledger_turn_summary",
        "delete_resource_ledger_entries",
    );
    for occupant_kind in [
        "champion",
        "town",
        "artifact",
        "neutral_army",
        "world_object",
    ] {
        assert_before(
            &report.operations,
            &format!("delete_map_occupancy:{occupant_kind}"),
            "delete_battle_occupancy",
        );
    }
    assert_before(
        &report.operations,
        "delete_battle_occupancy",
        "delete_battle_stacks",
    );
    assert_before(
        &report.operations,
        "delete_battle_obstacles",
        "delete_battle_rows",
    );
    assert_before(
        &report.operations,
        "delete_battle_stacks",
        "delete_battle_rows",
    );
    assert_before(
        &report.operations,
        "delete_known_object_rows",
        "delete_visibility_chunks",
    );
    assert_before(
        &report.operations,
        "delete_battle_events",
        "delete_battle_commands",
    );
    assert_before(
        &report.operations,
        "delete_aftermath_events",
        "delete_battle_commands",
    );
    assert_before(
        &report.operations,
        "delete_resource_ledger_entries",
        "delete_battle_commands",
    );
    assert_before(
        &report.operations,
        "delete_battle_commands",
        "delete_applied_command_markers",
    );

    assert!(!state.player_match_summaries.is_empty());
    assert!(!state.match_history.is_empty());
    assert!(!state.event_turn_summaries.is_empty());
    assert!(!state.economy.turn_summaries.is_empty());
    assert!(state.map.occupancy_rows.is_empty());
    assert!(state.map.known_objects.is_empty());
    assert!(state.map.visibility_chunks.is_empty());
    assert!(state.battle.occupancy.is_empty());
    assert!(state.battle.obstacles.is_empty());
    assert!(state.battle.stacks.is_empty());
    assert!(state.battle.battles.is_empty());
    assert!(state.battle.events.is_empty());
    assert!(state.battle.commands.is_empty());
    assert!(state.aftermath_events.is_empty());
    assert!(state.economy.ledger_entries.is_empty());
    assert!(state.applied_commands.is_empty());
}

#[test]
fn cleanup_refuses_active_sessions_and_active_recovery_rows_without_writes() {
    let mut active = build_first_playable_aftermath_state().expect("active state should build");
    let active_occupancy = active.map.occupancy_rows.len();
    let active_error = compact_finished_session(
        &mut active,
        expired_target(),
        CleanupBudget::default(),
        expired_policy(),
    )
    .expect_err("active session should not be compacted");
    assert!(matches!(
        active_error,
        CleanupError::SessionNotFinished { .. }
    ));
    assert_eq!(active.map.occupancy_rows.len(), active_occupancy);

    let fixture = first_playable_fixture();
    let mut recovering = finished_state();
    let battle_id = recovering.battle.battles[0].battle_id.clone();
    let occupancy_before = recovering.map.occupancy_rows.len();
    recovering.battle.commands.push(BattleCommandRecord {
        command_id: "command:battle:recovering".to_string(),
        battle_id,
        actor_participant_id: Some(fixture.ids.participant_one_id),
        battle_stack_id: None,
        client_nonce: "nonce:recovering".to_string(),
        payload_hash: "hash:recovering".to_string(),
        action: "attack".to_string(),
        target_stack_id: None,
        destination: None,
        system: false,
        status: "applying".to_string(),
        created_at: FINISHED_AT_MS,
        applied_at: None,
        retryable_error: None,
    });
    let recovery_error = compact_finished_session(
        &mut recovering,
        expired_target(),
        CleanupBudget::default(),
        expired_policy(),
    )
    .expect_err("active recovery rows should block cleanup");
    assert!(matches!(
        recovery_error,
        CleanupError::ActiveRecoveryRows { .. }
    ));
    assert_eq!(recovering.battle.commands.len(), 1);
    assert_eq!(recovering.map.occupancy_rows.len(), occupancy_before);
}

#[test]
fn raw_log_retention_and_active_session_caps_are_enforced() {
    let fresh_policy = CleanupPolicy::at(FINISHED_AT_MS + RAW_FINISHED_LOG_RETENTION_MS - 1);
    let old_policy = expired_policy();
    assert!(!should_compact_raw_finished_logs(
        &CleanupTarget {
            finished_at_ms: FINISHED_AT_MS,
            finished_raw_session_rank: 1,
        },
        &fresh_policy
    ));
    assert!(should_compact_raw_finished_logs(
        &expired_target(),
        &old_policy
    ));
    assert!(should_compact_raw_finished_logs(
        &CleanupTarget {
            finished_at_ms: FINISHED_AT_MS + 1,
            finished_raw_session_rank: 101,
        },
        &fresh_policy
    ));

    assert!(
        assert_active_session_capacity(
            &CleanupCanisterSnapshot {
                active_session_count: ACTIVE_SESSION_LIMIT - 1,
                finished_raw_session_count: 0,
            },
            &fresh_policy,
        )
        .is_ok()
    );
    assert!(matches!(
        assert_active_session_capacity(
            &CleanupCanisterSnapshot {
                active_session_count: ACTIVE_SESSION_LIMIT,
                finished_raw_session_count: 0,
            },
            &fresh_policy,
        ),
        Err(CleanupError::ActiveSessionLimitReached { .. })
    ));
}

#[test]
fn zero_finished_session_budget_fails_closed() {
    let mut state = finished_state();
    let err = compact_finished_session(
        &mut state,
        expired_target(),
        CleanupBudget {
            max_rows: 100,
            max_finished_sessions: 0,
        },
        expired_policy(),
    )
    .expect_err("zero finished-session budget should fail");

    assert_eq!(err, CleanupError::NoFinishedSessionBudget);
    assert!(!state.map.occupancy_rows.is_empty());
}

fn finished_state() -> AftermathState {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_aftermath_state().expect("aftermath state should build");
    state
        .economy
        .collect_resource_pile(
            &fixture.ids.participant_one_id,
            "pile:west-wood-1",
            1,
            "command:cleanup:pickup",
        )
        .expect("cleanup fixture should have one ledger row");
    state
        .applied_commands
        .push("command:cleanup:pickup".to_string());

    let neutral_battle_id =
        resolve_neutral_battle_for_fixture(&mut state, "command:cleanup:neutral-resolve")
            .expect("neutral battle should resolve");
    apply_battle_aftermath(
        &mut state,
        &neutral_battle_id,
        "command:cleanup:neutral-aftermath",
        FINISHED_AT_MS - 30_000,
    )
    .expect("neutral aftermath should apply");

    let town_battle_id = seed_resolved_town_capture_battle(&mut state);
    apply_battle_aftermath(
        &mut state,
        &town_battle_id,
        "command:cleanup:town-aftermath",
        FINISHED_AT_MS - 20_000,
    )
    .expect("town aftermath should apply");

    let champion_battle_id = seed_resolved_champion_defeat_battle(&mut state);
    apply_battle_aftermath(
        &mut state,
        &champion_battle_id,
        "command:cleanup:champion-aftermath",
        FINISHED_AT_MS - 10_000,
    )
    .expect("champion aftermath should apply");
    check_and_finalize_victory(&mut state, "command:cleanup:victory", FINISHED_AT_MS)
        .expect("victory should finalize");
    state.session.state = "finished".to_string();
    state
}

fn expired_target() -> CleanupTarget {
    CleanupTarget {
        finished_at_ms: FINISHED_AT_MS,
        finished_raw_session_rank: 1,
    }
}

fn expired_policy() -> CleanupPolicy {
    CleanupPolicy::at(FINISHED_AT_MS + RAW_FINISHED_LOG_RETENTION_MS + 1)
}

fn seed_cleanup_occupancy_marker(
    state: &mut AftermathState,
    occupant_kind: &str,
    occupant_id_text: &str,
) {
    state.map.occupancy_rows.push(MapOccupancyRecord {
        occupancy_id: format!("occ:cleanup:{occupant_kind}:{occupant_id_text}"),
        session_id: state.session.session_id.clone(),
        x: 47,
        y: 47,
        chunk_x: 2,
        chunk_y: 2,
        layer: occupant_kind.to_string(),
        occupant_kind: occupant_kind.to_string(),
        occupant_id_text: occupant_id_text.to_string(),
        occupant_cell_index: 0,
        blocking: true,
        last_command_id: Some("command:cleanup:occupancy-marker".to_string()),
    });
}

fn assert_before(operations: &[String], first: &str, second: &str) {
    let first_index = operations
        .iter()
        .position(|operation| operation == first)
        .unwrap_or_else(|| panic!("missing operation {first}; operations={operations:?}"));
    let second_index = operations
        .iter()
        .position(|operation| operation == second)
        .unwrap_or_else(|| panic!("missing operation {second}; operations={operations:?}"));
    assert!(
        first_index < second_index,
        "{first} should occur before {second}: {operations:?}"
    );
}
