use std::collections::BTreeSet;

use crate::aftermath::AftermathState;
use crate::command::GameEventTurnSummaryRecord;

use super::types::{
    CleanupBudget, CleanupCanisterSnapshot, CleanupError, CleanupPolicy, CleanupReport,
    CleanupTarget,
};

const OCCUPANCY_CLEANUP_ORDER: &[&str] = &[
    "champion",
    "town",
    "artifact",
    "neutral_army",
    "world_object",
];

pub fn assert_active_session_capacity(
    snapshot: &CleanupCanisterSnapshot,
    policy: &CleanupPolicy,
) -> Result<(), CleanupError> {
    if snapshot.active_session_count >= policy.active_session_limit {
        return Err(CleanupError::ActiveSessionLimitReached {
            active_session_count: snapshot.active_session_count,
            active_session_limit: policy.active_session_limit,
        });
    }
    Ok(())
}

#[must_use]
pub fn should_compact_raw_finished_logs(target: &CleanupTarget, policy: &CleanupPolicy) -> bool {
    policy.now_ms.saturating_sub(target.finished_at_ms) >= policy.raw_log_retention_ms
        || target.finished_raw_session_rank > policy.max_finished_raw_sessions
}

pub fn compact_finished_session(
    state: &mut AftermathState,
    target: CleanupTarget,
    budget: CleanupBudget,
    policy: CleanupPolicy,
) -> Result<CleanupReport, CleanupError> {
    if budget.max_finished_sessions == 0 {
        return Err(CleanupError::NoFinishedSessionBudget);
    }
    if state.session.state != "finished" {
        return Err(CleanupError::SessionNotFinished {
            session_id: state.session.session_id.clone(),
        });
    }
    ensure_no_active_recovery_rows(state)?;

    let mut report = CleanupReport::new(state.session.session_id.clone());
    report.cleaned_sessions = 1;
    report.raw_logs_retained = !should_compact_raw_finished_logs(&target, &policy);

    write_event_summaries(state, &mut report, &budget);
    if report.budget_exhausted {
        finalize_report(state, &mut report);
        return Ok(report);
    }

    write_ledger_summaries(state, &mut report, &budget)?;
    if report.budget_exhausted {
        finalize_report(state, &mut report);
        return Ok(report);
    }

    cleanup_map_occupancy(state, &mut report, &budget);
    if report.budget_exhausted {
        finalize_report(state, &mut report);
        return Ok(report);
    }

    cleanup_battle_operational_rows(state, &mut report, &budget);
    if report.budget_exhausted {
        finalize_report(state, &mut report);
        return Ok(report);
    }

    cleanup_visibility_rows(state, &mut report, &budget);
    if report.budget_exhausted {
        finalize_report(state, &mut report);
        return Ok(report);
    }

    if should_compact_raw_finished_logs(&target, &policy) {
        cleanup_raw_event_rows(state, &mut report, &budget);
        if report.budget_exhausted {
            finalize_report(state, &mut report);
            return Ok(report);
        }

        cleanup_raw_ledger_rows(state, &mut report, &budget);
        if report.budget_exhausted {
            finalize_report(state, &mut report);
            return Ok(report);
        }

        cleanup_raw_command_rows(state, &mut report, &budget);
    }

    finalize_report(state, &mut report);
    Ok(report)
}

fn ensure_no_active_recovery_rows(state: &AftermathState) -> Result<(), CleanupError> {
    if let Some(command) = state
        .battle
        .commands
        .iter()
        .find(|command| command.status == "pending" || command.status == "applying")
    {
        return Err(CleanupError::ActiveRecoveryRows {
            reason: format!("battle command {}", command.command_id),
        });
    }
    if let Some(entry) = state
        .economy
        .ledger_entries
        .iter()
        .find(|entry| entry.status != "applied")
    {
        return Err(CleanupError::ActiveRecoveryRows {
            reason: format!("resource ledger entry {}", entry.id),
        });
    }
    Ok(())
}

