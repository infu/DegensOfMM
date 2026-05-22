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

use crate::repos::{sessions, turn_ready};

use super::{
    economy_expansion,
    session_turn_runtime::{self, SessionTurnRuntime},
};

pub(crate) enum TurnAdvanceRuntimeMode {
    HydrateRows,
    CarryPrevious { previous_turn: u32 },
}

pub(crate) struct TurnReadyMark {
    pub created: bool,
    pub subject_id: String,
    pub readiness: session_turn_runtime::RuntimeReadinessCounts,
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
    #[cfg(feature = "benchmark")]
    let runtime_only_advance =
        matches!(&runtime_mode, TurnAdvanceRuntimeMode::CarryPrevious { .. });
    let prepared_runtime = match runtime_mode {
        TurnAdvanceRuntimeMode::HydrateRows => prepare_active_turn(session)?,
        TurnAdvanceRuntimeMode::CarryPrevious { previous_turn } => {
            prepare_next_turn_from_previous(session, previous_turn)?
        }
    };
    #[cfg(feature = "benchmark")]
    if runtime_only_advance {
        if let Some(runtime) = prepared_runtime {
            insert_active_turn(runtime);
        }
        return Ok(());
    }
    *session = sessions::update_session(session.clone())?;
    if let Some(runtime) = prepared_runtime {
        insert_active_turn(runtime);
    }
    Ok(())
}

pub(crate) fn mark_participant_ready(
    session: &GameSession,
    participant: &GameParticipant,
    command_id: Id<GameCommand>,
    prefer_runtime: bool,
) -> Result<TurnReadyMark, ApiError> {
    let session_id = session.id().to_string();
    let participant_id = participant.id().to_string();
    if prefer_runtime
        && let Some(mark) =
            session_turn_runtime::with_runtime_mut(&session_id, session.current_turn, |runtime| {
                let created = runtime.mark_ready(participant_id.clone());
                runtime.readiness_counts().map(|readiness| TurnReadyMark {
                    created,
                    subject_id: participant_id.clone(),
                    readiness,
                })
            })
            .flatten()
    {
        return Ok(mark);
    }

    let ready_mark = turn_ready::mark_turn_ready(
        session.id(),
        participant.id(),
        session.current_turn,
        Some(command_id),
        Timestamp::now(),
    )?;
    Ok(TurnReadyMark {
        created: ready_mark.created,
        subject_id: ready_mark.ready.id().to_string(),
        readiness: durable_turn_readiness_counts(session)?,
    })
}

pub(crate) fn all_participants_ready(session: &GameSession) -> Result<bool, ApiError> {
    let session_id = session.id().to_string();
    if let Some(readiness) =
        session_turn_runtime::with_runtime(&session_id, session.current_turn, |runtime| {
            runtime.readiness_counts()
        })
        .flatten()
    {
        return Ok(readiness.all_ready);
    }

    Ok(durable_turn_readiness_counts(session)?.all_ready)
}

pub(crate) fn has_no_ready_participants(session: &GameSession) -> bool {
    let session_id = session.id().to_string();
    session_turn_runtime::with_runtime(&session_id, session.current_turn, |runtime| {
        runtime.ready_participants.is_empty()
    })
    .unwrap_or(false)
}

fn durable_turn_readiness_counts(
    session: &GameSession,
) -> Result<session_turn_runtime::RuntimeReadinessCounts, ApiError> {
    let participants = sessions::page_participants_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    if participants.items.is_empty() {
        return Ok(session_turn_runtime::RuntimeReadinessCounts {
            ready_count: 0,
            participant_count: 0,
            all_ready: false,
        });
    }
    let ready_rows = turn_ready::page_turn_ready_by_session_turn(
        session.id(),
        session.current_turn,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    Ok(session_turn_runtime::RuntimeReadinessCounts {
        ready_count: ready_rows.items.len(),
        participant_count: participants.items.len(),
        all_ready: ready_rows.items.len() >= participants.items.len(),
    })
}

fn turn_deadline() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(i64::try_from(domm_game::TURN_DURATION_MS).unwrap_or(i64::MAX)),
    )
}
