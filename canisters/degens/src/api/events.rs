use canic_cdk::query;

use crate::dto::public::{ApiError, ApiEventPage, CommandStatusView};

#[query]
fn get_events_after(
    _session_id: String,
    _audience_key: String,
    _events_after_seq: u64,
    _limit: u32,
) -> Result<ApiEventPage, ApiError> {
    crate::services::events::unavailable("get_events_after")
}

#[query]
fn get_command_status(
    _session_id: String,
    _command_id_or_client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    crate::services::events::unavailable("get_command_status")
}