fn write_event_summaries(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    if state.aftermath_events.is_empty() && state.battle.events.is_empty() {
        return;
    }
    if state.event_turn_summaries.iter().any(|summary| {
        summary.audience_key == "public" && summary.turn_number == state.session.current_turn
    }) {
        return;
    }
    if spend(report, budget, 1, "write_event_turn_summary") == 0 {
        return;
    }

    let mut events = state
        .aftermath_events
        .iter()
        .map(|event| (event.sequence, event.event_type.as_str()))
        .chain(
            state
                .battle
                .events
                .iter()
                .map(|event| (event.event_seq, event.event_type.as_str())),
        )
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.0);

    let first = events.first().map_or(0, |event| event.0);
    let last = events.last().map_or(0, |event| event.0);
    let event_types = events
        .iter()
        .map(|event| event.1)
        .collect::<Vec<_>>()
        .join(",");
    state.event_turn_summaries.push(GameEventTurnSummaryRecord {
        session_id: state.session.session_id.clone(),
        audience_key: "public".to_string(),
        turn_number: state.session.current_turn,
        first_event_seq: first,
        last_event_seq: last,
        event_count: events.len() as u32,
        summary_json: format!(
            "{{\"event_count\":{},\"event_types\":\"{}\"}}",
            events.len(),
            escape_json(&event_types)
        ),
    });
    report.event_summaries_written = report.event_summaries_written.saturating_add(1);
}

fn write_ledger_summaries(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) -> Result<(), CleanupError> {
    let keys = state
        .economy
        .ledger_entries
        .iter()
        .filter(|entry| entry.status == "applied")
        .map(|entry| (entry.participant_id.clone(), entry.turn_number))
        .collect::<BTreeSet<_>>();

    for (participant_id, turn_number) in keys {
        let exists = state.economy.turn_summaries.iter().any(|summary| {
            summary.participant_id == participant_id && summary.turn_number == turn_number
        });
        if exists {
            continue;
        }
        if spend(report, budget, 1, "write_resource_ledger_turn_summary") == 0 {
            return Ok(());
        }
        state
            .economy
            .write_turn_summary(&participant_id, turn_number)?;
        report.ledger_summaries_written = report.ledger_summaries_written.saturating_add(1);
    }
    Ok(())
}

fn cleanup_map_occupancy(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    for occupant_kind in OCCUPANCY_CLEANUP_ORDER {
        let matching = state
            .map
            .occupancy_rows
            .iter()
            .filter(|row| row.occupant_kind == *occupant_kind)
            .count() as u32;
        if matching == 0 {
            continue;
        }
        let removed = spend(
            report,
            budget,
            matching,
            &format!("delete_map_occupancy:{occupant_kind}"),
        );
        if removed == 0 {
            return;
        }
        retain_after_removing_first(&mut state.map.occupancy_rows, removed as usize, |row| {
            row.occupant_kind == *occupant_kind
        });
        report.map_occupancy_rows_removed =
            report.map_occupancy_rows_removed.saturating_add(removed);
        if removed < matching {
            return;
        }
    }

    let remaining = state.map.occupancy_rows.len() as u32;
    if remaining > 0 {
        let removed = spend(report, budget, remaining, "delete_map_occupancy:remaining");
        if removed > 0 {
            state.map.occupancy_rows.drain(0..removed as usize);
            report.map_occupancy_rows_removed =
                report.map_occupancy_rows_removed.saturating_add(removed);
        }
    }
}

