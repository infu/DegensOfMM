//! Heap-resident town projection cache.
//!
//! This is the first Gate 6 step: keep a real-row-backed town aggregate in
//! heap so command/query paths can stop reloading the same child rows.

use std::{cell::RefCell, collections::BTreeMap};

use domm_degens_schema::schema::{
    BuildingDefinition, GameCommand, GameSession, TavernOffer, Town, TownBuilding,
    TownGarrisonStack, TownRecruitPool, UnitDefinition,
};
use domm_game::ApiError;
use icydb::{
    traits::{EntityKey, EntityValue},
    types::{Id, Timestamp, Ulid},
};

use crate::repos::{economy_expansion as economy_expansion_repo, towns};

#[derive(Clone)]
pub(crate) struct TownProjection {
    pub town: Town,
    pub buildings: Vec<TownBuilding>,
    pub recruit_pools: Vec<TownRecruitPool>,
    pub garrison_stacks: Vec<TownGarrisonStack>,
    pub tavern_offers: Vec<TavernOffer>,
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
        tavern_offers: Vec::new(),
    };
    TOWN_PROJECTIONS.with(|projections| {
        projections.borrow_mut().insert(key, projection.clone());
    });
    Ok(projection)
}

pub(crate) fn seed_town(town: &Town) {
    let key = town_key(town);
    TOWN_PROJECTIONS.with(|projections| {
        projections.borrow_mut().insert(
            key,
            TownProjection {
                town: town.clone(),
                buildings: Vec::new(),
                recruit_pools: Vec::new(),
                garrison_stacks: Vec::new(),
                tavern_offers: Vec::new(),
            },
        );
    });
}

pub(crate) fn cached_town_by_public_id(session_id: Id<GameSession>, town_id: &str) -> Option<Town> {
    if let Ok(id) = Ulid::from_str(town_id)
        && let Some(town) = cached_town_by_id(session_id, Id::<Town>::from_key(id))
    {
        return Some(town);
    }
    let start = domm_game::first_playable_scenario()
        .starts
        .into_iter()
        .find(|start| start.town_key == town_id)?;
    cached_town_by_xy(session_id, start.town_x, start.town_y)
}

fn cached_town_by_id(session_id: Id<GameSession>, town_id: Id<Town>) -> Option<Town> {
    TOWN_PROJECTIONS.with(|projections| {
        projections
            .borrow()
            .values()
            .find(|projection| {
                projection.town.session_id == session_id.key() && projection.town.id() == town_id
            })
            .map(|projection| projection.town.clone())
    })
}

