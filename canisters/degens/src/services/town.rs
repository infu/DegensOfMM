use std::collections::BTreeSet;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{GameCommand, GameEvent, GameParticipant, GameSession, Town};
use domm_game::{
    ApiError, ApiTownView, BuildPreview, CommandResponse, RecruitPreview, RecruitTarget,
};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{commands_events_effects, content, economy, sessions, towns};

use super::{
    command_response::{self, GameCommandAction},
    render_projection, session_context, session_turn_runtime,
};

pub(crate) fn get_town_view(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
) -> Result<ApiTownView, ApiError> {
    session_context::reject_anonymous(caller)?;
    let parsed_session_id = session_context::parse_id::<GameSession>(&session_id, "session_id")?;
    let town = resolve_town_by_session_id(parsed_session_id, &town_id)?;
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    if town.owner_participant_id == Some(context.participant.id().key()) {
        return render_projection::town_view(&town);
    }

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

pub(crate) fn preview_build_town_structure(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    building_def_id: String,
) -> Result<BuildPreview, ApiError> {
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
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
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
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
    } else if matches!(target, RecruitTarget::Champion { .. }) {
        Some("unsupported_recruit_target".to_string())
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
    session_turn_runtime::mirror_participant_update(&context.participant);

    let building_row = towns::create_town_building(
        context.session.id(),
        town.id(),
        building.id(),
        building_slug.clone(),
        context.session.current_turn,
    )?;
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

    let mut session = context.session.clone();
    let events = append_town_command_events(
        &mut session,
        command.id(),
        format!("town_build:{}:{building_slug}", town.id()),
        "town_building_built",
        &town.id().to_string(),
        &context.participant.id().to_string(),
        format!(r#"{{"building_slug":"{}"}}"#, building_slug),
    )?;
    context.session = session;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        command_response::result_json("submit_build_town_structure", context.session.current_turn),
        events,
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
    if matches!(target, RecruitTarget::Champion { .. }) {
        return Err(ApiError::new(
            "unsupported_recruit_target",
            "town recruitment to champions is reserved for v2",
            false,
        ));
    }
    let payload_json = format!(
        r#"{{"town_id":"{}","unit_slug":"{}","quantity":{},"target":{}}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&unit_slug),
        quantity,
        recruit_target_json(&target)
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
    session_turn_runtime::mirror_participant_update(&context.participant);

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
    let mut session = context.session.clone();
    let events = append_town_command_events(
        &mut session,
        command.id(),
        format!("town_recruit:{}:{unit_slug}:{}", town.id(), command.id()),
        "units_recruited",
        &town.id().to_string(),
        &context.participant.id().to_string(),
        format!(r#"{{"unit_slug":"{}","quantity":{}}}"#, unit_slug, quantity),
    )?;
    context.session = session;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        command_response::result_json("submit_recruit_units", context.session.current_turn),
        events,
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
    resolve_town_by_session_id(session.id(), town_id)
}

fn resolve_town_by_session_id(
    session_id: Id<GameSession>,
    town_id: &str,
) -> Result<Town, ApiError> {
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
    towns::find_town_by_session_xy(session_id, start.town_x, start.town_y)?
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
                session_id,
                town_id,
                unit_id,
                unit_slug,
                slot_index,
                quantity,
                front_hp,
                Some(command_id),
            )?;
            stack.last_command_id = Some(command_id.key());
            stack
        }
    };
    Ok(stack)
}

fn recruit_target_json(target: &RecruitTarget) -> String {
    match target {
        RecruitTarget::TownGarrison { slot_index } => format!(
            r#"{{"kind":"town_garrison","slot_index":{}}}"#,
            slot_index
                .map(|slot| slot.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        RecruitTarget::Champion {
            champion_id,
            slot_index,
        } => format!(
            r#"{{"kind":"champion","champion_id":"{}","slot_index":{}}}"#,
            command_response::escape_json(champion_id),
            slot_index
                .map(|slot| slot.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
    }
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
    if let Some(entry) = economy::find_resource_ledger_entry(command_id, &ledger_key)? {
        reconcile_resource_balance(participant, resource_key, entry.balance_after)?;
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

fn reconcile_resource_balance(
    participant: &mut GameParticipant,
    resource_key: &str,
    balance_after: u64,
) -> Result<(), ApiError> {
    match resource_key {
        "gold" => {
            participant.gold = balance_after;
            Ok(())
        }
        "wood" => {
            participant.wood = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "stone" => {
            participant.stone = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "iron" => {
            participant.iron = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "crystal" => {
            participant.crystal = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "ember" => {
            participant.ember = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "aether" => {
            participant.aether = u32::try_from(balance_after).map_err(|_| {
                ApiError::new("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        _ => Err(ApiError::new(
            "unknown_resource",
            "unknown resource key",
            false,
        )),
    }
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

fn append_town_command_events(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    event_key: String,
    event_type: &str,
    town_id: &str,
    participant_id: &str,
    detailed_payload: String,
) -> Result<Vec<domm_game::ApiEventView>, ApiError> {
    let audience_key = format!("participant:{participant_id}");
    let mut next_event_seq = session.next_event_seq;
    let private_event = append_town_event(
        session,
        command_id,
        &mut next_event_seq,
        format!("{event_key}:{audience_key}"),
        audience_key,
        event_type,
        town_id,
        detailed_payload,
    )?;
    let public_event = match append_town_event(
        session,
        command_id,
        &mut next_event_seq,
        format!("{event_key}:public"),
        "public".to_string(),
        event_type,
        town_id,
        format!(
            r#"{{"town_id":"{}","event_type":"{}","redacted":true}}"#,
            command_response::escape_json(town_id),
            command_response::escape_json(event_type)
        ),
    ) {
        Ok(event) => event,
        Err(error) => {
            if session.next_event_seq != next_event_seq {
                session.next_event_seq = next_event_seq;
                *session = sessions::update_session(session.clone())?;
            }
            return Err(error);
        }
    };
    if session.next_event_seq != next_event_seq {
        session.next_event_seq = next_event_seq;
        *session = sessions::update_session(session.clone())?;
    }
    Ok(vec![private_event, public_event])
}

fn append_town_event(
    session: &GameSession,
    command_id: Id<GameCommand>,
    next_event_seq: &mut u64,
    event_key: String,
    audience_key: String,
    event_type: &str,
    town_id: &str,
    payload_json: String,
) -> Result<domm_game::ApiEventView, ApiError> {
    match commands_events_effects::create_game_event(
        session.id(),
        Some(command_id),
        None,
        session.current_turn,
        *next_event_seq,
        event_key.clone(),
        audience_key,
        event_type.to_string(),
        Some("town".to_string()),
        Some(town_id.to_string()),
        payload_json,
    ) {
        Ok(event) => {
            *next_event_seq = next_event_seq.saturating_add(1);
            Ok(api_event_view(event))
        }
        Err(error) => {
            if let Some(event) =
                commands_events_effects::find_event_by_key(session.id(), &event_key)?
            {
                if *next_event_seq <= event.event_seq {
                    *next_event_seq = event.event_seq.saturating_add(1);
                }
                Ok(api_event_view(event))
            } else {
                Err(error)
            }
        }
    }
}

fn api_event_view(event: GameEvent) -> domm_game::ApiEventView {
    domm_game::ApiEventView {
        session_id: Id::<GameSession>::from_key(event.session_id).to_string(),
        event_seq: event.event_seq,
        event_key: event.event_key,
        audience_key: event.audience_key,
        turn_number: event.turn_number,
        event_type: event.event_type,
        subject_kind: event.subject_kind,
        subject_id_text: event.subject_id_text,
        payload: Some(event.payload_json),
        redacted: false,
    }
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
