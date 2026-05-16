use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, LobbyCommandResponse, ParticipantView, PlayerView, SessionView,
};

#[update]
fn register_player(
    _username: Option<String>,
    _display_name: Option<String>,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::unavailable("register_player")
}

#[query]
fn get_my_player() -> Result<PlayerView, ApiError> {
    crate::services::account_lobby_session::unavailable("get_my_player")
}

#[update]
fn create_session(
    _name: String,
    _ruleset_id: String,
    _seed: u64,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::unavailable("create_session")
}

#[update]
fn join_session(
    _session_id: String,
    _faction_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::unavailable("join_session")
}

#[update]
fn mark_ready(
    _session_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::unavailable("mark_ready")
}

#[update]
fn start_session(
    _session_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    crate::services::account_lobby_session::unavailable("start_session")
}

#[query]
fn get_session(_session_id: String) -> Result<SessionView, ApiError> {
    crate::services::account_lobby_session::unavailable("get_session")
}

#[query]
fn get_my_participant(_session_id: String) -> Result<ParticipantView, ApiError> {
    crate::services::account_lobby_session::unavailable("get_my_participant")
}
