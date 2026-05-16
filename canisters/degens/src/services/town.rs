use std::collections::BTreeSet;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{GameParticipant, GameSession, Town};
use domm_game::{
    ApiError, ApiTownView, BuildPreview, CommandResponse, RecruitPreview, RecruitTarget,
};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{content, economy, sessions, towns};

use super::{
    command_response::{self, GameCommandAction},
    render_projection, session_context,
};

pub(crate) fn get_town_view(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
) -> Result<ApiTownView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    if town.owner_participant_id == Some(context.participant.id().key()) {
        render_projection::town_view(&town)
    } else {
        if !render_projection::is_visible_at(
            &context.session,
            context.participant.id(),
            town.x,
            town.y,
        )? {
            return Err(session_context::public_error(
                "not_visible",
                "town is not visible",
                false,
            ));
        }
        render_projection::town_view(&town)
    }
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
    if building_slug == "freehold-training-yard" {
        return first_playable_build_preview(&context.participant, &town, building_slug);
    }
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

fn first_playable_build_preview(
    participant: &GameParticipant,
    town: &Town,
    building_slug: String,
) -> Result<BuildPreview, ApiError> {
    let manifest = domm_game::first_playable_content_manifest();
    let building = manifest.building(&building_slug).ok_or_else(|| {
        ApiError::new(
            "building_not_found",
            "building definition was not found",
            false,
        )
    })?;
    let cost = resource_balances(
        building.cost.gold,
        building.cost.wood,
        building.cost.stone,
        building.cost.iron,
        building.cost.crystal,
        building.cost.ember,
        building.cost.aether,
    );
    let disabled_reason = if town.owner_participant_id != Some(participant.id().key()) {
        Some("not_owner".to_string())
    } else if town.last_built_turn > 0 {
        Some("already_built".to_string())
    } else {
        (!can_afford(participant, &cost)).then(|| "insufficient_resources".to_string())
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

pub(crate) fn submit_build_town_structure(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    building_def_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let building_slug = slug_from_public_id(&building_def_id, "building:");
    let payload_json = format!(
        r#"{{"town_id":"{}","building_slug":"{}"}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&building_slug)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "submit_build_town_structure",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

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
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("not_owner", "caller does not own this town", false),
        );
    }
    if built_building_ids.contains(&building.id) {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("already_built", "town already has this building", false),
        );
    }
    if let Some(reason) = missing_prerequisite {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new(reason, "building prerequisite is missing", false),
        );
    }
    spend_resources(
        context.session.id(),
        &mut context.participant,
        command.id(),
        &format!("build:{building_slug}"),
        context.session.current_turn,
        &cost,
        "build",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    context.participant = sessions::update_participant(context.participant.clone())?;

    let building_row = match towns::find_town_building(town.id(), building.id())? {
        Some(row) => row,
        None => towns::create_town_building(
            context.session.id(),
            town.id(),
            building.id(),
            building_slug.clone(),
            context.session.current_turn,
        )?,
    };
    if let Some(unit_slug) = &building.unlocks_unit_slug {
        let unit = content::find_unit_by_ruleset_slug(ruleset_id, unit_slug)?.ok_or_else(|| {
            ApiError::new(
                "unit_not_found",
                "building unlock unit definition was not found",
                true,
            )
        })?;
        if towns::find_town_recruit_pool(town.id(), unit.id())?.is_none() {
            towns::create_town_recruit_pool(
                context.session.id(),
                town.id(),
                unit.id(),
                unit_slug.clone(),
                u32::from(unit.weekly_growth),
                1,
            )?;
        }
    }
    let mut town = town;
    town.last_built_turn = context.session.current_turn;
    town.last_command_id = Some(command.id);
    towns::update_town(town.clone())?;

    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("build:{building_slug}:{}", town.id()),
        "town_build".to_string(),
        "town".to_string(),
        town.id().to_string(),
        format!(r#"{{"building_slug":"{}"}}"#, building_slug),
    )?;
    let mut session = context.session.clone();
    let event = command_response::append_public_event(
        &mut session,
        command.id(),
        format!("town_build:{}:{building_slug}", town.id()),
        "town_building_built".to_string(),
        Some("town".to_string()),
        Some(town.id().to_string()),
        format!(r#"{{"building_slug":"{}"}}"#, building_slug),
    )?;
    context.session = session;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        command_response::result_json("submit_build_town_structure", context.session.current_turn),
        vec![event],
        vec![
            command_response::changed("town", &town.id().to_string(), "update"),
            command_response::changed("town_building", &building_row.id().to_string(), "create"),
            command_response::changed(
                "participant",
                &context.participant.id().to_string(),
                "resources",
            ),
        ],
    )
}

