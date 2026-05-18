//! Controller-gated diagnostics endpoints belong here; public gameplay must not use SQL/DDL.

use canic_cdk::{query, update};
use domm_game::ApiError;

use crate::contract::{
    DiagnosticStorageSnapshot, DiagnosticSystemJobPage, DiagnosticSystemJobView,
};

#[cfg(feature = "benchmark")]
use crate::contract::DiagnosticBenchmarkCallPage;

#[query]
fn get_diagnostic_storage_snapshot(
    entity_names: Vec<String>,
) -> Result<DiagnosticStorageSnapshot, ApiError> {
    crate::metrics::benchmark_query("get_diagnostic_storage_snapshot", || {
        crate::services::diagnostics::get_diagnostic_storage_snapshot(entity_names)
    })
}

#[query]
fn get_diagnostic_system_jobs(
    session_id: Option<String>,
    status: Option<String>,
    limit: u32,
    cursor: Option<String>,
) -> Result<DiagnosticSystemJobPage, ApiError> {
    crate::metrics::benchmark_query("get_diagnostic_system_jobs", || {
        crate::services::diagnostics::get_diagnostic_system_jobs(session_id, status, limit, cursor)
    })
}

#[cfg(feature = "benchmark")]
#[query]
fn get_diagnostic_benchmark_metrics(
    cursor: Option<u64>,
    limit: u32,
) -> Result<DiagnosticBenchmarkCallPage, ApiError> {
    crate::metrics::benchmark_query("get_diagnostic_benchmark_metrics", || {
        crate::services::diagnostics::get_diagnostic_benchmark_metrics(cursor, limit)
    })
}

#[cfg(feature = "benchmark")]
#[update]
fn reset_diagnostic_benchmark_metrics() -> Result<(), ApiError> {
    crate::services::diagnostics::reset_diagnostic_benchmark_metrics()
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

#[update]
fn run_diagnostic_system_job(job_key: String) -> Result<u32, ApiError> {
    crate::services::diagnostics::run_diagnostic_system_job(job_key)
}
