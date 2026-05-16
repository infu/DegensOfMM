use crate::content::{
    FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH,
    FIRST_PLAYABLE_PLAYER_COUNT,
};
use crate::rng::{RollKey, hash64};

use super::types::{
    MAX_GENERATED_CHUNKS_PER_UPDATE, MAX_GENERATED_MAP_HEIGHT, MAX_GENERATED_MAP_WIDTH,
    MAX_SIEGE_OBSTACLES_PER_BATTLE, MAX_WATER_CROSSINGS_PER_PATH, NAVAL_ROUTE_KEY,
    NavalRouteRecord, PROCEDURAL_GENERATION_KEY, ProceduralMapRecord, SIEGE_RULE_KEY,
    SKIRMISH_PROFILE_KEY, SiegeRuleRecord, SkirmishSettingsRecord, WorldgenError,
};

#[must_use]
pub fn first_playable_skirmish_settings(map_seed: u64) -> SkirmishSettingsRecord {
    SkirmishSettingsRecord {
        profile_key: SKIRMISH_PROFILE_KEY.to_string(),
        status: "active".to_string(),
        map_seed,
        map_width: FIRST_PLAYABLE_MAP_WIDTH,
        map_height: FIRST_PLAYABLE_MAP_HEIGHT,
        chunk_size: FIRST_PLAYABLE_CHUNK_SIZE,
        player_count: FIRST_PLAYABLE_PLAYER_COUNT,
        fog_enabled: true,
        neutral_difficulty: "standard".to_string(),
        victory_condition: "conquest".to_string(),
        generation_key: PROCEDURAL_GENERATION_KEY.to_string(),
        naval_enabled: false,
        siege_enabled: false,
        larger_map_enabled: false,
    }
}

pub fn deterministic_procedural_map(
    map_seed: u64,
    map_width: u16,
    map_height: u16,
    chunk_size: u8,
    generated_turn: u32,
) -> Result<ProceduralMapRecord, WorldgenError> {
    validate_generation_caps(map_width, map_height, chunk_size)?;

    let mut water_tile_count = 0_u32;
    let mut road_tile_count = 0_u32;
    for y in 0..map_height {
        for x in 0..map_width {
            if is_main_road_tile(x, y, map_width, map_height) {
                road_tile_count = road_tile_count.saturating_add(1);
                continue;
            }
            if is_deterministic_water_tile(map_seed, x, y) {
                water_tile_count = water_tile_count.saturating_add(1);
            }
        }
    }

    let total_tiles = u32::from(map_width).saturating_mul(u32::from(map_height));
    let land_tile_count = total_tiles.saturating_sub(water_tile_count);
    let chunks_x = u32::from(map_width).div_ceil(u32::from(chunk_size));
    let chunks_y = u32::from(map_height).div_ceil(u32::from(chunk_size));
    let chunk_count = chunks_x.saturating_mul(chunks_y);
    let scenario_hash = procedural_scenario_hash(
        map_seed,
        map_width,
        map_height,
        chunk_size,
        water_tile_count,
        road_tile_count,
    );

    Ok(ProceduralMapRecord {
        generation_key: PROCEDURAL_GENERATION_KEY.to_string(),
        status: "validated".to_string(),
        map_seed,
        map_width,
        map_height,
        chunk_size,
        chunk_count,
        land_tile_count,
        water_tile_count,
        road_tile_count,
        town_count: u32::from(FIRST_PLAYABLE_PLAYER_COUNT),
        mine_count: 2,
        scenario_hash,
        generated_turn,
    })
}

pub fn validate_generation_caps(
    map_width: u16,
    map_height: u16,
    chunk_size: u8,
) -> Result<(), WorldgenError> {
    if chunk_size == 0 {
        return Err(WorldgenError::new(
            "invalid_chunk_size",
            "chunk size must be at least one",
        ));
    }
    if map_width == 0 || map_height == 0 {
        return Err(WorldgenError::new(
            "invalid_map_size",
            "map dimensions must be non-zero",
        ));
    }
    if map_width > MAX_GENERATED_MAP_WIDTH || map_height > MAX_GENERATED_MAP_HEIGHT {
        return Err(WorldgenError::new(
            "generated_map_too_large",
            format!(
                "generated map exceeds {}x{} cap",
                MAX_GENERATED_MAP_WIDTH, MAX_GENERATED_MAP_HEIGHT
            ),
        ));
    }
    let chunks_x = u32::from(map_width).div_ceil(u32::from(chunk_size));
    let chunks_y = u32::from(map_height).div_ceil(u32::from(chunk_size));
    let chunk_count = chunks_x.saturating_mul(chunks_y);
    if chunk_count > MAX_GENERATED_CHUNKS_PER_UPDATE {
        return Err(WorldgenError::new(
            "generated_chunk_cap_exceeded",
            format!("generated map needs {chunk_count} chunks"),
        ));
    }
    Ok(())
}

#[must_use]
pub fn first_playable_naval_route() -> NavalRouteRecord {
    NavalRouteRecord {
        route_key: NAVAL_ROUTE_KEY.to_string(),
        status: "disabled".to_string(),
        from_x: 7,
        from_y: 22,
        to_x: 13,
        to_y: 22,
        water_crossings: 3,
        boat_required: true,
        disabled_reason: Some("checkpoint_25_schema_only".to_string()),
    }
}

#[must_use]
pub fn first_playable_siege_rule() -> SiegeRuleRecord {
    SiegeRuleRecord {
        rule_key: SIEGE_RULE_KEY.to_string(),
        status: "disabled".to_string(),
        fortification_level: "palisade".to_string(),
        wall_segments: 6,
        gate_count: 1,
        tower_count: 1,
        siege_engine_slots: 1,
        battle_obstacle_cap: MAX_SIEGE_OBSTACLES_PER_BATTLE,
        disabled_reason: Some("checkpoint_25_schema_only".to_string()),
    }
}

pub fn validate_boat_movement(
    route: &NavalRouteRecord,
    has_boat: bool,
) -> Result<(), WorldgenError> {
    if route.status != "active" {
        return Err(WorldgenError::new(
            "naval_route_disabled",
            route
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "naval route is disabled".to_string()),
        ));
    }
    if route.water_crossings > MAX_WATER_CROSSINGS_PER_PATH {
        return Err(WorldgenError::new(
            "water_crossing_cap_exceeded",
            format!("path crosses {} water tiles", route.water_crossings),
        ));
    }
    if route.boat_required && !has_boat {
        return Err(WorldgenError::new(
            "boat_required",
            "water route requires a boat",
        ));
    }
    Ok(())
}

fn is_main_road_tile(x: u16, y: u16, map_width: u16, map_height: u16) -> bool {
    x == map_width / 2 || y == map_height / 2
}

fn is_deterministic_water_tile(map_seed: u64, x: u16, y: u16) -> bool {
    let roll_key = RollKey::new(
        &map_seed.to_string(),
        "procedural_map",
        u32::from(y),
        x.to_string(),
        "terrain",
        "water",
        0,
    );
    hash64(&roll_key) % 17 == 0
}

fn procedural_scenario_hash(
    map_seed: u64,
    map_width: u16,
    map_height: u16,
    chunk_size: u8,
    water_tile_count: u32,
    road_tile_count: u32,
) -> String {
    let roll_key = RollKey::new(
        &map_seed.to_string(),
        "procedural_map_hash",
        u32::from(map_width),
        map_height.to_string(),
        &format!("chunk:{chunk_size}"),
        &format!("water:{water_tile_count}:road:{road_tile_count}"),
        0,
    );
    format!("{:016x}", hash64(&roll_key))
}
