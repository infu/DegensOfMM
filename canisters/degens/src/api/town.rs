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
    session_id: String,
    town_id: String,
    building_def_id: String,
) -> Result<BuildPreview, ApiError> {
    crate::services::town::preview_build_town_structure(
        canic_cdk::api::msg_caller(),
        session_id,
        town_id,
        building_def_id,
    )
}

#[query]
fn preview_recruit_units(
    session_id: String,
    town_id: String,
    unit_id: String,
    quantity: u32,
    target: RecruitTarget,
) -> Result<RecruitPreview, ApiError> {
    crate::services::town::preview_recruit_units(
        canic_cdk::api::msg_caller(),
        session_id,
        town_id,
        unit_id,
        quantity,
        target,
    )
}

#[update]
fn submit_build_town_structure(
    session_id: String,
    town_id: String,
    building_def_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_build_town_structure", || {
        crate::services::town::submit_build_town_structure(
            canic_cdk::api::msg_caller(),
            session_id,
            town_id,
            building_def_id,
            client_nonce,
        )
    })
}

#[update]
fn submit_recruit_units(
    session_id: String,
    town_id: String,
    unit_id: String,
    quantity: u32,
    target: RecruitTarget,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_recruit_units", || {
        crate::services::town::submit_recruit_units(
            canic_cdk::api::msg_caller(),
            session_id,
            town_id,
            unit_id,
            quantity,
            target,
            client_nonce,
        )
    })
}
