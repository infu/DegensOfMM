use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    GameCommand, GameSession, NavalRouteState, ProceduralMapState, SiegeRuleState,
    SkirmishSettingsState,
};
use domm_game::{
    ApiError, ChangedSubject, CommandResponse, CommandResult, NAVAL_ROUTE_KEY, NavalRouteRecord,
    NavalRoutesView, PROCEDURAL_GENERATION_KEY, ProceduralMapRecord, ProceduralMapView,
    SIEGE_RULE_KEY, SiegeRuleRecord, SiegeRulesView, SkirmishSettingsRecord, SkirmishSettingsView,
    WorldGenerationReceipt, WorldgenError, deterministic_procedural_map,
    first_playable_naval_route, first_playable_siege_rule, first_playable_skirmish_settings,
};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::worldgen;

use super::{
    command_response::{self, GameCommandAction},
    session_context::{self, public_error},
};

pub(crate) fn get_skirmish_settings(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<SkirmishSettingsView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let settings = worldgen::find_skirmish_settings(context.session.id())?.ok_or_else(|| {
        public_error(
            "skirmish_settings_missing",
            "skirmish settings missing",
            true,
        )
    })?;
    Ok(SkirmishSettingsView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        settings: skirmish_settings_record(settings),
    })
}

pub(crate) fn get_procedural_map_state(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<ProceduralMapView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let maps = worldgen::page_procedural_maps_by_status(context.session.id(), "validated")?
        .items
        .into_iter()
        .map(procedural_map_record)
        .collect();
    Ok(ProceduralMapView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        maps,
    })
}

pub(crate) fn get_naval_routes(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<NavalRoutesView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let routes = worldgen::page_naval_routes_by_status(context.session.id(), "disabled")?
        .items
        .into_iter()
        .map(naval_route_record)
        .collect();
    Ok(NavalRoutesView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        routes,
    })
}

pub(crate) fn get_siege_rules(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<SiegeRulesView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let rules = worldgen::page_siege_rules_by_status(context.session.id(), "disabled")?
        .items
        .into_iter()
        .map(siege_rule_record)
        .collect();
    Ok(SiegeRulesView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        rules,
    })
}

pub(crate) fn sync_world_generation(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "sync_world_generation",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let map = ensure_seeded_worldgen_state(&context.session, Some(command.id()))?;
    let receipt = receipt_from_map(command.id(), context.session.current_turn, &map);
    let result_json = receipt_json(&receipt);
    let session_id_text = context.session.id().to_string();
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        "worldgen:sync_world_generation".to_string(),
        "world_generation_sync".to_string(),
        "session".to_string(),
        session_id_text,
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
        &client_nonce,
        result_json,
        Vec::new(),
        vec![changed(
            "procedural_map",
            &map.id().to_string(),
            "validated",
        )],
        CommandResult::WorldGeneration(receipt),
    )
}

