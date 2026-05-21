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

use domm_degens_schema::schema::{GameParticipant, GameSession};
use domm_game::ApiError;
use icydb::traits::EntityValue;

use super::session_turn_runtime::{self, SessionTurnRuntime};

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
