//! Repository boundary for movement intents, movement snapshots, and turn-final path state.

use domm_degens_schema::schema::{Champion, GameSession, MovementIntent};
use icydb::{db::query::FieldRef, types::Id};

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