pub(crate) fn ensure_seeded_worldgen_state(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<ProceduralMapState, ApiError> {
    let settings_record = first_playable_skirmish_settings(session.seed);
    let settings = match worldgen::find_skirmish_settings(session.id())? {
        Some(row) => row,
        None => worldgen::create_skirmish_settings(session.id(), &settings_record)?,
    };

    let preview = deterministic_procedural_map(
        settings.map_seed,
        settings.map_width,
        settings.map_height,
        settings.chunk_size,
        session.current_turn,
    )
    .map_err(worldgen_error)?;

    let mut map =
        match worldgen::find_procedural_map_by_key(session.id(), PROCEDURAL_GENERATION_KEY)? {
            Some(row) => row,
            None => worldgen::create_procedural_map(session.id(), &preview, command_id)?,
        };
    if map.status != preview.status
        || map.scenario_hash != preview.scenario_hash
        || map.generated_turn != preview.generated_turn
    {
        apply_procedural_preview(&mut map, &preview);
        map.last_command_id = command_id.map(|id| id.key());
        map = worldgen::update_procedural_map(map)?;
    }

    if worldgen::find_naval_route_by_key(session.id(), NAVAL_ROUTE_KEY)?.is_none() {
        worldgen::create_naval_route(session.id(), &first_playable_naval_route())?;
    }
    if worldgen::find_siege_rule_by_key(session.id(), SIEGE_RULE_KEY)?.is_none() {
        worldgen::create_siege_rule(session.id(), &first_playable_siege_rule())?;
    }
    Ok(map)
}

fn apply_procedural_preview(row: &mut ProceduralMapState, record: &ProceduralMapRecord) {
    row.status = record.status.clone();
    row.map_seed = record.map_seed;
    row.map_width = record.map_width;
    row.map_height = record.map_height;
    row.chunk_size = record.chunk_size;
    row.chunk_count = record.chunk_count;
    row.land_tile_count = record.land_tile_count;
    row.water_tile_count = record.water_tile_count;
    row.road_tile_count = record.road_tile_count;
    row.town_count = record.town_count;
    row.mine_count = record.mine_count;
    row.scenario_hash = record.scenario_hash.clone();
    row.generated_turn = record.generated_turn;
}

fn skirmish_settings_record(row: SkirmishSettingsState) -> SkirmishSettingsRecord {
    SkirmishSettingsRecord {
        profile_key: row.profile_key,
        status: row.status,
        map_seed: row.map_seed,
        map_width: row.map_width,
        map_height: row.map_height,
        chunk_size: row.chunk_size,
        player_count: row.player_count,
        fog_enabled: row.fog_enabled,
        neutral_difficulty: row.neutral_difficulty,
        victory_condition: row.victory_condition,
        generation_key: row.generation_key,
        naval_enabled: row.naval_enabled,
        siege_enabled: row.siege_enabled,
        larger_map_enabled: row.larger_map_enabled,
    }
}

fn procedural_map_record(row: ProceduralMapState) -> ProceduralMapRecord {
    ProceduralMapRecord {
        generation_key: row.generation_key,
        status: row.status,
        map_seed: row.map_seed,
        map_width: row.map_width,
        map_height: row.map_height,
        chunk_size: row.chunk_size,
        chunk_count: row.chunk_count,
        land_tile_count: row.land_tile_count,
        water_tile_count: row.water_tile_count,
        road_tile_count: row.road_tile_count,
        town_count: row.town_count,
        mine_count: row.mine_count,
        scenario_hash: row.scenario_hash,
        generated_turn: row.generated_turn,
    }
}

fn naval_route_record(row: NavalRouteState) -> NavalRouteRecord {
    NavalRouteRecord {
        route_key: row.route_key,
        status: row.status,
        actionable: false,
        from_x: row.from_x,
        from_y: row.from_y,
        to_x: row.to_x,
        to_y: row.to_y,
        water_crossings: row.water_crossings,
        boat_required: row.boat_required,
        disabled_reason: row.disabled_reason,
    }
}

fn siege_rule_record(row: SiegeRuleState) -> SiegeRuleRecord {
    SiegeRuleRecord {
        rule_key: row.rule_key,
        status: row.status,
        actionable: false,
        fortification_level: row.fortification_level,
        wall_segments: row.wall_segments,
        gate_count: row.gate_count,
        tower_count: row.tower_count,
        siege_engine_slots: row.siege_engine_slots,
        battle_obstacle_cap: row.battle_obstacle_cap,
        disabled_reason: row.disabled_reason,
    }
}

fn receipt_from_map(
    command_id: Id<GameCommand>,
    current_turn: u32,
    map: &ProceduralMapState,
) -> WorldGenerationReceipt {
    WorldGenerationReceipt {
        command_id: command_id.to_string(),
        action: "sync_world_generation".to_string(),
        generation_key: map.generation_key.clone(),
        state: map.status.clone(),
        current_turn,
        map_width: map.map_width,
        map_height: map.map_height,
        chunk_count: map.chunk_count,
        scenario_hash: map.scenario_hash.clone(),
    }
}

pub(crate) fn receipt_json(receipt: &WorldGenerationReceipt) -> String {
    format!(
        r#"{{"command_id":"{}","action":"{}","generation_key":"{}","state":"{}","current_turn":{},"map_width":{},"map_height":{},"chunk_count":{},"scenario_hash":"{}"}}"#,
        command_response::escape_json(&receipt.command_id),
        command_response::escape_json(&receipt.action),
        command_response::escape_json(&receipt.generation_key),
        command_response::escape_json(&receipt.state),
        receipt.current_turn,
        receipt.map_width,
        receipt.map_height,
        receipt.chunk_count,
        command_response::escape_json(&receipt.scenario_hash)
    )
}

fn changed(subject_kind: &str, subject_id_text: &str, operation: &str) -> ChangedSubject {
    ChangedSubject {
        subject_kind: subject_kind.to_string(),
        subject_id_text: subject_id_text.to_string(),
        operation: operation.to_string(),
    }
}

fn worldgen_error(error: WorldgenError) -> ApiError {
    ApiError::new(error.code, error.message, false)
}
