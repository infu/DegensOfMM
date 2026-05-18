use canic_cdk::{query, update};

use crate::dto::public::{ApiError, CommandResponse, MoveCoord, MovementPreview};

#[query]
fn preview_move_path(
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
) -> Result<MovementPreview, ApiError> {
    crate::metrics::benchmark_query("preview_move_path", || {
        crate::services::movement::preview_move_path(
            canic_cdk::api::msg_caller(),
            session_id,
            champion_id,
            path,
            crate::services::clock::now_ms(),
        )
    })
}

#[update]
fn submit_move_intent(
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_move_intent", || {
        crate::services::movement::submit_move_intent(
            canic_cdk::api::msg_caller(),
            session_id,
            champion_id,
            path,
            client_nonce,
            crate::services::clock::now_ms(),
        )
    })
}

#[update]
fn end_turn(session_id: String, client_nonce: String) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("end_turn", || {
        crate::services::movement::end_turn(canic_cdk::api::msg_caller(), session_id, client_nonce)
    })
}

#[update]
fn sync_session_turn(
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_session_turn", || {
        crate::services::movement::sync_session_turn(
            canic_cdk::api::msg_caller(),
            session_id,
            crate::services::clock::now_ms(),
            client_nonce,
        )
    })
}