fn cached_town_by_xy(session_id: Id<GameSession>, town_x: u16, town_y: u16) -> Option<Town> {
    TOWN_PROJECTIONS.with(|projections| {
        projections
            .borrow()
            .values()
            .find(|projection| {
                projection.town.session_id == session_id.key()
                    && projection.town.x == town_x
                    && projection.town.y == town_y
            })
            .map(|projection| projection.town.clone())
    })
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

pub(crate) fn mirror_tavern_offer(row: &TavernOffer) {
    let key = row_key(row.session_id, row.town_id);
    TOWN_PROJECTIONS.with(|projections| {
        let mut projections = projections.borrow_mut();
        let Some(projection) = projections.get_mut(&key) else {
            return;
        };
        if let Some(existing) = projection
            .tavern_offers
            .iter_mut()
            .find(|existing| existing.id() == row.id())
        {
            *existing = row.clone();
        } else {
            projection.tavern_offers.push(row.clone());
        }
    });
}

pub(crate) fn tavern_offers_for_week(
    town: &Town,
    week_number: u32,
) -> Result<Vec<TavernOffer>, ApiError> {
    projection_for_town(town)?;
    if let Some(offers) = cached_tavern_offers_for_week(town, week_number) {
        return Ok(offers);
    }
    let offers =
        economy_expansion_repo::page_tavern_offers(town_session_id(town), town.id(), week_number)?
            .items;
    for offer in &offers {
        mirror_tavern_offer(offer);
    }
    Ok(offers)
}

pub(crate) fn tavern_offer_by_key(
    town: &Town,
    offer_key: &str,
) -> Result<Option<TavernOffer>, ApiError> {
    projection_for_town(town)?;
    if let Some(offer) = TOWN_PROJECTIONS.with(|projections| {
        projections
            .borrow()
            .get(&town_key(town))
            .and_then(|projection| {
                projection
                    .tavern_offers
                    .iter()
                    .find(|offer| offer.offer_key == offer_key)
                    .cloned()
            })
    }) {
        return Ok(Some(offer));
    }
    let Some(offer) = economy_expansion_repo::find_tavern_offer_by_key(offer_key)? else {
        return Ok(None);
    };
    if offer.session_id == town.session_id && offer.town_id == town.id().key() {
        mirror_tavern_offer(&offer);
    }
    Ok(Some(offer))
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

pub(crate) fn building_slugs(town: &Town) -> Result<Vec<String>, ApiError> {
    let projection = projection_for_town(town)?;
    Ok(projection
        .buildings
        .into_iter()
        .map(|building| building.building_slug)
        .collect())
}

pub(crate) fn create_building(
    town: &Town,
    building_def_id: Id<BuildingDefinition>,
    building_slug: String,
    built_turn: u32,
) -> Result<TownBuilding, ApiError> {
    projection_for_town(town)?;
    let now = Timestamp::now();
    let row = TownBuilding {
        id: Ulid::generate(),
        session_id: town.session_id,
        town_id: town.id().key(),
        building_def_id: building_def_id.key(),
        building_slug,
        built_turn,
        created_at: now,
        updated_at: now,
    };
    mirror_building(&row);
    Ok(row)
}

pub(crate) fn create_recruit_pool(
    town: &Town,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    available: u32,
    last_growth_week: u32,
) -> Result<TownRecruitPool, ApiError> {
    projection_for_town(town)?;
    let now = Timestamp::now();
    let row = TownRecruitPool {
        id: Ulid::generate(),
        session_id: town.session_id,
        town_id: town.id().key(),
        unit_id: unit_id.key(),
        unit_slug,
        available,
        last_growth_week,
        last_command_id: None,
        created_at: now,
        updated_at: now,
    };
    mirror_recruit_pool(&row);
    Ok(row)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_garrison_stack(
    town: &Town,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    slot_index: u8,
    quantity: u32,
    front_hp: u16,
    command_id: Id<GameCommand>,
) -> Result<TownGarrisonStack, ApiError> {
    projection_for_town(town)?;
    let now = Timestamp::now();
    let row = TownGarrisonStack {
        id: Ulid::generate(),
        session_id: town.session_id,
        town_id: town.id().key(),
        unit_id: unit_id.key(),
        unit_slug,
        slot_index,
        quantity,
        front_hp,
        last_command_id: Some(command_id.key()),
        created_at: now,
        updated_at: now,
    };
    mirror_garrison_stack(&row);
    Ok(row)
}

pub(crate) fn evict_town(session_id: Id<GameSession>, town_id: Id<Town>) {
    let key = projection_key(session_id, town_id);
    TOWN_PROJECTIONS.with(|projections| {
        projections.borrow_mut().remove(&key);
    });
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn flush_all_projections_to_durable() -> Result<usize, ApiError> {
    let projections = TOWN_PROJECTIONS.with(|projections| {
        projections
            .borrow()
            .values()
            .cloned()
            .collect::<Vec<TownProjection>>()
    });
    let mut flushed = 0_usize;
    for projection in projections {
        if towns::load_town(projection.town.id())?.is_some() {
            towns::update_town(projection.town.clone())?;
            flushed = flushed.saturating_add(1);
        }
        for building in projection.buildings {
            if towns::load_town_building(building.id())?.is_some() {
                towns::update_town_building(building)?;
            } else {
                towns::insert_town_building(building)?;
            }
            flushed = flushed.saturating_add(1);
        }
        for pool in projection.recruit_pools {
            if towns::load_town_recruit_pool(pool.id())?.is_some() {
                towns::update_town_recruit_pool(pool)?;
            } else {
                towns::insert_town_recruit_pool(pool)?;
            }
            flushed = flushed.saturating_add(1);
        }
        for stack in projection.garrison_stacks {
            if towns::load_town_garrison_stack(stack.id())?.is_some() {
                towns::update_town_garrison_stack(stack)?;
            } else {
                towns::insert_town_garrison_stack(stack)?;
            }
            flushed = flushed.saturating_add(1);
        }
    }
    Ok(flushed)
}

fn cached_tavern_offers_for_week(town: &Town, week_number: u32) -> Option<Vec<TavernOffer>> {
    TOWN_PROJECTIONS.with(|projections| {
        let mut offers = projections
            .borrow()
            .get(&town_key(town))?
            .tavern_offers
            .iter()
            .filter(|offer| offer.week_number == week_number)
            .cloned()
            .collect::<Vec<_>>();
        if offers.len() < domm_game::TAVERN_OFFERS_PER_WEEK {
            return None;
        }
        offers.sort_by_key(|offer| (offer.offer_slot, offer.id));
        Some(offers)
    })
}

fn town_session_id(town: &Town) -> Id<GameSession> {
    Id::<GameSession>::from_key(town.session_id)
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
