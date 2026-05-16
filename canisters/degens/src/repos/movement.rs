//! Repository boundary for movement intents, movement snapshots, and turn-final path state.

use domm_degens_schema::schema::{
    Champion, GameCommand, GameParticipant, GameSession, MovementIntent, MovementSnapshot,
};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const MOVEMENT_INTENT_UNIQUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "movement.intent_by_champion_turn",
    entity: "MovementIntent",
    indexed_fields: &["session_id", "champion_id", "turn_number"],
    bounded_limit: Some(1),
};

pub(crate) const MOVEMENT_INTENTS_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "movement.intents_by_session_turn_status",
    entity: "MovementIntent",
    indexed_fields: &["session_id", "turn_number", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const MOVEMENT_SNAPSHOTS_BY_CHAMPION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "movement.snapshots_by_champion_turn",
    entity: "MovementSnapshot",
    indexed_fields: &["session_id", "turn_number", "champion_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const MOVEMENT_SNAPSHOT_UNIQUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "movement.snapshot_by_command_intent_step",
    entity: "MovementSnapshot",
    indexed_fields: &["command_id", "intent_id", "step_index"],
    bounded_limit: Some(1),
};

pub(crate) fn find_movement_intent(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    turn_number: u32,
) -> RepoResult<Option<MovementIntent>> {
    foundation::storage_result(
        MOVEMENT_INTENT_UNIQUE_LOOKUP.name,
        crate::db()
            .load::<MovementIntent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_movement_intents_by_status(
    session_id: Id<GameSession>,
    turn_number: u32,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<MovementIntent>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        MOVEMENT_INTENTS_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<MovementIntent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("champion_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn create_movement_intent(
    session_id: Id<GameSession>,
    turn_number: u32,
    actor_participant_id: Id<GameParticipant>,
    champion_id: Id<Champion>,
    command_id: Id<GameCommand>,
    status: String,
    path_json: String,
    path_hash: String,
) -> RepoResult<MovementIntent> {
    let input: Create<MovementIntent> = Create::<MovementIntent> {
        session_id: Some(session_id.key()),
        turn_number: Some(turn_number),
        actor_participant_id: Some(actor_participant_id.key()),
        champion_id: Some(champion_id.key()),
        command_id: Some(command_id.key()),
        status: Some(status),
        path_json: Some(path_json),
        path_hash: Some(path_hash),
        resolved_at: Some(None),
    };

    foundation::create("movement.create_intent", input)
}

pub(crate) fn update_movement_intent(intent: MovementIntent) -> RepoResult<MovementIntent> {
    foundation::update("movement.update_intent", intent)
}

pub(crate) fn mark_intent_resolved(mut intent: MovementIntent) -> RepoResult<MovementIntent> {
    intent.status = "resolved".to_string();
    intent.resolved_at = Some(Timestamp::now());
    update_movement_intent(intent)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_movement_snapshot(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    intent_id: Id<MovementIntent>,
    champion_id: Id<Champion>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
    step_index: u16,
    from_x: u16,
    from_y: u16,
    to_x: u16,
    to_y: u16,
    movement_cost: u16,
    remaining_after: u16,
    outcome: String,
    interaction_kind: Option<String>,
    interaction_id_text: Option<String>,
) -> RepoResult<MovementSnapshot> {
    let input: Create<MovementSnapshot> = Create::<MovementSnapshot> {
        session_id: Some(session_id.key()),
        command_id: Some(command_id.key()),
        intent_id: Some(intent_id.key()),
        champion_id: Some(champion_id.key()),
        participant_id: Some(participant_id.key()),
        turn_number: Some(turn_number),
        step_index: Some(step_index),
        from_x: Some(from_x),
        from_y: Some(from_y),
        to_x: Some(to_x),
        to_y: Some(to_y),
        movement_cost: Some(movement_cost),
        remaining_after: Some(remaining_after),
        outcome: Some(outcome),
        interaction_kind: Some(interaction_kind),
        interaction_id_text: Some(interaction_id_text),
    };

    foundation::create("movement.create_snapshot", input)
}

pub(crate) fn find_movement_snapshot(
    command_id: Id<GameCommand>,
    intent_id: Id<MovementIntent>,
    step_index: u16,
) -> RepoResult<Option<MovementSnapshot>> {
    foundation::storage_result(
        MOVEMENT_SNAPSHOT_UNIQUE_LOOKUP.name,
        crate::db()
            .load::<MovementSnapshot>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .filter(FieldRef::new("intent_id").eq(intent_id.key()))
            .filter(FieldRef::new("step_index").eq(step_index))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_movement_snapshots_for_champion_turn(
    session_id: Id<GameSession>,
    turn_number: u32,
    champion_id: Id<Champion>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<MovementSnapshot>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        MOVEMENT_SNAPSHOTS_BY_CHAMPION_LOOKUP.name,
        crate::db()
            .load::<MovementSnapshot>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("step_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn movement_intent_plan_text(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    turn_number: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        MOVEMENT_INTENT_UNIQUE_LOOKUP.name,
        crate::db()
            .load::<MovementIntent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn movement_snapshot_plan_text(
    session_id: Id<GameSession>,
    turn_number: u32,
    champion_id: Id<Champion>,
) -> RepoResult<String> {
    foundation::explain_text(
        MOVEMENT_SNAPSHOTS_BY_CHAMPION_LOOKUP.name,
        crate::db()
            .load::<MovementSnapshot>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("step_index")
            .order_asc("id")
            .limit(domm_game::MAX_LIST_LIMIT),
    )
}
