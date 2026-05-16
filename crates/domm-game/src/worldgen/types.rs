use candid::CandidType;
use serde::{Deserialize, Serialize};

pub const SKIRMISH_PROFILE_KEY: &str = "skirmish:first-playable-compact";
pub const PROCEDURAL_GENERATION_KEY: &str = "procedural:first-playable-preview";
pub const NAVAL_ROUTE_KEY: &str = "naval:west-river-disabled";
pub const SIEGE_RULE_KEY: &str = "siege:first-playable-disabled";

pub const MAX_GENERATED_MAP_WIDTH: u16 = 64;
pub const MAX_GENERATED_MAP_HEIGHT: u16 = 64;
pub const MAX_GENERATED_CHUNKS_PER_UPDATE: u32 = 16;
pub const MAX_NAVAL_ROUTE_ROWS_PER_SESSION: u32 = 8;
pub const MAX_SIEGE_RULE_ROWS_PER_SESSION: u32 = 8;
pub const MAX_WATER_CROSSINGS_PER_PATH: u16 = 8;
pub const MAX_SIEGE_OBSTACLES_PER_BATTLE: u16 = 24;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SkirmishSettingsRecord {
    pub profile_key: String,
    pub status: String,
    pub map_seed: u64,
    pub map_width: u16,
    pub map_height: u16,
    pub chunk_size: u8,
    pub player_count: u8,
    pub fog_enabled: bool,
    pub neutral_difficulty: String,
    pub victory_condition: String,
    pub generation_key: String,
    pub naval_enabled: bool,
    pub siege_enabled: bool,
    pub larger_map_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SkirmishSettingsView {
    pub session_id: String,
    pub current_turn: u32,
    pub settings: SkirmishSettingsRecord,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ProceduralMapRecord {
    pub generation_key: String,
    pub status: String,
    pub map_seed: u64,
    pub map_width: u16,
    pub map_height: u16,
    pub chunk_size: u8,
    pub chunk_count: u32,
    pub land_tile_count: u32,
    pub water_tile_count: u32,
    pub road_tile_count: u32,
    pub town_count: u32,
    pub mine_count: u32,
    pub scenario_hash: String,
    pub generated_turn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ProceduralMapView {
    pub session_id: String,
    pub current_turn: u32,
    pub maps: Vec<ProceduralMapRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NavalRouteRecord {
    pub route_key: String,
    pub status: String,
    pub from_x: u16,
    pub from_y: u16,
    pub to_x: u16,
    pub to_y: u16,
    pub water_crossings: u16,
    pub boat_required: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NavalRoutesView {
    pub session_id: String,
    pub current_turn: u32,
    pub routes: Vec<NavalRouteRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SiegeRuleRecord {
    pub rule_key: String,
    pub status: String,
    pub fortification_level: String,
    pub wall_segments: u16,
    pub gate_count: u8,
    pub tower_count: u8,
    pub siege_engine_slots: u8,
    pub battle_obstacle_cap: u16,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SiegeRulesView {
    pub session_id: String,
    pub current_turn: u32,
    pub rules: Vec<SiegeRuleRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldGenerationReceipt {
    pub command_id: String,
    pub action: String,
    pub generation_key: String,
    pub state: String,
    pub current_turn: u32,
    pub map_width: u16,
    pub map_height: u16,
    pub chunk_count: u32,
    pub scenario_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldgenError {
    pub code: &'static str,
    pub message: String,
}

impl WorldgenError {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
