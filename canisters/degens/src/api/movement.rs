use canic_cdk::{query, update};

use crate::dto::public::{ApiError, CommandResponse, MoveCoord, MovementPreview};

#[query]
fn preview_move_path(
    _session_id: String,
    _champion_id: String,
    _path: Vec<MoveCoord>,
    _now_ms: u64,
) -> Result<MovementPreview, ApiError> {
    crate::services::movement::unavailable("preview_move_path")
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
