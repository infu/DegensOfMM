use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, CommandResponse, ObjectiveProgressView, QuestPreview, ScenarioRulesView,
    WorldEventsView,
};

#[query]
fn get_objective_progress(session_id: String) -> Result<ObjectiveProgressView, ApiError> {
    crate::metrics::benchmark_query("get_objective_progress", || {
        crate::services::scenario_progress::get_objective_progress(
            canic_cdk::api::msg_caller(),
            session_id,
        )
    })
}

#[query]
fn get_scenario_rules(session_id: String) -> Result<ScenarioRulesView, ApiError> {
    crate::metrics::benchmark_query("get_scenario_rules", || {
        crate::services::scenario_progress::get_scenario_rules(
            canic_cdk::api::msg_caller(),
            session_id,
        )
    })
}

#[query]
fn get_world_events(session_id: String) -> Result<WorldEventsView, ApiError> {
    crate::metrics::benchmark_query("get_world_events", || {
        crate::services::scenario_progress::get_world_events(
            canic_cdk::api::msg_caller(),
            session_id,
        )
    })
}

#[query]
fn preview_quest(session_id: String, quest_key: String) -> Result<QuestPreview, ApiError> {
    crate::metrics::benchmark_query("preview_quest", || {
        crate::services::scenario_progress::preview_quest(
            canic_cdk::api::msg_caller(),
            session_id,
            quest_key,
        )
    })
}

#[update]
fn accept_quest(
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("accept_quest", || {
        crate::services::scenario_progress::accept_quest(
            canic_cdk::api::msg_caller(),
            session_id,
            quest_key,
            client_nonce,
        )
    })
}

#[update]
fn claim_quest_reward(
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("claim_quest_reward", || {
        crate::services::scenario_progress::claim_quest_reward(
            canic_cdk::api::msg_caller(),
            session_id,
            quest_key,
            client_nonce,
        )
    })
}

#[update]
fn sync_objectives(session_id: String, client_nonce: String) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_objectives", || {
        crate::services::scenario_progress::sync_objectives(
            canic_cdk::api::msg_caller(),
            session_id,
            client_nonce,
        )
    })
}

#[update]
fn sync_world_events(
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_world_events", || {
        crate::services::scenario_progress::sync_world_events(
            canic_cdk::api::msg_caller(),
            session_id,
            client_nonce,
        )
    })
}

#[update]
fn sync_advanced_victory(
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_advanced_victory", || {
        crate::services::scenario_progress::sync_advanced_victory(
            canic_cdk::api::msg_caller(),
            session_id,
            client_nonce,
        )
    })
}
