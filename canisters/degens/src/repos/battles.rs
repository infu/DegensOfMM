//! Repository boundary for battle state, stacks, occupancy, obstacles, and tactical events.

use domm_degens_schema::schema::{Battle, BattleOccupancy, BattleStack, Champion, GameSession};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const BATTLES_BY_SESSION_STATE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.by_session_state",
    entity: "Battle",
    indexed_fields: &["session_id", "state"],
    bounded_limit: Some(domm_game::MAX_ACTIVE_BATTLES_PER_SESSION),
};

pub(crate) const BATTLE_STACKS_BY_SIDE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.stacks_by_side",
    entity: "BattleStack",
    indexed_fields: &["battle_id", "side"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const BATTLE_OCCUPANCY_CELL_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.occupancy_by_cell",
    entity: "BattleOccupancy",
    indexed_fields: &["battle_id", "battle_x", "battle_y"],
    bounded_limit: Some(1),
};

pub(crate) fn page_battles_by_session_state(
    session_id: Id<GameSession>,
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Battle>> {
    let limit = foundation::validate_limit(
        "limit",
        limit,
        domm_game::MAX_ACTIVE_BATTLES_PER_SESSION,
        "active_battle_limit_exceeded",
    )?;
    foundation::execute_page(
        BATTLES_BY_SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("created_turn")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_battle_by_attacker(champion_id: Id<Champion>) -> RepoResult<Option<Battle>> {
    foundation::storage_result(
        "battles.by_attacker",
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("attacker_champion_id").eq(champion_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_battle_stacks_by_side(
    battle_id: Id<Battle>,
    side: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_STACKS_BY_SIDE_LOOKUP.name,
        crate::db()
            .load::<BattleStack>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("side").eq(side))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_battle_occupancy_cell(
    battle_id: Id<Battle>,
    battle_x: u8,
    battle_y: u8,
) -> RepoResult<Option<BattleOccupancy>> {
    foundation::storage_result(
        BATTLE_OCCUPANCY_CELL_LOOKUP.name,
        crate::db()
            .load::<BattleOccupancy>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("battle_x").eq(battle_x))
            .filter(FieldRef::new("battle_y").eq(battle_y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[cfg(test)]
pub(crate) fn active_battles_plan_text(
    session_id: Id<GameSession>,
    state: &str,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        BATTLES_BY_SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("created_turn")
            .order_asc("id")
            .limit(limit),
    )
}
