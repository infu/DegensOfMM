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
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    client_nonce: String,
    now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    crate::services::movement::submit_move_intent(
        canic_cdk::api::msg_caller(),
        session_id,
        champion_id,
        path,
        client_nonce,
        now_ms,
    )
}

#[update]
fn sync_session_turn(
    session_id: String,
    now_ms: u64,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::services::movement::sync_session_turn(
        canic_cdk::api::msg_caller(),
        session_id,
        now_ms,
        client_nonce,
    )
}
