//! Worldmap kernel facade.
//!
//! `SessionTurnRuntime` is the live store for active worldmap gameplay. This
//! module is the boundary new public/timer drivers should use so runtime-owned
//! state is not split across competing stores.
//!
//! Ownership is expected to grow here: active sessions, participants,
//! champions, map occupancy/contact indexes, visible/known objects, towns,
//! resources, quests, scenario state, deadlines, pending commands, runtime
//! receipts/events, dirty projection queues, and projection cursors.

use domm_degens_schema::schema::{GameCommand, GameParticipant, GameSession};
use domm_game::ApiError;
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::sessions;

use super::{
    economy_expansion,
    session_turn_runtime::{self, SessionTurnRuntime},
};

pub(crate) enum TurnAdvanceRuntimeMode {
    HydrateRows,
    CarryPrevious { previous_turn: u32 },
}

pub(crate) fn contains_active_turn_id(session_id: &str, turn_number: u32) -> bool {
    session_turn_runtime::contains_runtime(session_id, turn_number)
}

pub(crate) fn contains_active_turn(session: &GameSession) -> bool {
    contains_active_turn_id(&session.id().to_string(), session.current_turn)
}

pub(crate) fn latest_turn_number_for_session(session_id: &str) -> Option<u32> {
    session_turn_runtime::latest_turn_number_for_session(session_id)
}

pub(crate) fn caller_context_rows(
    caller_text: &str,
    session_id: &str,
) -> Option<(GameSession, GameParticipant)> {
    session_turn_runtime::caller_context_rows(caller_text, session_id)
}

pub(crate) fn insert_active_turn(runtime: SessionTurnRuntime) -> Option<SessionTurnRuntime> {
    session_turn_runtime::insert_runtime(runtime)
}

pub(crate) fn remove_active_turn_id(
    session_id: &str,
    turn_number: u32,
) -> Option<SessionTurnRuntime> {
    session_turn_runtime::remove_runtime(session_id, turn_number)
}

pub(crate) fn prepare_active_turn(
    session: &mut GameSession,
) -> Result<Option<SessionTurnRuntime>, ApiError> {
    session_turn_runtime::prepare_active_turn_runtime(session)
}

pub(crate) fn prepare_next_turn_from_previous(
    session: &mut GameSession,
    previous_turn: u32,
) -> Result<Option<SessionTurnRuntime>, ApiError> {
    session_turn_runtime::prepare_active_turn_runtime_from_previous(session, previous_turn)
}

pub(crate) fn ensure_active_turn(session: &mut GameSession) -> Result<(), ApiError> {
    session_turn_runtime::ensure_active_turn_runtime(session)
}

pub(crate) fn advance_turn(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    income_turn: u32,
    runtime_mode: TurnAdvanceRuntimeMode,
) -> Result<(), ApiError> {
    session.current_turn = session.current_turn.saturating_add(1);
    session.turn_started_at = Timestamp::now();
    session.turn_deadline_at = turn_deadline();
    session.last_command_id = Some(command_id.key());
    if domm_game::week_for_turn(session.current_turn) != domm_game::week_for_turn(income_turn) {
        economy_expansion::materialize_weekly_economy(session, command_id)?;
    }
    let prepared_runtime = match runtime_mode {
        TurnAdvanceRuntimeMode::HydrateRows => prepare_active_turn(session)?,
        TurnAdvanceRuntimeMode::CarryPrevious { previous_turn } => {
            prepare_next_turn_from_previous(session, previous_turn)?
        }
    };
    *session = sessions::update_session(session.clone())?;
    if let Some(runtime) = prepared_runtime {
        insert_active_turn(runtime);
    }
    Ok(())
}

fn turn_deadline() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(i64::try_from(domm_game::TURN_DURATION_MS).unwrap_or(i64::MAX)),
    )
}