pub(crate) fn submit_recruit_units(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    unit_id: String,
    quantity: u32,
    target: RecruitTarget,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let unit_slug = slug_from_public_id(&unit_id, "unit:");
    let payload_json = format!(
        r#"{{"town_id":"{}","unit_slug":"{}","quantity":{}}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&unit_slug),
        quantity
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "submit_recruit_units",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let unit = content::find_unit_by_ruleset_slug(ruleset_id()?, &unit_slug)?
        .ok_or_else(|| ApiError::new("unit_not_found", "unit definition was not found", false))?;
    let Some(mut pool) = towns::find_town_recruit_pool(town.id(), unit.id())? else {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("recruit_pool_empty", "no recruit pool is available", false),
        );
    };
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("not_owner", "caller does not own this town", false),
        );
    }
    if quantity == 0 || pool.available < quantity {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new(
                "recruit_pool_empty",
                "not enough units are available",
                false,
            ),
        );
    }
    let total_cost = resource_balances(
        unit.gold_cost.saturating_mul(quantity),
        unit.wood_cost.saturating_mul(quantity),
        unit.stone_cost.saturating_mul(quantity),
        unit.iron_cost.saturating_mul(quantity),
        unit.crystal_cost.saturating_mul(quantity),
        unit.ember_cost.saturating_mul(quantity),
        unit.aether_cost.saturating_mul(quantity),
    );
    spend_resources(
        context.session.id(),
        &mut context.participant,
        command.id(),
        &format!("recruit:{unit_slug}"),
        context.session.current_turn,
        &total_cost,
        "recruit",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    context.participant = sessions::update_participant(context.participant.clone())?;

    pool.available = pool.available.saturating_sub(quantity);
    pool.last_command_id = Some(command.id);
    pool = towns::update_town_recruit_pool(pool)?;

    let garrison = recruit_to_garrison(
        context.session.id(),
        town.id(),
        unit.id(),
        unit_slug.clone(),
        unit.max_hp,
        quantity,
        target,
        command.id(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("recruit:{unit_slug}:{}", town.id()),
        "town_recruit".to_string(),
        "town".to_string(),
        town.id().to_string(),
        format!(r#"{{"unit_slug":"{}","quantity":{}}}"#, unit_slug, quantity),
    )?;
    let mut session = context.session.clone();
    let event = command_response::append_public_event(
        &mut session,
        command.id(),
        format!("town_recruit:{}:{unit_slug}:{}", town.id(), command.id()),
        "units_recruited".to_string(),
        Some("town".to_string()),
        Some(town.id().to_string()),
        format!(r#"{{"unit_slug":"{}","quantity":{}}}"#, unit_slug, quantity),
    )?;
    context.session = session;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        command_response::result_json("submit_recruit_units", context.session.current_turn),
        vec![event],
        vec![
            command_response::changed("town", &town.id().to_string(), "update"),
            command_response::changed("town_recruit_pool", &pool.id().to_string(), "update"),
            command_response::changed("town_garrison_stack", &garrison.id().to_string(), "upsert"),
            command_response::changed(
                "participant",
                &context.participant.id().to_string(),
                "resources",
            ),
        ],
    )
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

fn recruit_to_garrison(
    session_id: Id<domm_degens_schema::schema::GameSession>,
    town_id: Id<domm_degens_schema::schema::Town>,
    unit_id: Id<domm_degens_schema::schema::UnitDefinition>,
    unit_slug: String,
    front_hp: u16,
    quantity: u32,
    target: RecruitTarget,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
) -> Result<domm_degens_schema::schema::TownGarrisonStack, ApiError> {
    let requested_slot = match target {
        RecruitTarget::TownGarrison { slot_index } => slot_index,
        RecruitTarget::Champion { .. } => {
            return Err(ApiError::new(
                "unsupported_recruit_target",
                "canister recruit currently supports town garrison targets",
                false,
            ));
        }
    };
    let slot_index = requested_slot.unwrap_or(0);
    let stack = match towns::find_town_garrison_stack(town_id, slot_index)? {
        Some(mut stack) => {
            if stack.unit_id != unit_id.key() {
                return Err(ApiError::new(
                    "incompatible_garrison_stack",
                    "target garrison slot contains another unit",
                    false,
                ));
            }
            stack.quantity = stack.quantity.saturating_add(quantity);
            stack.last_command_id = Some(command_id.key());
            towns::update_town_garrison_stack(stack)?
        }
        None => {
            let mut stack = towns::create_town_garrison_stack(
                session_id, town_id, unit_id, unit_slug, slot_index, quantity, front_hp,
            )?;
            stack.last_command_id = Some(command_id.key());
            towns::update_town_garrison_stack(stack)?
        }
    };
    Ok(stack)
}

fn spend_resources(
    session_id: Id<domm_degens_schema::schema::GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
    ledger_prefix: &str,
    turn_number: u32,
    cost: &domm_game::ResourceBalances,
    reason: &str,
) -> Result<(), ApiError> {
    for (resource_key, amount) in [
        ("gold", i64::try_from(cost.gold).unwrap_or(i64::MAX)),
        ("wood", i64::from(cost.wood)),
        ("stone", i64::from(cost.stone)),
        ("iron", i64::from(cost.iron)),
        ("crystal", i64::from(cost.crystal)),
        ("ember", i64::from(cost.ember)),
        ("aether", i64::from(cost.aether)),
    ] {
        if amount == 0 {
            continue;
        }
        apply_resource_delta(
            session_id,
            participant,
            command_id,
            format!("{ledger_prefix}:{resource_key}"),
            turn_number,
            resource_key,
            -amount,
            reason,
        )?;
    }
    participant.last_resource_command_id = Some(command_id.key());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_resource_delta(
    session_id: Id<domm_degens_schema::schema::GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
    ledger_key: String,
    turn_number: u32,
    resource_key: &str,
    delta: i64,
    reason: &str,
) -> Result<(), ApiError> {
    if economy::find_resource_ledger_entry(command_id, &ledger_key)?.is_some() {
        return Ok(());
    }
    let balance_after = match resource_key {
        "gold" => {
            participant.gold = apply_u64_delta(participant.gold, delta)?;
            participant.gold
        }
        "wood" => {
            participant.wood = apply_u32_delta(participant.wood, delta)?;
            u64::from(participant.wood)
        }
        "stone" => {
            participant.stone = apply_u32_delta(participant.stone, delta)?;
            u64::from(participant.stone)
        }
        "iron" => {
            participant.iron = apply_u32_delta(participant.iron, delta)?;
            u64::from(participant.iron)
        }
        "crystal" => {
            participant.crystal = apply_u32_delta(participant.crystal, delta)?;
            u64::from(participant.crystal)
        }
        "ember" => {
            participant.ember = apply_u32_delta(participant.ember, delta)?;
            u64::from(participant.ember)
        }
        "aether" => {
            participant.aether = apply_u32_delta(participant.aether, delta)?;
            u64::from(participant.aether)
        }
        _ => {
            return Err(ApiError::new(
                "unknown_resource",
                "unknown resource key",
                false,
            ));
        }
    };
    economy::create_resource_ledger_entry(
        session_id,
        participant.id(),
        command_id,
        ledger_key,
        turn_number,
        resource_key.to_string(),
        delta,
        balance_after,
        reason.to_string(),
        "applied".to_string(),
    )?;
    Ok(())
}

fn apply_u64_delta(value: u64, delta: i64) -> Result<u64, ApiError> {
    if delta.is_negative() {
        value
            .checked_sub(delta.unsigned_abs())
            .ok_or_else(|| ApiError::new("insufficient_resources", "not enough resources", false))
    } else {
        Ok(value.saturating_add(delta as u64))
    }
}

fn apply_u32_delta(value: u32, delta: i64) -> Result<u32, ApiError> {
    let value = apply_u64_delta(u64::from(value), delta)?;
    u32::try_from(value)
        .map_err(|_| ApiError::new("resource_cap_exceeded", "resource cap exceeded", false))
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
