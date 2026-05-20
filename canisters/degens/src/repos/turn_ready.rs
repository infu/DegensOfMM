//! Repository boundary for active map-turn readiness markers.

use domm_degens_schema::schema::{GameCommand, GameParticipant, GameSession, ParticipantTurnReady};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const TURN_READY_UNIQUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "turn_ready.by_session_participant_turn",
    entity: "ParticipantTurnReady",
    indexed_fields: &["session_id", "participant_id", "turn_number"],
    bounded_limit: Some(1),
};

pub(crate) const TURN_READY_BY_SESSION_TURN_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "turn_ready.by_session_turn",
    entity: "ParticipantTurnReady",
    indexed_fields: &["session_id", "turn_number"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TURN_READY_BY_COMMAND_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "turn_ready.by_command",
    entity: "ParticipantTurnReady",
    indexed_fields: &["command_id"],
    bounded_limit: Some(1),
};

pub(crate) fn create_turn_ready(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
    command_id: Option<Id<GameCommand>>,
    _ended_at: Timestamp,
) -> RepoResult<ParticipantTurnReady> {
    let input: Create<ParticipantTurnReady> = Create::<ParticipantTurnReady> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        turn_number: Some(turn_number),
        command_id: Some(command_id.map(|id| id.key())),
    };

    foundation::create("turn_ready.create_turn_ready", input)
}

pub(crate) struct MarkTurnReadyResult {
    pub ready: ParticipantTurnReady,
    pub created: bool,
}

pub(crate) fn mark_turn_ready(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
    command_id: Option<Id<GameCommand>>,
    ended_at: Timestamp,
) -> RepoResult<MarkTurnReadyResult> {
    if let Some(mut ready) = find_turn_ready(session_id, participant_id, turn_number)? {
        if ready.command_id.is_none() {
            ready.command_id = command_id.map(|id| id.key());
            return update_turn_ready(ready).map(|ready| MarkTurnReadyResult {
                ready,
                created: false,
            });
        }
        return Ok(MarkTurnReadyResult {
            ready,
            created: false,
        });
    }

    create_turn_ready(
        session_id,
        participant_id,
        turn_number,
        command_id,
        ended_at,
    )
    .map(|ready| MarkTurnReadyResult {
        ready,
        created: true,
    })
}

pub(crate) fn load_turn_ready(
    id: Id<ParticipantTurnReady>,
) -> RepoResult<Option<ParticipantTurnReady>> {
    foundation::load_by_id("turn_ready.load_turn_ready", id)
}

pub(crate) fn update_turn_ready(ready: ParticipantTurnReady) -> RepoResult<ParticipantTurnReady> {
    foundation::update("turn_ready.update_turn_ready", ready)
}

pub(crate) fn find_turn_ready(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
) -> RepoResult<Option<ParticipantTurnReady>> {
    foundation::storage_result(
        TURN_READY_UNIQUE_LOOKUP.name,
        crate::db()
            .load::<ParticipantTurnReady>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_turn_ready_by_session_turn(
    session_id: Id<GameSession>,
    turn_number: u32,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ParticipantTurnReady>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TURN_READY_BY_SESSION_TURN_LOOKUP.name,
        crate::db()
            .load::<ParticipantTurnReady>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("participant_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_turn_ready_by_command(
    command_id: Id<GameCommand>,
) -> RepoResult<Option<ParticipantTurnReady>> {
    foundation::storage_result(
        TURN_READY_BY_COMMAND_LOOKUP.name,
        crate::db()
            .load::<ParticipantTurnReady>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[cfg(test)]
pub(crate) fn turn_ready_session_turn_plan_text(
    session_id: Id<GameSession>,
    turn_number: u32,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        TURN_READY_BY_SESSION_TURN_LOOKUP.name,
        crate::db()
            .load::<ParticipantTurnReady>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("participant_id")
            .order_asc("id")
            .limit(limit),
    )
}
