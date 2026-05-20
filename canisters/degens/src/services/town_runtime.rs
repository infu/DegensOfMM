//! Heap-resident town projection cache.
//!
//! This is the first Gate 6 step: keep a real-row-backed town aggregate in
//! heap so command/query paths can stop reloading the same child rows.

use std::{cell::RefCell, collections::BTreeMap};

use domm_degens_schema::schema::{
    GameSession, Town, TownBuilding, TownGarrisonStack, TownRecruitPool, UnitDefinition,
};
use domm_game::ApiError;
use icydb::{
    traits::{EntityKey, EntityValue},
    types::Id,
};

use crate::repos::towns;

#[derive(Clone)]
pub(crate) struct TownProjection {
    pub town: Town,
    pub buildings: Vec<TownBuilding>,
    pub recruit_pools: Vec<TownRecruitPool>,
    pub garrison_stacks: Vec<TownGarrisonStack>,
}

thread_local! {
    static TOWN_PROJECTIONS: RefCell<BTreeMap<(String, String), TownProjection>> =
        RefCell::new(BTreeMap::new());
}

pub(crate) fn projection_for_town(town: &Town) -> Result<TownProjection, ApiError> {
    let key = town_key(town);
    if let Some(projection) =
        TOWN_PROJECTIONS.with(|projections| projections.borrow().get(&key).cloned())
    {
        return Ok(projection);
    }

    let projection = TownProjection {
        town: town.clone(),
        buildings: towns::list_town_buildings(town.id(), 16)?,
        recruit_pools: towns::list_town_recruit_pools(town.id(), 16)?,
        garrison_stacks: towns::list_town_garrison(
            town.id(),
            u32::from(domm_game::MAX_ARMY_SLOTS),
        )?,
    };
    TOWN_PROJECTIONS.with(|projections| {
        projections.borrow_mut().insert(key, projection.clone());
    });
    Ok(projection)
}

pub(crate) fn mirror_town(town: &Town) {
    let key = town_key(town);
    TOWN_PROJECTIONS.with(|projections| {
        if let Some(projection) = projections.borrow_mut().get_mut(&key) {
            projection.town = town.clone();
        }
    });
}

pub(crate) fn mirror_building(row: &TownBuilding) {
    let key = row_key(row.session_id, row.town_id);
    TOWN_PROJECTIONS.with(|projections| {
        let mut projections = projections.borrow_mut();
        let Some(projection) = projections.get_mut(&key) else {
            return;
        };
        if let Some(existing) = projection
            .buildings
            .iter_mut()
            .find(|existing| existing.id() == row.id())
        {
            *existing = row.clone();
        } else {
            projection.buildings.push(row.clone());
        }
    });
}

pub(crate) fn mirror_recruit_pool(row: &TownRecruitPool) {
    let key = row_key(row.session_id, row.town_id);
    TOWN_PROJECTIONS.with(|projections| {
        let mut projections = projections.borrow_mut();
        let Some(projection) = projections.get_mut(&key) else {
            return;
        };
        if let Some(existing) = projection
            .recruit_pools
            .iter_mut()
            .find(|existing| existing.id() == row.id())
        {
            *existing = row.clone();
        } else {
            projection.recruit_pools.push(row.clone());
        }
    });
}

pub(crate) fn mirror_garrison_stack(row: &TownGarrisonStack) {
    let key = row_key(row.session_id, row.town_id);
    TOWN_PROJECTIONS.with(|projections| {
        let mut projections = projections.borrow_mut();
        let Some(projection) = projections.get_mut(&key) else {
            return;
        };
        if let Some(existing) = projection
            .garrison_stacks
            .iter_mut()
            .find(|existing| existing.id() == row.id())
        {
            *existing = row.clone();
        } else {
            projection.garrison_stacks.push(row.clone());
        }
    });
}

pub(crate) fn recruit_pool(
    town: &Town,
    unit_id: Id<UnitDefinition>,
) -> Result<Option<TownRecruitPool>, ApiError> {
    let projection = projection_for_town(town)?;
    Ok(projection
        .recruit_pools
        .into_iter()
        .find(|pool| pool.unit_id == unit_id.key()))
}

pub(crate) fn garrison_stack(
    town: &Town,
    slot_index: u8,
) -> Result<Option<TownGarrisonStack>, ApiError> {
    let projection = projection_for_town(town)?;
    Ok(projection
        .garrison_stacks
        .into_iter()
        .find(|stack| stack.slot_index == slot_index))
}

pub(crate) fn evict_town(session_id: Id<GameSession>, town_id: Id<Town>) {
    let key = projection_key(session_id, town_id);
    TOWN_PROJECTIONS.with(|projections| {
        projections.borrow_mut().remove(&key);
    });
}

fn town_key(town: &Town) -> (String, String) {
    projection_key(Id::<GameSession>::from_key(town.session_id), town.id())
}

fn row_key(
    session_id: <GameSession as EntityKey>::Key,
    town_id: <Town as EntityKey>::Key,
) -> (String, String) {
    projection_key(
        Id::<GameSession>::from_key(session_id),
        Id::<Town>::from_key(town_id),
    )
}

fn projection_key(session_id: Id<GameSession>, town_id: Id<Town>) -> (String, String) {
    (session_id.to_string(), town_id.to_string())
}
