//! Repository boundary for towns, buildings, recruit pools, and garrisons.

use domm_degens_schema::schema::{
    BuildingDefinition, FactionDefinition, GameParticipant, GameSession, Town, TownBuilding,
    TownGarrisonStack, TownRecruitPool, UnitDefinition,
};
use icydb::{Create, db::query::FieldRef, traits::EntityValue, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const TOWNS_BY_OWNER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.by_owner",
    entity: "Town",
    indexed_fields: &["session_id", "owner_participant_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWNS_BY_SESSION_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.by_session_status",
    entity: "Town",
    indexed_fields: &["session_id", "chunk_x", "chunk_y", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_BUILDINGS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.buildings_by_town",
    entity: "TownBuilding",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_BUILDING_DEFINITION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.building_by_town_definition",
    entity: "TownBuilding",
    indexed_fields: &["town_id", "building_def_id"],
    bounded_limit: Some(1),
};

pub(crate) const TOWN_RECRUIT_POOLS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.recruit_pools_by_town",
    entity: "TownRecruitPool",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_RECRUIT_POOL_UNIT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.recruit_pool_by_town_unit",
    entity: "TownRecruitPool",
    indexed_fields: &["town_id", "unit_id"],
    bounded_limit: Some(1),
};

pub(crate) const TOWN_GARRISON_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.garrison_by_town",
    entity: "TownGarrisonStack",
    indexed_fields: &["town_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const TOWN_GARRISON_SLOT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.garrison_by_town_slot",
    entity: "TownGarrisonStack",
    indexed_fields: &["town_id", "slot_index"],
    bounded_limit: Some(1),
};

pub(crate) const TOWN_COORD_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "towns.by_session_xy",
    entity: "Town",
    indexed_fields: &["session_id", "x", "y"],
    bounded_limit: Some(1),
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_town(
    session_id: Id<GameSession>,
    owner_participant_id: Option<Id<GameParticipant>>,
    faction_id: Id<FactionDefinition>,
    name: String,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    status: String,
    hall_level: u8,
    fort_level: u8,
    last_built_turn: u32,
    captured_turn: u32,
    income_started_turn: u32,
    unrest_until_turn: u32,
) -> RepoResult<Town> {
    let input: Create<Town> = Create::<Town> {
        session_id: Some(session_id.key()),
        owner_participant_id: Some(owner_participant_id.map(|id| id.key())),
        faction_id: Some(faction_id.key()),
        name: Some(name),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        status: Some(status),
        hall_level: Some(hall_level),
        fort_level: Some(fort_level),
        last_built_turn: Some(last_built_turn),
        captured_turn: Some(captured_turn),
        income_started_turn: Some(income_started_turn),
        unrest_until_turn: Some(unrest_until_turn),
        last_command_id: Some(None),
    };

    foundation::create("towns.create_town", input)
}

pub(crate) fn load_town(id: Id<Town>) -> RepoResult<Option<Town>> {
    foundation::load_by_id("towns.load_town", id)
}

pub(crate) fn update_town(town: Town) -> RepoResult<Town> {
    foundation::update("towns.update_town", town)
}

pub(crate) fn find_town_by_session_xy(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
) -> RepoResult<Option<Town>> {
    foundation::storage_result(
        TOWN_COORD_LOOKUP.name,
        crate::db()
            .load::<Town>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("x").eq(x))
            .filter(FieldRef::new("y").eq(y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

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

pub(crate) fn page_towns_by_session_status(
    session_id: Id<GameSession>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Town>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        TOWNS_BY_SESSION_STATUS_LOOKUP.name,
        crate::db()
            .load::<Town>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("chunk_y")
            .order_asc("chunk_x")
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
            .order_asc("town_id")
            .order_asc("building_def_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_town_buildings(town_id: Id<Town>, limit: u32) -> RepoResult<Vec<TownBuilding>> {
    let limit = foundation::validate_list_limit(limit)?;
    let mut rows = foundation::storage_result(
        TOWN_BUILDINGS_LOOKUP.name,
        crate::db()
            .load::<TownBuilding>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit)
            .entities(),
    )?;
    rows.sort_by(|left, right| {
        left.building_def_id
            .cmp(&right.building_def_id)
            .then_with(|| left.id().cmp(&right.id()))
    });
    Ok(rows)
}

pub(crate) fn find_town_building(
    town_id: Id<Town>,
    building_def_id: Id<BuildingDefinition>,
) -> RepoResult<Option<TownBuilding>> {
    foundation::storage_result(
        TOWN_BUILDING_DEFINITION_LOOKUP.name,
        crate::db()
            .load::<TownBuilding>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("building_def_id").eq(building_def_id.key()))
            .order_asc("town_id")
            .order_asc("building_def_id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_town_building(
    session_id: Id<GameSession>,
    town_id: Id<Town>,
    building_def_id: Id<BuildingDefinition>,
    building_slug: String,
    built_turn: u32,
) -> RepoResult<TownBuilding> {
    let input: Create<TownBuilding> = Create::<TownBuilding> {
        session_id: Some(session_id.key()),
        town_id: Some(town_id.key()),
        building_def_id: Some(building_def_id.key()),
        building_slug: Some(building_slug),
        built_turn: Some(built_turn),
    };

    foundation::create("towns.create_town_building", input)
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
            .order_asc("town_id")
            .order_asc("unit_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_town_recruit_pools(
    town_id: Id<Town>,
    limit: u32,
) -> RepoResult<Vec<TownRecruitPool>> {
    let limit = foundation::validate_list_limit(limit)?;
    let mut rows = foundation::storage_result(
        TOWN_RECRUIT_POOLS_LOOKUP.name,
        crate::db()
            .load::<TownRecruitPool>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit)
            .entities(),
    )?;
    rows.sort_by(|left, right| {
        left.unit_id
            .cmp(&right.unit_id)
            .then_with(|| left.id().cmp(&right.id()))
    });
    Ok(rows)
}

pub(crate) fn find_town_recruit_pool(
    town_id: Id<Town>,
    unit_id: Id<UnitDefinition>,
) -> RepoResult<Option<TownRecruitPool>> {
    foundation::storage_result(
        TOWN_RECRUIT_POOL_UNIT_LOOKUP.name,
        crate::db()
            .load::<TownRecruitPool>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("unit_id").eq(unit_id.key()))
            .order_asc("town_id")
            .order_asc("unit_id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_town_recruit_pool(
    session_id: Id<GameSession>,
    town_id: Id<Town>,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    available: u32,
    last_growth_week: u32,
) -> RepoResult<TownRecruitPool> {
    let input: Create<TownRecruitPool> = Create::<TownRecruitPool> {
        session_id: Some(session_id.key()),
        town_id: Some(town_id.key()),
        unit_id: Some(unit_id.key()),
        unit_slug: Some(unit_slug),
        available: Some(available),
        last_growth_week: Some(last_growth_week),
        last_command_id: Some(None),
    };

    foundation::create("towns.create_town_recruit_pool", input)
}

pub(crate) fn update_town_recruit_pool(pool: TownRecruitPool) -> RepoResult<TownRecruitPool> {
    foundation::update("towns.update_recruit_pool", pool)
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
            .order_asc("town_id")
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_town_garrison(
    town_id: Id<Town>,
    limit: u32,
) -> RepoResult<Vec<TownGarrisonStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    let mut rows = foundation::storage_result(
        TOWN_GARRISON_LOOKUP.name,
        crate::db()
            .load::<TownGarrisonStack>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit)
            .entities(),
    )?;
    rows.sort_by(|left, right| {
        left.slot_index
            .cmp(&right.slot_index)
            .then_with(|| left.id().cmp(&right.id()))
    });
    Ok(rows)
}

pub(crate) fn find_town_garrison_stack(
    town_id: Id<Town>,
    slot_index: u8,
) -> RepoResult<Option<TownGarrisonStack>> {
    foundation::storage_result(
        TOWN_GARRISON_SLOT_LOOKUP.name,
        crate::db()
            .load::<TownGarrisonStack>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("slot_index").eq(slot_index))
            .order_asc("town_id")
            .order_asc("slot_index")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_town_garrison_stack(
    session_id: Id<GameSession>,
    town_id: Id<Town>,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    slot_index: u8,
    quantity: u32,
    front_hp: u16,
) -> RepoResult<TownGarrisonStack> {
    let input: Create<TownGarrisonStack> = Create::<TownGarrisonStack> {
        session_id: Some(session_id.key()),
        town_id: Some(town_id.key()),
        unit_id: Some(unit_id.key()),
        unit_slug: Some(unit_slug),
        slot_index: Some(slot_index),
        quantity: Some(quantity),
        front_hp: Some(front_hp),
        last_command_id: Some(None),
    };

    foundation::create("towns.create_town_garrison_stack", input)
}

pub(crate) fn update_town_garrison_stack(
    stack: TownGarrisonStack,
) -> RepoResult<TownGarrisonStack> {
    foundation::update("towns.update_garrison_stack", stack)
}

pub(crate) fn delete_town_garrison_stack(id: Id<TownGarrisonStack>) -> RepoResult<u32> {
    foundation::delete_by_id("towns.delete_garrison_stack", id)
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

#[cfg(test)]
pub(crate) fn town_buildings_plan_text(town_id: Id<Town>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_BUILDINGS_LOOKUP.name,
        crate::db()
            .load::<TownBuilding>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit),
    )
}

#[cfg(test)]
pub(crate) fn town_recruit_pools_plan_text(town_id: Id<Town>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_RECRUIT_POOLS_LOOKUP.name,
        crate::db()
            .load::<TownRecruitPool>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit),
    )
}

#[cfg(test)]
pub(crate) fn town_garrison_plan_text(town_id: Id<Town>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_GARRISON_LOOKUP.name,
        crate::db()
            .load::<TownGarrisonStack>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .order_asc("town_id")
            .limit(limit),
    )
}

#[cfg(test)]
pub(crate) fn town_building_definition_plan_text(
    town_id: Id<Town>,
    building_def_id: Id<BuildingDefinition>,
) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_BUILDING_DEFINITION_LOOKUP.name,
        crate::db()
            .load::<TownBuilding>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("building_def_id").eq(building_def_id.key()))
            .order_asc("town_id")
            .order_asc("building_def_id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn town_recruit_pool_unit_plan_text(
    town_id: Id<Town>,
    unit_id: Id<UnitDefinition>,
) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_RECRUIT_POOL_UNIT_LOOKUP.name,
        crate::db()
            .load::<TownRecruitPool>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("unit_id").eq(unit_id.key()))
            .order_asc("town_id")
            .order_asc("unit_id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn town_garrison_slot_plan_text(
    town_id: Id<Town>,
    slot_index: u8,
) -> RepoResult<String> {
    foundation::explain_text(
        TOWN_GARRISON_SLOT_LOOKUP.name,
        crate::db()
            .load::<TownGarrisonStack>()
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("slot_index").eq(slot_index))
            .order_asc("town_id")
            .order_asc("slot_index")
            .limit(1),
    )
}