fn cleanup_battle_operational_rows(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    drain_vec(
        &mut state.battle.occupancy,
        report,
        budget,
        "delete_battle_occupancy",
        |report, removed| {
            report.battle_rows_removed = report.battle_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.battle.obstacles,
        report,
        budget,
        "delete_battle_obstacles",
        |report, removed| {
            report.battle_rows_removed = report.battle_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.battle.stacks,
        report,
        budget,
        "delete_battle_stacks",
        |report, removed| {
            report.battle_rows_removed = report.battle_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.battle.battles,
        report,
        budget,
        "delete_battle_rows",
        |report, removed| {
            report.battle_rows_removed = report.battle_rows_removed.saturating_add(removed);
        },
    );
}

fn cleanup_visibility_rows(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    drain_vec(
        &mut state.map.known_objects,
        report,
        budget,
        "delete_known_object_rows",
        |report, removed| {
            report.visibility_rows_removed = report.visibility_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.map.visibility_chunks,
        report,
        budget,
        "delete_visibility_chunks",
        |report, removed| {
            report.visibility_rows_removed = report.visibility_rows_removed.saturating_add(removed);
        },
    );
}

fn cleanup_raw_event_rows(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    drain_vec(
        &mut state.battle.events,
        report,
        budget,
        "delete_battle_events",
        |report, removed| {
            report.raw_event_rows_removed = report.raw_event_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.aftermath_events,
        report,
        budget,
        "delete_aftermath_events",
        |report, removed| {
            report.raw_event_rows_removed = report.raw_event_rows_removed.saturating_add(removed);
        },
    );
}

fn cleanup_raw_ledger_rows(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    drain_vec(
        &mut state.economy.ledger_entries,
        report,
        budget,
        "delete_resource_ledger_entries",
        |report, removed| {
            report.raw_ledger_rows_removed = report.raw_ledger_rows_removed.saturating_add(removed);
        },
    );
}

fn cleanup_raw_command_rows(
    state: &mut AftermathState,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
) {
    drain_vec(
        &mut state.battle.commands,
        report,
        budget,
        "delete_battle_commands",
        |report, removed| {
            report.battle_rows_removed = report.battle_rows_removed.saturating_add(removed);
        },
    );
    if report.budget_exhausted {
        return;
    }
    drain_vec(
        &mut state.applied_commands,
        report,
        budget,
        "delete_applied_command_markers",
        |_, _| {},
    );
}

fn drain_vec<T, F>(
    rows: &mut Vec<T>,
    report: &mut CleanupReport,
    budget: &CleanupBudget,
    operation: &str,
    mut record: F,
) where
    F: FnMut(&mut CleanupReport, u32),
{
    let count = rows.len() as u32;
    if count == 0 {
        return;
    }
    let removed = spend(report, budget, count, operation);
    if removed == 0 {
        return;
    }
    rows.drain(0..removed as usize);
    record(report, removed);
}

fn spend(report: &mut CleanupReport, budget: &CleanupBudget, wanted: u32, operation: &str) -> u32 {
    if wanted == 0 {
        return 0;
    }
    let remaining = budget.max_rows.saturating_sub(report.rows_compacted);
    if remaining == 0 {
        report.budget_exhausted = true;
        return 0;
    }
    let taken = wanted.min(remaining);
    report.rows_compacted = report.rows_compacted.saturating_add(taken);
    report.operations.push(operation.to_string());
    if taken < wanted {
        report.budget_exhausted = true;
    }
    taken
}

fn retain_after_removing_first<T, F>(rows: &mut Vec<T>, mut remove_count: usize, mut matches: F)
where
    F: FnMut(&T) -> bool,
{
    rows.retain(|row| {
        if remove_count > 0 && matches(row) {
            remove_count -= 1;
            false
        } else {
            true
        }
    });
}

fn finalize_report(state: &AftermathState, report: &mut CleanupReport) {
    report.retained_player_match_summaries = state.player_match_summaries.len() as u32;
    report.retained_match_history_entries = state.match_history.len() as u32;
    report.retained_event_summaries = state.event_turn_summaries.len() as u32;
    report.retained_ledger_summaries = state.economy.turn_summaries.len() as u32;
    report.completed = !report.budget_exhausted
        && state.map.occupancy_rows.is_empty()
        && state.map.visibility_chunks.is_empty()
        && state.map.known_objects.is_empty()
        && state.battle.battles.is_empty()
        && state.battle.stacks.is_empty()
        && state.battle.occupancy.is_empty()
        && state.battle.obstacles.is_empty()
        && (report.raw_logs_retained
            || (state.aftermath_events.is_empty()
                && state.battle.events.is_empty()
                && state.battle.commands.is_empty()
                && state.economy.ledger_entries.is_empty()));
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
