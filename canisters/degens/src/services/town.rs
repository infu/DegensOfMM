use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    Champion, GameCommand, GameParticipant, GameSession, Town, UnitDefinition,
};
use domm_game::{
    ApiError, ApiTownView, BuildPreview, CommandPhase, CommandResponse, CommandResult,
    CommandStatus, RecruitPreview, RecruitTarget, StrategicCommandReceipt,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Ulid},
};

use crate::repos::{champions_artifacts, content};

use super::{
    command_response, economy_expansion, render_projection, session_context, session_turn_runtime,
    town_runtime,
};

struct RuntimeTownCommand {
    command_id: Id<GameCommand>,
    command_id_text: String,
    client_nonce: u64,
    payload_hash: String,
    #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
    payload_json: String,
    command_type: String,
}

enum RuntimeTownCommandAction {
    Apply(RuntimeTownCommand),
    Return(CommandResponse),
}

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
    let ruleset_id =
        Id::<domm_degens_schema::schema::RulesetDefinition>::from_key(context.session.ruleset_id);
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
    let built_building_slugs = town_runtime::building_slugs(&town)?;
    let missing_prerequisite = missing_required_building_slug(&built_building_slugs, &building);
    let disabled_reason = if town.owner_participant_id != Some(context.participant.id().key()) {
        Some("not_owner".to_string())
    } else if built_building_slugs
        .iter()
        .any(|slug| slug == &building_slug)
    {
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
    let ruleset_id =
        Id::<domm_degens_schema::schema::RulesetDefinition>::from_key(context.session.ruleset_id);
    let unit = content::find_unit_by_ruleset_slug(ruleset_id, &unit_slug)?
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
    let available = town_runtime::recruit_pool(&town, unit.id())?.map_or(0, |pool| pool.available);
    let mut target_slot_index = match &target {
        RecruitTarget::TownGarrison { slot_index } => *slot_index,
        RecruitTarget::Champion { slot_index, .. } => *slot_index,
    };
    let mut disabled_reason = if town.owner_participant_id != Some(context.participant.id().key()) {
        Some("not_owner".to_string())
    } else if quantity == 0 {
        Some("invalid_quantity".to_string())
    } else if available < quantity {
        Some("recruit_pool_empty".to_string())
    } else if !can_afford(&context.participant, &total_cost) {
        Some("insufficient_resources".to_string())
    } else {
        None
    };
    if disabled_reason.is_none() {
        let target_check = recruit_target_check(
            &context.session,
            &context.participant,
            &town,
            unit.id(),
            quantity,
            &target,
        )?;
        target_slot_index = target_check.target_slot_index;
        disabled_reason = target_check.disabled_reason;
    }

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
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let building_slug = slug_from_public_id(&building_def_id, "building:");
    let payload_json = format!(
        r#"{{"town_id":"{}","building_slug":"{}"}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&building_slug)
    );
    let command = match begin_runtime_town_command(
        caller,
        &context,
        "submit_build_town_structure",
        &client_nonce,
        payload_json,
    )? {
        RuntimeTownCommandAction::Apply(command) => command,
        RuntimeTownCommandAction::Return(response) => return Ok(response),
    };

    let ruleset_id =
        Id::<domm_degens_schema::schema::RulesetDefinition>::from_key(context.session.ruleset_id);
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
    let built_building_slugs = town_runtime::building_slugs(&town)?;
    let missing_prerequisite = missing_required_building_slug(&built_building_slugs, &building);
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("not_owner", "caller does not own this town", false),
        ));
    }
    if built_building_slugs
        .iter()
        .any(|slug| slug == &building_slug)
    {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("already_built", "town already has this building", false),
        ));
    }
    if let Some(reason) = missing_prerequisite {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new(reason, "building prerequisite is missing", false),
        ));
    }
    spend_resources_runtime(
        context.session.id(),
        &mut context.participant,
        command.command_id,
        &format!("build:{building_slug}"),
        context.session.current_turn,
        &cost,
        "build",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    session_turn_runtime::mirror_participant_update(&context.participant);

    let building_row = town_runtime::create_building(
        &town,
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
        if town_runtime::recruit_pool(&town, unit.id())?.is_none() {
            town_runtime::create_recruit_pool(
                &town,
                unit.id(),
                unit_slug.clone(),
                u32::from(unit.weekly_growth),
                1,
            )?;
        }
    }
    let mut town = town;
    town.last_built_turn = context.session.current_turn;
    town.last_command_id = Some(command.command_id.key());
    town_runtime::mirror_town(&town);

    let mut session = context.session.clone();
    let events = append_runtime_town_command_events(
        &context,
        &mut session,
        &command.command_id_text,
        format!("town_build:{}:{building_slug}", town.id()),
        "town_building_built",
        &town.id().to_string(),
        &context.participant.id().to_string(),
        format!(r#"{{"building_slug":"{}"}}"#, building_slug),
    )?;
    context.session = session;

    apply_runtime_town_command(
        caller,
        &context,
        command,
        &client_nonce,
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
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let unit_slug = slug_from_public_id(&unit_id, "unit:");
    let payload_json = format!(
        r#"{{"town_id":"{}","unit_slug":"{}","quantity":{},"target":{}}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&unit_slug),
        quantity,
        recruit_target_json(&target)
    );
    let command = match begin_runtime_town_command(
        caller,
        &context,
        "submit_recruit_units",
        &client_nonce,
        payload_json,
    )? {
        RuntimeTownCommandAction::Apply(command) => command,
        RuntimeTownCommandAction::Return(response) => return Ok(response),
    };

    let ruleset_id =
        Id::<domm_degens_schema::schema::RulesetDefinition>::from_key(context.session.ruleset_id);
    let unit = content::find_unit_by_ruleset_slug(ruleset_id, &unit_slug)?
        .ok_or_else(|| ApiError::new("unit_not_found", "unit definition was not found", false))?;
    let Some(mut pool) = town_runtime::recruit_pool(&town, unit.id())? else {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("recruit_pool_empty", "no recruit pool is available", false),
        ));
    };
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new("not_owner", "caller does not own this town", false),
        ));
    }
    if quantity == 0 || pool.available < quantity {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new(
                "recruit_pool_empty",
                "not enough units are available",
                false,
            ),
        ));
    }
    let target_check = match recruit_target_check(
        &context.session,
        &context.participant,
        &town,
        unit.id(),
        quantity,
        &target,
    ) {
        Ok(check) => check,
        Err(error) => {
            return Ok(fail_runtime_town_command(
                caller,
                &context,
                command,
                &client_nonce,
                error,
            ));
        }
    };
    if let Some(reason) = target_check.disabled_reason.clone() {
        return Ok(fail_runtime_town_command(
            caller,
            &context,
            command,
            &client_nonce,
            ApiError::new(
                reason.clone(),
                format!("town recruit target disabled: {reason}"),
                false,
            ),
        ));
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
    spend_resources_runtime(
        context.session.id(),
        &mut context.participant,
        command.command_id,
        &format!("recruit:{unit_slug}"),
        context.session.current_turn,
        &total_cost,
        "recruit",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    session_turn_runtime::mirror_participant_update(&context.participant);

    pool.available = pool.available.saturating_sub(quantity);
    pool.last_command_id = Some(command.command_id.key());
    town_runtime::mirror_recruit_pool(&pool);

    let stack_subject = match (&target, target_check.champion_id) {
        (RecruitTarget::TownGarrison { .. }, _) => {
            let garrison = recruit_to_garrison_runtime(
                &town,
                unit.id(),
                unit_slug.clone(),
                unit.max_hp,
                quantity,
                target.clone(),
                command.command_id,
            )?;
            command_response::changed("town_garrison_stack", &garrison.id().to_string(), "upsert")
        }
        (RecruitTarget::Champion { .. }, Some(champion_id)) => {
            let stack = economy_expansion::recruit_to_champion_stack(
                &context.session,
                champion_id,
                unit.id(),
                unit.max_hp,
                quantity,
                target_check.target_slot_index,
                command.command_id,
            )?;
            command_response::changed("champion_army_stack", &stack.id().to_string(), "upsert")
        }
        (RecruitTarget::Champion { .. }, None) => {
            return Ok(fail_runtime_town_command(
                caller,
                &context,
                command,
                &client_nonce,
                ApiError::new(
                    "champion_not_found",
                    "champion recruit target was not resolved",
                    false,
                ),
            ));
        }
    };
    let mut session = context.session.clone();
    let events = append_runtime_town_command_events(
        &context,
        &mut session,
        &command.command_id_text,
        format!(
            "town_recruit:{}:{unit_slug}:{}",
            town.id(),
            command.command_id
        ),
        "units_recruited",
        &town.id().to_string(),
        &context.participant.id().to_string(),
        format!(r#"{{"unit_slug":"{}","quantity":{}}}"#, unit_slug, quantity),
    )?;
    context.session = session;

    apply_runtime_town_command(
        caller,
        &context,
        command,
        &client_nonce,
        events,
        vec![
            command_response::changed("town", &town.id().to_string(), "update"),
            command_response::changed("town_recruit_pool", &pool.id().to_string(), "update"),
            stack_subject,
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
    town_runtime::load_town_by_public_id(session_id, town_id)?
        .ok_or_else(|| session_context::public_error("not_found", "town not found", false))
}

struct RecruitTargetCheck {
    disabled_reason: Option<String>,
    target_slot_index: Option<u8>,
    champion_id: Option<Id<Champion>>,
}

fn recruit_target_check(
    session: &GameSession,
    participant: &GameParticipant,
    town: &Town,
    unit_id: Id<UnitDefinition>,
    quantity: u32,
    target: &RecruitTarget,
) -> Result<RecruitTargetCheck, ApiError> {
    match target {
        RecruitTarget::TownGarrison { slot_index } => Ok(RecruitTargetCheck {
            disabled_reason: None,
            target_slot_index: *slot_index,
            champion_id: None,
        }),
        RecruitTarget::Champion {
            champion_id,
            slot_index,
        } => {
            let champion = resolve_recruit_champion(session, champion_id)?;
            let champion_id = champion.id();
            let disabled_reason = if champion.participant_id != participant.id().key()
                || champion.status != "active"
                || champion.in_battle_id.is_some()
            {
                Some("not_active_stack".to_string())
            } else if champion.x != town.x || champion.y != town.y {
                Some("champion_not_at_town".to_string())
            } else {
                None
            };
            if disabled_reason.is_some() {
                return Ok(RecruitTargetCheck {
                    disabled_reason,
                    target_slot_index: *slot_index,
                    champion_id: Some(champion_id),
                });
            }
            let resolved_slot_index = economy_expansion::resolve_champion_recruit_stack_slot(
                champion_id,
                unit_id,
                quantity,
                *slot_index,
            )?;
            Ok(RecruitTargetCheck {
                disabled_reason: None,
                target_slot_index: Some(resolved_slot_index),
                champion_id: Some(champion_id),
            })
        }
    }
}

fn resolve_recruit_champion(
    session: &GameSession,
    champion_id: &str,
) -> Result<Champion, ApiError> {
    let session_id_text = session.id().to_string();
    if let Ok(id) = session_context::parse_id::<Champion>(champion_id, "champion_id") {
        if let Some(champion) =
            session_turn_runtime::champion_snapshot(&session_id_text, &id.to_string())
        {
            if champion.session_id != session.id().key() {
                return Err(ApiError::new(
                    "champion_wrong_session",
                    "champion does not belong to this session",
                    false,
                ));
            }
            return Ok(champion);
        }
        let champion = champions_artifacts::load_champion(id)?
            .ok_or_else(|| ApiError::new("champion_not_found", "champion was not found", false))?;
        if champion.session_id != session.id().key() {
            return Err(ApiError::new(
                "champion_wrong_session",
                "champion does not belong to this session",
                false,
            ));
        }
        return Ok(champion);
    }

    let start = domm_game::first_playable_scenario()
        .starts
        .into_iter()
        .find(|start| start.champion_key == champion_id)
        .ok_or_else(|| ApiError::new("champion_not_found", "champion was not found", false))?;
    if let Some(champion) = session_turn_runtime::champion_snapshot_by_start(
        &session_id_text,
        start.champion_x,
        start.champion_y,
    ) {
        return Ok(champion);
    }
    champions_artifacts::find_champion_by_session_xy(
        session.id(),
        start.champion_x,
        start.champion_y,
    )?
    .ok_or_else(|| ApiError::new("champion_not_found", "champion was not found", false))
}

fn missing_required_building_slug(
    built_building_slugs: &[String],
    building: &domm_degens_schema::schema::BuildingDefinition,
) -> Option<String> {
    for required in &building.requires_building_slugs {
        if !built_building_slugs.iter().any(|slug| slug == required) {
            return Some(format!("missing_prerequisite:{required}"));
        }
    }
    None
}

fn begin_runtime_town_command(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command_type: &str,
    client_nonce_text: &str,
    payload_json: String,
) -> Result<RuntimeTownCommandAction, ApiError> {
    let payload_hash = command_response::payload_hash(
        command_type,
        &context.participant.id().to_string(),
        client_nonce_text,
        &payload_json,
    );
    if payload_json.len() > domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES {
        return Ok(RuntimeTownCommandAction::Return(
            runtime_town_failed_response(
                caller,
                context,
                Ulid::generate().to_string(),
                command_type.to_string(),
                client_nonce_text,
                payload_hash,
                session_context::public_error(
                    "payload_too_large",
                    "game command payload is too large",
                    false,
                ),
            ),
        ));
    }

    let client_nonce = command_response::nonce_u64(command_type, client_nonce_text);
    let session_id = context.session.id().to_string();
    let actor_participant_id = context.participant.id().to_string();
    if let Some(existing) = session_turn_runtime::command_receipt_by_nonce(
        &session_id,
        &actor_participant_id,
        client_nonce,
    ) {
        if existing.payload_hash != payload_hash {
            return Ok(RuntimeTownCommandAction::Return(
                runtime_town_failed_response(
                    caller,
                    context,
                    Ulid::generate().to_string(),
                    command_type.to_string(),
                    client_nonce_text,
                    payload_hash,
                    session_context::public_error(
                        "duplicate_nonce_payload_mismatch",
                        format!(
                            "client nonce {client_nonce_text} was reused with a different payload"
                        ),
                        false,
                    ),
                ),
            ));
        }
        return Ok(RuntimeTownCommandAction::Return(existing.response));
    }

    ensure_town_command_runtime(context)?;
    if !command_response::runtime_proves_pre_deadline_turn_open(context) {
        command_response::ensure_map_turn_accepts_new_command(context, command_type)?;
    }
    let command_id = Id::<GameCommand>::from_key(Ulid::generate());
    Ok(RuntimeTownCommandAction::Apply(RuntimeTownCommand {
        command_id_text: command_id.to_string(),
        command_id,
        client_nonce,
        payload_hash,
        #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
        payload_json,
        command_type: command_type.to_string(),
    }))
}

fn ensure_town_command_runtime(
    context: &session_context::SessionCallerContext,
) -> Result<(), ApiError> {
    let mut session = context.session.clone();
    session_turn_runtime::ensure_active_turn_runtime(&mut session)?;
    session_turn_runtime::mirror_participant_update(&context.participant);
    Ok(())
}

fn fail_runtime_town_command(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command: RuntimeTownCommand,
    client_nonce_text: &str,
    error: ApiError,
) -> CommandResponse {
    let response = runtime_town_failed_response(
        caller,
        context,
        command.command_id_text.clone(),
        command.command_type.clone(),
        client_nonce_text,
        command.payload_hash.clone(),
        error,
    );
    remember_runtime_town_receipt(
        context,
        command,
        client_nonce_text.to_string(),
        response.clone(),
    );
    response
}

fn runtime_town_failed_response(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command_id: String,
    command_type: String,
    client_nonce_text: &str,
    payload_hash: String,
    error: ApiError,
) -> CommandResponse {
    let retryable = error.retryable;
    command_response::runtime_command_response(
        caller,
        context,
        command_id,
        command_type,
        client_nonce_text,
        payload_hash,
        CommandStatus::Failed,
        CommandPhase::Failed,
        retryable,
        Vec::new(),
        Vec::new(),
        CommandResult::None,
        Some(error),
    )
}

fn apply_runtime_town_command(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command: RuntimeTownCommand,
    client_nonce_text: &str,
    events: Vec<domm_game::ApiEventView>,
    changed_subjects: Vec<domm_game::ChangedSubject>,
) -> Result<CommandResponse, ApiError> {
    let event_count = events.len() as u32;
    let response = command_response::runtime_command_response(
        caller,
        context,
        command.command_id_text.clone(),
        command.command_type.clone(),
        client_nonce_text,
        command.payload_hash.clone(),
        CommandStatus::Applied,
        CommandPhase::Complete,
        false,
        events,
        changed_subjects,
        CommandResult::StrategicReceipt(StrategicCommandReceipt {
            command_kind: command.command_type.clone(),
            command_id: command.command_id_text.clone(),
            current_turn: context.session.current_turn,
            command_count: 1,
            event_count,
        }),
        None,
    );
    remember_runtime_town_receipt(
        context,
        command,
        client_nonce_text.to_string(),
        response.clone(),
    );
    session_context::remember_active_session_caller(caller, context);
    Ok(response)
}

fn remember_runtime_town_receipt(
    context: &session_context::SessionCallerContext,
    command: RuntimeTownCommand,
    client_nonce_text: String,
    response: CommandResponse,
) {
    let receipt = session_turn_runtime::SessionTurnCommandReceipt {
        command_id: command.command_id_text,
        command_type: command.command_type,
        actor_participant_id: context.participant.id().to_string(),
        client_nonce_text,
        client_nonce: command.client_nonce,
        turn_number: context.session.current_turn,
        payload_hash: command.payload_hash,
        #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
        payload_json: Some(command.payload_json),
        response,
    };
    session_turn_runtime::with_runtime_mut(
        &context.session.id().to_string(),
        context.session.current_turn,
        |runtime| runtime.insert_command_receipt(receipt),
    );
}

fn append_runtime_town_command_events(
    context: &session_context::SessionCallerContext,
    session: &mut GameSession,
    command_id_text: &str,
    event_key: String,
    event_type: &str,
    town_id: &str,
    participant_id: &str,
    detailed_payload: String,
) -> Result<Vec<domm_game::ApiEventView>, ApiError> {
    let session_id = context.session.id().to_string();
    let turn_number = context.session.current_turn;
    session_turn_runtime::with_runtime_mut(&session_id, turn_number, |runtime| {
        let audience_key = format!("participant:{participant_id}");
        let private_event = push_runtime_town_event(
            runtime,
            session,
            command_id_text,
            format!("{event_key}:{audience_key}"),
            audience_key,
            event_type,
            town_id,
            detailed_payload,
        )?;
        let public_event = push_runtime_town_event(
            runtime,
            session,
            command_id_text,
            format!("{event_key}:public"),
            "public".to_string(),
            event_type,
            town_id,
            format!(
                r#"{{"town_id":"{}","event_type":"{}","redacted":true}}"#,
                command_response::escape_json(town_id),
                command_response::escape_json(event_type)
            ),
        )?;
        Ok(vec![private_event, public_event])
    })
    .ok_or_else(|| {
        session_context::public_error(
            "turn_runtime_missing",
            "active turn runtime was not available",
            true,
        )
    })?
}

#[allow(clippy::too_many_arguments)]
fn push_runtime_town_event(
    runtime: &mut session_turn_runtime::SessionTurnRuntime,
    session: &mut GameSession,
    command_id_text: &str,
    event_key: String,
    audience_key: String,
    event_type: &str,
    town_id: &str,
    payload_json: String,
) -> Result<domm_game::ApiEventView, ApiError> {
    let event_seq = session_turn_runtime::reserve_session_event_seq(runtime, session)?;
    let event = domm_game::ApiEventView {
        session_id: session.id().to_string(),
        event_seq,
        event_key,
        audience_key,
        turn_number: session.current_turn,
        event_type: event_type.to_string(),
        subject_kind: Some("town".to_string()),
        subject_id_text: Some(town_id.to_string()),
        payload: Some(payload_json),
        redacted: false,
    };
    runtime.push_event(session_turn_runtime::SessionTurnEvent {
        command_id: Some(command_id_text.to_string()),
        event: event.clone(),
        flushed: false,
    });
    Ok(event)
}

fn recruit_to_garrison_runtime(
    town: &Town,
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
    let stack = match town_runtime::garrison_stack(town, slot_index)? {
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
            town_runtime::mirror_garrison_stack(&stack);
            stack
        }
        None => town_runtime::create_garrison_stack(
            town, unit_id, unit_slug, slot_index, quantity, front_hp, command_id,
        )?,
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

fn spend_resources_runtime(
    session_id: Id<domm_degens_schema::schema::GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
    ledger_prefix: &str,
    turn_number: u32,
    cost: &domm_game::ResourceBalances,
    reason: &str,
) -> Result<(), ApiError> {
    #[cfg(feature = "benchmark")]
    let _ = (ledger_prefix, reason);

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
        let delta = -amount;
        #[cfg(feature = "benchmark")]
        {
            apply_resource_balance_delta(participant, resource_key, delta)?;
            session_turn_runtime::record_resource_delta(
                &session_id.to_string(),
                turn_number,
                &participant.id().to_string(),
                resource_key,
                delta,
            );
        }
        #[cfg(not(feature = "benchmark"))]
        {
            let balance_after = apply_resource_balance_delta(participant, resource_key, delta)?;
            session_turn_runtime::record_resource_ledger_delta(
                &session_id.to_string(),
                turn_number,
                &participant.id().to_string(),
                &command_id.to_string(),
                format!("{ledger_prefix}:{resource_key}"),
                resource_key,
                delta,
                balance_after,
                reason,
            );
        }
    }
    participant.last_resource_command_id = Some(command_id.key());
    Ok(())
}

fn apply_resource_balance_delta(
    participant: &mut GameParticipant,
    resource_key: &str,
    delta: i64,
) -> Result<u64, ApiError> {
    match resource_key {
        "gold" => {
            participant.gold = apply_u64_delta(participant.gold, delta)?;
            Ok(participant.gold)
        }
        "wood" => {
            participant.wood = apply_u32_delta(participant.wood, delta)?;
            Ok(u64::from(participant.wood))
        }
        "stone" => {
            participant.stone = apply_u32_delta(participant.stone, delta)?;
            Ok(u64::from(participant.stone))
        }
        "iron" => {
            participant.iron = apply_u32_delta(participant.iron, delta)?;
            Ok(u64::from(participant.iron))
        }
        "crystal" => {
            participant.crystal = apply_u32_delta(participant.crystal, delta)?;
            Ok(u64::from(participant.crystal))
        }
        "ember" => {
            participant.ember = apply_u32_delta(participant.ember, delta)?;
            Ok(u64::from(participant.ember))
        }
        "aether" => {
            participant.aether = apply_u32_delta(participant.aether, delta)?;
            Ok(u64::from(participant.aether))
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
