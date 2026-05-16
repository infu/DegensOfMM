//! Controller-gated diagnostics endpoints belong here; public gameplay must not use SQL/DDL.

use canic_cdk::query;
use domm_game::ApiError;

use crate::contract::DiagnosticStorageSnapshot;

#[query]
fn get_diagnostic_storage_snapshot(
    entity_names: Vec<String>,
) -> Result<DiagnosticStorageSnapshot, ApiError> {
    crate::services::diagnostics::get_diagnostic_storage_snapshot(entity_names)
}
