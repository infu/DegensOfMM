use std::collections::BTreeSet;

use candid::Principal as CandidPrincipal;
use domm_game::{ApiError, MoveCoord, MovementPreview};
use icydb::traits::EntityValue;

use super::{
    render_projection,
    session_context::{self, public_error},
};

pub(crate) fn preview_move_path(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    _now_ms: u64,
) -> Result<MovementPreview, ApiError> {
    if path.len() > domm_game::MAX_MOVE_PATH_STEPS_LIMIT {
        return Err(ApiError::new(
            "movement_path_too_long",
            "movement path exceeds the v1 public query limit",
            false,
        )
        .with_details(format!(
            r#"{{"path_len":{},"max_len":{}}}"#,
            path.len(),
            domm_game::MAX_MOVE_PATH_STEPS_LIMIT
        )));
    }

    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let champion = render_projection::champion_view_by_id(&context, &champion_id)?;
    if champion.redacted || champion.owner_participant_id != context.participant.id().to_string() {
        return Err(public_error(
            "not_champion_owner",
            "caller does not own this champion",
            false,
        ));
    }
    validate_path_bounds(&context.session, &path)?;
    let chunks_touched = chunks_touched(&context.session, &path);

    Ok(MovementPreview {
        champion_id: champion.champion_id,
        participant_id: context.participant.id().to_string(),
        turn_number: context.session.current_turn,
        total_cost: u16::try_from(path.len()).unwrap_or(u16::MAX),
        available_movement: champion.effective_movement,
        chunks_touched,
        path,
        stop: None,
    })
}

fn validate_path_bounds(
    session: &domm_degens_schema::schema::GameSession,
    path: &[MoveCoord],
) -> Result<(), ApiError> {
    for coord in path {
        if coord.x >= session.map_width || coord.y >= session.map_height {
            return Err(public_error(
                "movement_path_out_of_bounds",
                "movement path leaves the session map",
                false,
            ));
        }
    }
    Ok(())
}

fn chunks_touched(session: &domm_degens_schema::schema::GameSession, path: &[MoveCoord]) -> u32 {
    let chunk_size = u16::from(session.chunk_size);
    path.iter()
        .map(|coord| (coord.x / chunk_size, coord.y / chunk_size))
        .collect::<BTreeSet<_>>()
        .len()
        .try_into()
        .unwrap_or(u32::MAX)
}

pub(crate) use super::repository_not_implemented as unavailable;
