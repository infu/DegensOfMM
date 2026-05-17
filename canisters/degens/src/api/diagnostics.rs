//! Controller-gated diagnostics endpoints belong here; public gameplay must not use SQL/DDL.

use canic_cdk::{query, update};
use domm_game::ApiError;

use crate::contract::{
    DiagnosticStorageSnapshot, DiagnosticSystemJobPage, DiagnosticSystemJobView,
};

#[query]
fn get_diagnostic_storage_snapshot(
    entity_names: Vec<String>,
) -> Result<DiagnosticStorageSnapshot, ApiError> {
    crate::services::diagnostics::get_diagnostic_storage_snapshot(entity_names)
}

#[query]
fn get_diagnostic_system_jobs(
    session_id: Option<String>,
    status: Option<String>,
    limit: u32,
    cursor: Option<String>,
) -> Result<DiagnosticSystemJobPage, ApiError> {
    crate::services::diagnostics::get_diagnostic_system_jobs(session_id, status, limit, cursor)
}

#[update]
fn force_diagnostic_system_job_running(
    job_key: String,
    lease_expires_at_ms: u64,
) -> Result<DiagnosticSystemJobView, ApiError> {
    crate::services::diagnostics::force_diagnostic_system_job_running(job_key, lease_expires_at_ms)
}

#[update]
fn run_diagnostic_system_jobs(max_ticks: u32) -> Result<u32, ApiError> {
    crate::services::diagnostics::run_diagnostic_system_jobs(max_ticks)
}
