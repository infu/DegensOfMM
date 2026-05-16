//! Repository boundary for neutral armies, guard encounters, and neutral stack rows.

use domm_degens_schema::schema::{GameSession, NeutralArmy, NeutralArmyStack};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const NEUTRALS_BY_CHUNK_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "neutrals.by_chunk_status",
    entity: "NeutralArmy",
    indexed_fields: &["session_id", "chunk_x", "chunk_y", "state"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const NEUTRAL_STACKS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "neutrals.stacks_by_army",
    entity: "NeutralArmyStack",
    indexed_fields: &["neutral_army_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn page_neutral_armies_by_chunk_state(
    session_id: Id<GameSession>,
    chunk_x: u16,
    chunk_y: u16,
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<NeutralArmy>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        NEUTRALS_BY_CHUNK_STATUS_LOOKUP.name,
        crate::db()
            .load::<NeutralArmy>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_neutral_army_stacks(
    neutral_army_id: Id<NeutralArmy>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<NeutralArmyStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        NEUTRAL_STACKS_LOOKUP.name,
        crate::db()
            .load::<NeutralArmyStack>()
            .filter(FieldRef::new("neutral_army_id").eq(neutral_army_id.key()))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}
