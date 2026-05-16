//! Repository boundary for towns, buildings, recruit pools, and garrisons.

use domm_degens_schema::schema::{
    GameParticipant, GameSession, Town, TownBuilding, TownGarrisonStack, TownRecruitPool,
};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const TOWNS_BY_OWNER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.by_owner",
    entity: "Town",
    indexed_fields: &["session_id", "owner_participant_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_BUILDINGS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.buildings_by_town",
    entity: "TownBuilding",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_RECRUIT_POOLS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.recruit_pools_by_town",
    entity: "TownRecruitPool",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_GARRISON_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.garrison_by_town",
    entity: "TownGarrisonStack",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn page_towns_by_owner(
    session_id: Id<GameSession>,
    owner_participant_id: Id<GameParticipant>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Town>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TOWNS_BY_OWNER_LOOKUP.name,
        crate::db()
            .load::<Town>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("owner_participant_id").eq(owner_participant_id.key()))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_town_buildings(
    town_id: Id<Town>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<TownBuilding>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TOWN_BUILDINGS_LOOKUP.name,
        crate::db()
            .load::<TownBuilding>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("building_def_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_town_recruit_pools(
    town_id: Id<Town>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<TownRecruitPool>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TOWN_RECRUIT_POOLS_LOOKUP.name,
        crate::db()
            .load::<TownRecruitPool>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("unit_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_town_garrison(
    town_id: Id<Town>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<TownGarrisonStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TOWN_GARRISON_LOOKUP.name,
        crate::db()
            .load::<TownGarrisonStack>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn towns_by_owner_plan_text(
    session_id: Id<GameSession>,
    owner_participant_id: Id<GameParticipant>,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        TOWNS_BY_OWNER_LOOKUP.name,
        crate::db()
            .load::<Town>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("owner_participant_id").eq(owner_participant_id.key()))
            .order_asc("id")
            .limit(limit),
    )
}
