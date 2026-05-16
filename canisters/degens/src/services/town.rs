use std::collections::BTreeSet;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{GameSession, Town};
use domm_game::{ApiError, ApiTownView, BuildPreview, RecruitPreview, RecruitTarget};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{content, towns};

use super::{render_projection, session_context};

pub(crate) fn get_town_view(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
) -> Result<ApiTownView, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    render_projection::town_view_by_id(&context, &town_id)
}

pub(crate) fn preview_build_town_structure(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    building_def_id: String,
) -> Result<BuildPreview, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let building_slug = slug_from_public_id(&building_def_id, "building:");
    let ruleset_id = ruleset_id()?;
    let building =
        content::find_building_by_ruleset_slug(ruleset_id, &building_slug)?.ok_or_else(|| {
            ApiError::new(
                "building_not_found",
                "building definition was not found",
                false,
            )
        })?;
    let cost = resource_balances(
        building.gold_cost,
        building.wood_cost,
        building.stone_cost,
        building.iron_cost,
        building.crystal_cost,
        building.ember_cost,
        building.aether_cost,
    );
    let built_building_ids = built_building_ids(town.id())?;
    let missing_prerequisite =
        missing_required_building_slug(ruleset_id, &built_building_ids, &building)?;
    let disabled_reason = if town.owner_participant_id != Some(context.participant.id().key()) {
        Some("not_owner".to_string())
    } else if built_building_ids.contains(&building.id) {
        Some("already_built".to_string())
    } else {
        missing_prerequisite.or_else(|| {
            (!can_afford(&context.participant, &cost)).then(|| "insufficient_resources".to_string())
        })
    };

    Ok(BuildPreview {
        allowed: disabled_reason.is_none(),
        disabled_reason,
        town_id: town.id().to_string(),
        building_slug,
        cost,
    })
}

pub(crate) fn preview_recruit_units(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    unit_id: String,
    quantity: u32,
    target: RecruitTarget,
) -> Result<RecruitPreview, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let unit_slug = slug_from_public_id(&unit_id, "unit:");
    let unit = content::find_unit_by_ruleset_slug(ruleset_id()?, &unit_slug)?
        .ok_or_else(|| ApiError::new("unit_not_found", "unit definition was not found", false))?;
    let total_cost = resource_balances(
        unit.gold_cost.saturating_mul(quantity),
        unit.wood_cost.saturating_mul(quantity),
        unit.stone_cost.saturating_mul(quantity),
        unit.iron_cost.saturating_mul(quantity),
        unit.crystal_cost.saturating_mul(quantity),
        unit.ember_cost.saturating_mul(quantity),
        unit.aether_cost.saturating_mul(quantity),
    );
    let available = towns::page_town_recruit_pools(town.id(), domm_game::MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .find(|pool| pool.unit_id == unit.id)
        .map_or(0, |pool| pool.available);
    let target_slot_index = match target {
        RecruitTarget::TownGarrison { slot_index } => slot_index,
        RecruitTarget::Champion { slot_index, .. } => slot_index,
    };
    let disabled_reason = if town.owner_participant_id != Some(context.participant.id().key()) {
        Some("not_owner".to_string())
    } else if quantity == 0 {
        Some("invalid_quantity".to_string())
    } else if available < quantity {
        Some("recruit_pool_empty".to_string())
    } else {
        (!can_afford(&context.participant, &total_cost))
            .then(|| "insufficient_resources".to_string())
    };

    Ok(RecruitPreview {
        allowed: disabled_reason.is_none(),
        disabled_reason,
        town_id: town.id().to_string(),
        unit_slug,
        quantity,
        target_slot_index,
        total_cost,
        available,
    })
}

fn resolve_town(session: &GameSession, town_id: &str) -> Result<Town, ApiError> {
    if let Ok(id) = session_context::parse_id::<Town>(town_id, "town_id") {
        return towns::load_town(id)?
            .ok_or_else(|| session_context::public_error("not_found", "town not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.town_key == town_id)
        .ok_or_else(|| session_context::public_error("not_found", "town not found", false))?;
    towns::find_town_by_session_xy(session.id(), start.town_x, start.town_y)?
        .ok_or_else(|| session_context::public_error("not_found", "town not found", false))
}

fn built_building_ids(
    town_id: Id<domm_degens_schema::schema::Town>,
) -> Result<BTreeSet<icydb::types::Ulid>, ApiError> {
    Ok(
        towns::page_town_buildings(town_id, domm_game::MAX_LIST_LIMIT, None)?
            .items
            .into_iter()
            .map(|building| building.building_def_id)
            .collect(),
    )
}

fn missing_required_building_slug(
    ruleset_id: Id<domm_degens_schema::schema::RulesetDefinition>,
    built_building_ids: &BTreeSet<icydb::types::Ulid>,
    building: &domm_degens_schema::schema::BuildingDefinition,
) -> Result<Option<String>, ApiError> {
    for required in &building.requires_building_slugs {
        let required_building = content::find_building_by_ruleset_slug(ruleset_id, required)?
            .ok_or_else(|| {
                ApiError::new(
                    "building_not_found",
                    format!("required building definition was not found: {required}"),
                    true,
                )
            })?;
        if !built_building_ids.contains(&required_building.id) {
            return Ok(Some(format!("missing_prerequisite:{required}")));
        }
    }
    Ok(None)
}

fn ruleset_id() -> Result<Id<domm_degens_schema::schema::RulesetDefinition>, ApiError> {
    content::find_ruleset_by_slug_version(
        domm_game::FIRST_PLAYABLE_RULESET_SLUG,
        domm_game::FIRST_PLAYABLE_RULESET_VERSION,
    )?
    .map(|ruleset| ruleset.id())
    .ok_or_else(|| {
        ApiError::new(
            "content_manifest_not_seeded",
            "first playable content rows have not been seeded",
            true,
        )
    })
}

fn resource_balances(
    gold: u32,
    wood: u32,
    stone: u32,
    iron: u32,
    crystal: u32,
    ember: u32,
    aether: u32,
) -> domm_game::ResourceBalances {
    domm_game::ResourceBalances {
        gold: u64::from(gold),
        wood,
        stone,
        iron,
        crystal,
        ember,
        aether,
    }
}

fn can_afford(
    participant: &domm_degens_schema::schema::GameParticipant,
    cost: &domm_game::ResourceBalances,
) -> bool {
    participant.gold >= cost.gold
        && participant.wood >= cost.wood
        && participant.stone >= cost.stone
        && participant.iron >= cost.iron
        && participant.crystal >= cost.crystal
        && participant.ember >= cost.ember
        && participant.aether >= cost.aether
}

fn slug_from_public_id(value: &str, prefix: &str) -> String {
    value.strip_prefix(prefix).unwrap_or(value).to_string()
}

pub(crate) use super::repository_not_implemented as unavailable;
