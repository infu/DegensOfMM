//! Repository boundary for neutral armies, guard encounters, and neutral stack rows.

use domm_degens_schema::schema::{GameSession, NeutralArmy, NeutralArmyStack, UnitDefinition};
use icydb::{Create, db::query::FieldRef, types::Id};

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

pub(crate) const NEUTRAL_COORD_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "neutrals.by_session_xy",
    entity: "NeutralArmy",
    indexed_fields: &["session_id", "x", "y"],
    bounded_limit: Some(1),
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_neutral_army(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    state: String,
    aggression: String,
    growth_rule_key: String,
    last_growth_week: u32,
) -> RepoResult<NeutralArmy> {
    let input: Create<NeutralArmy> = Create::<NeutralArmy> {
        session_id: Some(session_id.key()),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        state: Some(state),
        aggression: Some(aggression),
        growth_rule_key: Some(growth_rule_key),
        last_growth_week: Some(last_growth_week),
        last_command_id: Some(None),
    };

    foundation::create("neutrals.create_neutral_army", input)
}

pub(crate) fn load_neutral_army(id: Id<NeutralArmy>) -> RepoResult<Option<NeutralArmy>> {
    foundation::load_by_id("neutrals.load_neutral_army", id)
}

pub(crate) fn find_neutral_army_by_session_xy(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
) -> RepoResult<Option<NeutralArmy>> {
    foundation::storage_result(
        NEUTRAL_COORD_LOOKUP.name,
        crate::db()
            .load::<NeutralArmy>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("x").eq(x))
            .filter(FieldRef::new("y").eq(y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

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

pub(crate) fn find_neutral_army_stack(
    neutral_army_id: Id<NeutralArmy>,
    slot_index: u8,
) -> RepoResult<Option<NeutralArmyStack>> {
    foundation::storage_result(
        "neutrals.stack_by_army_slot",
        crate::db()
            .load::<NeutralArmyStack>()
            .filter(FieldRef::new("neutral_army_id").eq(neutral_army_id.key()))
            .filter(FieldRef::new("slot_index").eq(slot_index))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_neutral_army_stack(
    session_id: Id<GameSession>,
    neutral_army_id: Id<NeutralArmy>,
    unit_id: Id<UnitDefinition>,
    slot_index: u8,
    quantity: u32,
    front_hp: u16,
) -> RepoResult<NeutralArmyStack> {
    let input: Create<NeutralArmyStack> = Create::<NeutralArmyStack> {
        session_id: Some(session_id.key()),
        neutral_army_id: Some(neutral_army_id.key()),
        unit_id: Some(unit_id.key()),
        slot_index: Some(slot_index),
        quantity: Some(quantity),
        front_hp: Some(front_hp),
        last_command_id: Some(None),
    };

    foundation::create("neutrals.create_neutral_army_stack", input)
}
