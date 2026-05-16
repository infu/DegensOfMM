use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, ApiTownView, BuildPreview, CommandResponse, RecruitPreview, RecruitTarget,
};

#[query]
fn get_town_view(session_id: String, town_id: String) -> Result<ApiTownView, ApiError> {
    crate::services::town::get_town_view(canic_cdk::api::msg_caller(), session_id, town_id)
}

#[query]
fn preview_build_town_structure(
    _session_id: String,
    _town_id: String,
    _building_def_id: String,
) -> Result<BuildPreview, ApiError> {
    crate::services::town::unavailable("preview_build_town_structure")
}

#[query]
fn preview_recruit_units(
    _session_id: String,
    _town_id: String,
    _unit_id: String,
    _quantity: u32,
    _target: RecruitTarget,
) -> Result<RecruitPreview, ApiError> {
    crate::services::town::unavailable("preview_recruit_units")
}

#[update]
fn submit_build_town_structure(
    _session_id: String,
    _town_id: String,
    _building_def_id: String,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::services::town::unavailable("submit_build_town_structure")
}

#[update]
fn submit_recruit_units(
    _session_id: String,
    _town_id: String,
    _unit_id: String,
    _quantity: u32,
    _target: RecruitTarget,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::services::town::unavailable("submit_recruit_units")
}
