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
    QuestPreview, RecruitPreview, RecruitTarget, ScenarioRulesView, SessionView, SiegeRulesView,
    SkirmishSettingsView, TavernOffersView, Viewport, WorldEventsView,
};

pub use contract::{
    CanisterEndpointView, DeferredEndpointDecision, DiagnosticRowCount, DiagnosticStorageSnapshot,
    EndpointKind, EndpointSpec, REQUIRED_GAME_ENDPOINTS, deferred_endpoint_decisions,
    required_endpoint_views,
};

icydb::start!();

#[canic_cdk::init]
fn init() {
    if let Err(error) = services::system_jobs::repair_and_schedule_after_install_or_upgrade() {
        canic_cdk::eprintln!("system job init repair failed: {}", error.message);
    }
}

#[canic_cdk::post_upgrade]
fn post_upgrade() {
    if let Err(error) = services::system_jobs::repair_and_schedule_after_install_or_upgrade() {
        canic_cdk::eprintln!("system job post-upgrade repair failed: {}", error.message);
    }
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
