use crate::playable::run_first_playable_backend_gate;

use super::types::{
    MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION, MAX_EVENTS_RETAINED_PER_ACTIVE_SESSION,
    MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION, PerformanceBudgetError,
    PerformanceBudgetReport,
};

type PerformanceBudgetResult = Result<PerformanceBudgetReport, PerformanceBudgetError>;

pub fn measure_first_playable_performance() -> PerformanceBudgetResult {
    let report = run_first_playable_backend_gate()?;
    let estimated_response_bytes = report.max_query_bytes;
    let storage_active_budget = (MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION
        + MAX_EVENTS_RETAINED_PER_ACTIVE_SESSION
        + MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION)
        as u32;

    Ok(PerformanceBudgetReport {
        command_count: report.command_count,
        event_count: report.event_count,
        query_count: report.query_count,
        storage_row_count: report.storage_row_count,
        max_query_bytes: report.max_query_bytes,
        estimated_response_bytes,
        max_query_under_budget: report.max_query_bytes <= 16_384,
        storage_under_active_row_budget: report.storage_row_count < storage_active_budget,
        concerns: report.concerns,
    })
}
