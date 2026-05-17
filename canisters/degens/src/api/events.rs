use canic_cdk::query;

use crate::dto::public::{ApiError, ApiEventPage, CommandStatusView};

#[query]
fn get_events_after(
    session_id: String,
    audience_key: String,
    events_after_seq: u64,
    limit: u32,
) -> Result<ApiEventPage, ApiError> {
    crate::services::events::get_events_after(
        canic_cdk::api::msg_caller(),
        session_id,
        audience_key,
        events_after_seq,
        limit,
    )
}

#[query]
fn get_command_status(
    session_id: String,
    command_id_or_client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    crate::services::events::get_command_status(
        canic_cdk::api::msg_caller(),
        session_id,
        command_id_or_client_nonce,
    )
}

#[query]
fn get_command_status_by_nonce(
    session_id: String,
    command_type: String,
    client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    crate::services::events::get_command_status_by_nonce(
        canic_cdk::api::msg_caller(),
        session_id,
        command_type,
        client_nonce,
    )
}
