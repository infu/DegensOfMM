use canic_cdk::{query, update};

use crate::dto::public::{ApiError, CommandResponse, MoveCoord, MovementPreview};

#[query]
fn preview_move_path(
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    now_ms: u64,
) -> Result<MovementPreview, ApiError> {
    crate::services::movement::preview_move_path(
        canic_cdk::api::msg_caller(),
        session_id,
        champion_id,
        path,
        now_ms,
    )
}

#[update]
fn submit_move_intent(
    _session_id: String,
    _champion_id: String,
    _path: Vec<MoveCoord>,
    _client_nonce: String,
    _now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    crate::services::movement::unavailable("submit_move_intent")
}

#[update]
fn sync_session_turn(
    _session_id: String,
    _now_ms: u64,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::services::movement::unavailable("sync_session_turn")
}
