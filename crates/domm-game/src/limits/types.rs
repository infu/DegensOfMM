use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_ACTIVE_SESSIONS_PER_CANISTER: u32 = 100;
pub const MAX_PARTICIPANTS_PER_SESSION: usize = 2;
pub const MAX_CHAMPIONS_PER_PARTICIPANT: usize = 3;
pub const MAX_TOWNS_PER_SESSION: usize = 6;
pub const MAX_MAP_WIDTH: u16 = 48;
pub const MAX_MAP_HEIGHT: u16 = 48;
pub const MAX_MAP_CHUNKS_PER_SESSION: u32 = 9;
pub const MAX_DYNAMIC_OBJECTS_PER_SESSION: usize = 200;
pub const MAX_ACTIVE_BATTLES_PER_SESSION: u32 = 2;
pub const MAX_STACKS_PER_BATTLE_SIDE: usize = 7;
pub const MAX_BATTLE_OBSTACLES: usize = 16;
pub const MAX_BATTLE_ROUNDS: u16 = 20;
pub const MAX_AI_ACTORS_PER_UPDATE: u16 = 2;
pub const MAX_AI_CANDIDATES_PER_ACTOR: u16 = 16;
pub const MAX_AI_PATH_NODES_PER_ACTOR: u16 = 128;
pub const MAX_AI_CHUNKS_LOADED_PER_ACTOR: u16 = 8;
pub const MAX_AI_EMITTED_COMMANDS_PER_UPDATE: u16 = 2;
pub const MAX_EVENTS_PER_TURN: usize = 100;
pub const MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION: usize = 2_000;
pub const MAX_EVENTS_RETAINED_PER_ACTIVE_SESSION: usize = 5_000;
pub const MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION: usize = 3_000;
pub const MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN: u32 = 6;
pub const MAX_MOVEMENT_MICROSTEPS_PER_SYNC: u32 = 384;
pub const MAX_BATTLE_STARTS_FROM_MOVEMENT: u32 = 2;
pub const MAX_OBJECT_INTERACTIONS_FROM_MOVEMENT: u32 = 6;
pub const MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE: u32 = 2;
pub const MAX_CLEANUP_ROWS_PER_UPDATE: u32 = 100;
pub const MAX_FINISHED_SESSIONS_CLEANED_PER_UPDATE: u32 = 1;
pub const DEFAULT_LIST_LIMIT: u32 = 50;
pub const MAX_LIST_LIMIT: u32 = 200;
pub const MAX_VIEWPORT_CHUNKS_PER_REQUEST: u32 = 9;
pub const MAX_RECENT_EVENTS_IN_GAME_VIEW: u32 = 50;
pub const MAX_COMMAND_PAYLOAD_JSON_BYTES: usize = 4_096;
pub const MAX_COMMAND_RESULT_JSON_BYTES: usize = 4_096;
pub const MAX_EVENT_PAYLOAD_JSON_BYTES: usize = 4_096;
pub const MAX_MOVEMENT_INTENT_PATH_JSON_BYTES: usize = 2_048;
pub const MAX_MOVE_PATH_STEPS_LIMIT: usize = 64;
pub const MAX_MOVE_CHUNKS_TOUCHED_LIMIT: usize = 8;
pub const TURN_CATCHUP_CAP: u32 = 10;
pub const RECOVERY_COMMANDS_INSPECTED_PER_UPDATE: u32 = 25;
pub const RECOVERY_COMMANDS_ADVANCED_PER_UPDATE: u32 = 8;
pub const RECOVERY_COMMAND_EFFECTS_PER_UPDATE: u32 = 32;
pub const RECOVERY_GAME_EVENTS_PER_UPDATE: u32 = 32;
pub const RECOVERY_GAMEPLAY_ROWS_PER_UPDATE: u32 = 160;
pub const GAME_COMMAND_EFFECT_CAP: usize = 16;
pub const GAME_COMMAND_EVENT_CAP: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PerformanceBudgetReport {
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub storage_row_count: u32,
    pub max_query_bytes: u32,
    pub estimated_response_bytes: u32,
    pub max_query_under_budget: bool,
    pub storage_under_active_row_budget: bool,
    pub concerns: Vec<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PerformanceBudgetError {
    #[error(transparent)]
    Playable(#[from] crate::playable::PlayableError),
}
