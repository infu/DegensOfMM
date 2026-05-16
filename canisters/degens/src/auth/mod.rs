//! Authentication and authorization helpers for caller principals and controller-only diagnostics.

use domm_game::ApiError;

pub(crate) fn require_controller(action: &str) -> Result<(), ApiError> {
    let caller = canic_cdk::api::msg_caller();
    if canic_cdk::api::is_controller(&caller) {
        return Ok(());
    }

    Err(ApiError::new(
        "controller_required",
        format!("{action} requires a controller caller"),
        false,
    ))
}
