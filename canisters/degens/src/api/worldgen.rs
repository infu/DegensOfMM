use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, CommandResponse, NavalRoutesView, ProceduralMapView, SiegeRulesView,
    SkirmishSettingsView,
};

#[query]
fn get_skirmish_settings(session_id: String) -> Result<SkirmishSettingsView, ApiError> {
    crate::metrics::benchmark_query("get_skirmish_settings", || {
        crate::services::worldgen::get_skirmish_settings(canic_cdk::api::msg_caller(), session_id)
    })
}

#[query]
fn get_procedural_map_state(session_id: String) -> Result<ProceduralMapView, ApiError> {
    crate::metrics::benchmark_query("get_procedural_map_state", || {
        crate::services::worldgen::get_procedural_map_state(
            canic_cdk::api::msg_caller(),
            session_id,
        )
    })
}

#[query]
fn get_naval_routes(session_id: String) -> Result<NavalRoutesView, ApiError> {
    crate::metrics::benchmark_query("get_naval_routes", || {
        crate::services::worldgen::get_naval_routes(canic_cdk::api::msg_caller(), session_id)
    })
}

#[query]
fn get_siege_rules(session_id: String) -> Result<SiegeRulesView, ApiError> {
    crate::metrics::benchmark_query("get_siege_rules", || {
        crate::services::worldgen::get_siege_rules(canic_cdk::api::msg_caller(), session_id)
    })
}

#[update]
fn sync_world_generation(
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_world_generation", || {
        crate::services::worldgen::sync_world_generation(
            canic_cdk::api::msg_caller(),
            session_id,
            client_nonce,
        )
    })
}
