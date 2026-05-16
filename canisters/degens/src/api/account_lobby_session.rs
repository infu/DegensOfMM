use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, LobbyCommandResponse, ParticipantView, PlayerView, SessionView,
};

#[update]
fn register_player(
    username: Option<String>,
    display_name: Option<String>,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::register_player(
        canic_cdk::api::msg_caller(),
        username,
        display_name,
        client_nonce,
    )
}

#[query]
fn get_my_player() -> Result<PlayerView, ApiError> {
    crate::services::account_lobby_session::get_my_player(canic_cdk::api::msg_caller())
}

#[update]
fn create_session(
    name: String,
    ruleset_id: String,
    seed: u64,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::create_session(
        canic_cdk::api::msg_caller(),
        name,
        ruleset_id,
        seed,
        client_nonce,
    )
}

#[update]
fn join_session(
    session_id: String,
    faction_id: String,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::join_session(
        canic_cdk::api::msg_caller(),
        session_id,
        faction_id,
        client_nonce,
    )
}

#[update]
fn mark_ready(session_id: String, client_nonce: String) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::mark_ready(
        canic_cdk::api::msg_caller(),
        session_id,
        client_nonce,
    )
}

#[update]
fn start_session(
    session_id: String,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::start_session(
        canic_cdk::api::msg_caller(),
        session_id,
        client_nonce,
    )
}

#[query]
fn get_session(session_id: String) -> Result<SessionView, ApiError> {
    crate::services::account_lobby_session::get_session(session_id)
}

#[query]
fn get_my_participant(session_id: String) -> Result<ParticipantView, ApiError> {
    crate::services::account_lobby_session::get_my_participant(
        canic_cdk::api::msg_caller(),
        session_id,
    )
}
