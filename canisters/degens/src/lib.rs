//! Degens canister entrypoint.

extern crate canic_cdk as ic_cdk;

mod api;
mod auth;
mod contract;
mod dto;
mod metrics;
mod repos;
mod services;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
use crate::dto::public::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview,
    ChampionHirePreview, ChampionMagicReceipt, ChampionProgressionView, ChampionView,
    CommandResponse, CommandStatusView, ContentManifestResponse, DwellingPoolView,
    DwellingRecruitPreview, GameView, GameViewRequest, LobbyCommandResponse, MapChunkPage,
    MarketTradePreview, MatchHistoryPage, MoveCoord, MovementPreview, NavalRoutesView, ObjectView,
    ObjectViewPage, ObjectiveProgressView, ParticipantView, PlayerView, ProceduralMapView,
    QuestPreview, RecruitPreview, RecruitTarget, ScenarioRulesView, SessionView, SetupProgressView,
    SiegeRulesView, SkirmishSettingsView, TavernOffersView, Viewport, WorldEventsView,
};

pub use contract::{
    CanisterEndpointView, DeferredEndpointDecision, DiagnosticBenchmarkCallPage,
    DiagnosticBenchmarkCallView, DiagnosticBenchmarkRepoOpView, DiagnosticRowCount,
    DiagnosticStorageSnapshot, DiagnosticSystemJobPage, DiagnosticSystemJobView, EndpointKind,
    EndpointSpec, REQUIRED_GAME_ENDPOINTS, deferred_endpoint_decisions, required_endpoint_views,
};
#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
pub use contract::{
    DiagnosticProjectionFlushView, DiagnosticProjectionKernelView, DiagnosticProjectionSnapshot,
};

icydb::start!();

#[canic_cdk::init]
fn init() {
    if let Err(_error) = services::account_lobby_session::repair_first_playable_content_cache() {}
    if let Err(_error) = services::account_lobby_session::repair_active_session_admission_cache() {}
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn pre_upgrade_impl() {
    #[cfg(not(feature = "benchmark"))]
    {
        if let Err(error) = services::session_turn_runtime::persist_snapshot_for_upgrade() {
            panic!("session turn runtime pre-upgrade snapshot failed: {error}");
        }
        if let Err(error) =
            services::flush_barrier::flush_barrier(services::flush_barrier::FLUSH_BARRIER_UPGRADE)
        {
            panic!("upgrade flush barrier failed: {}", error.message);
        }
        if let Err(error) = services::session_turn_runtime::persist_snapshot_for_upgrade() {
            panic!("session turn runtime post-barrier snapshot failed: {error}");
        }
    }
    #[cfg(all(feature = "benchmark", feature = "projection-benchmark"))]
    {
        if let Err(error) = services::session_turn_runtime::persist_snapshot_for_upgrade() {
            panic!("session turn runtime benchmark pre-upgrade snapshot failed: {error}");
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(export_name = "canister_pre_upgrade")]
extern "C" fn canister_pre_upgrade() {
    pre_upgrade_impl();
}

#[canic_cdk::post_upgrade]
fn post_upgrade() {
    if let Err(error) = services::battle_runtime::restore_snapshot_after_upgrade() {
        panic!("battle runtime post-upgrade restore failed: {error}");
    }
    #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
    if let Err(error) = services::session_turn_runtime::restore_snapshot_after_upgrade() {
        panic!("session turn runtime post-upgrade restore failed: {error}");
    }
    if let Err(_error) = services::account_lobby_session::repair_active_session_admission_cache() {}
    if let Err(_error) = services::account_lobby_session::repair_first_playable_content_cache() {}
}

#[allow(dead_code)]
fn icydb_admin_sql_load_default() -> Result<(), icydb::Error> {
    Ok(())
}

canic_cdk::export_candid!();

#[cfg(test)]
pub fn exported_candid_text_for_tests() -> String {
    use std::ffi::CString;

    let ptr = get_candid_pointer();
    assert!(!ptr.is_null(), "exported candid pointer must not be null");
    unsafe { CString::from_raw(ptr) }
        .into_string()
        .expect("exported Candid should be valid UTF-8")
}
