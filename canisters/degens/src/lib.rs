//! Degens canister entrypoint.

extern crate canic_cdk as ic_cdk;

mod contract;
mod endpoints;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
use domm_game::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview, ChampionView,
    CommandResponse, CommandStatusView, ContentManifestResponse, GameView, GameViewRequest,
    LobbyCommandResponse, MapChunkPage, MatchHistoryPage, MoveCoord, MovementPreview,
    ObjectViewPage, ParticipantView, PlayerView, RecruitPreview, RecruitTarget, SessionView,
    Viewport,
};

pub use contract::{
    CanisterEndpointView, DeferredEndpointDecision, EndpointKind, EndpointSpec,
    REQUIRED_GAME_ENDPOINTS, deferred_endpoint_decisions, required_endpoint_views,
};

icydb::start!();

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
