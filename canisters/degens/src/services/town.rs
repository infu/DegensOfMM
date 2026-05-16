use candid::Principal as CandidPrincipal;
use domm_game::{ApiError, ApiTownView};

use super::{render_projection, session_context};

pub(crate) fn get_town_view(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
) -> Result<ApiTownView, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    render_projection::town_view_by_id(&context, &town_id)
}

pub(crate) use super::repository_not_implemented as unavailable;
