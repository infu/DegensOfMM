use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::effects::EffectError;
use crate::rng::{RngError, RollAudit};

pub const BATTLE_GRID_WIDTH: u8 = 12;
pub const BATTLE_GRID_HEIGHT: u8 = 10;
pub const BATTLE_MAX_ROUNDS: u16 = 20;
pub const BATTLE_ACTION_DEADLINE_MS: u64 = 30_000;

pub const BATTLE_SIDE_ATTACKER: &str = "attacker";
pub const BATTLE_SIDE_DEFENDER: &str = "defender";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, CandidType, Serialize, Deserialize)]
pub struct BattleCoord {
    pub x: u8,
    pub y: u8,
}

impl BattleCoord {
    #[must_use]
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn manhattan(self, other: Self) -> u16 {
        let dx = self.x.abs_diff(other.x);
        let dy = self.y.abs_diff(other.y);
        u16::from(dx) + u16::from(dy)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleRecord {
    pub battle_id: String,
    pub session_id: String,
    pub state: String,
    pub battle_type: String,
    pub attacker_champion_id: Option<String>,
    pub defender_champion_id: Option<String>,
    pub defender_town_id: Option<String>,
    pub defender_neutral_army_id: Option<String>,
    pub current_round: u16,
    pub active_side: String,
    pub active_stack_id: Option<String>,
    pub grid_width: u8,
    pub grid_height: u8,
    pub max_rounds: u16,
    pub turn_seed: u64,
    pub winner_participant_id: Option<String>,
    pub created_turn: u32,
    pub action_deadline_at: Option<u64>,
    pub resolved_at: Option<u64>,
    pub cleanup_after_turn: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleStackRecord {
    pub battle_stack_id: String,
    pub battle_id: String,
    pub unit_id: String,
    pub owner_participant_id: Option<String>,
    pub side: String,
    pub slot_index: u8,
    pub origin_kind: String,
    pub origin_stack_id_text: Option<String>,
    pub origin_slot_index: u8,
    pub champion_might: i16,
    pub champion_guard: i16,
    pub attack: i16,
    pub defense: i16,
    pub damage_min: u16,
    pub damage_max: u16,
    pub max_hp: u16,
    pub speed: u8,
    pub initiative: u8,
    pub ranged: bool,
    pub flying: bool,
    pub quantity: u32,
    pub front_hp: u16,
    pub shots_remaining: u16,
    pub battle_x: u8,
    pub battle_y: u8,
    pub readiness: u16,
    pub acted_round: u16,
    pub retaliated_round: u16,
    pub defended_round: u16,
    pub waited_round: u16,
    pub cast_round: u16,
    pub status: String,
    pub last_command_id: Option<String>,
    pub status_keys: Vec<String>,
}

impl BattleStackRecord {
    #[must_use]
    pub fn coord(&self) -> BattleCoord {
        BattleCoord::new(self.battle_x, self.battle_y)
    }

    #[must_use]
    pub fn is_living(&self) -> bool {
        self.status == "active" && self.quantity > 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleObstacleRecord {
    pub battle_obstacle_id: String,
    pub battle_id: String,
    pub obstacle_type: String,
    pub battle_x: u8,
    pub battle_y: u8,
    pub width: u8,
    pub height: u8,
    pub hp: u16,
    pub state: String,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleOccupancyRecord {
    pub battle_occupancy_id: String,
    pub battle_id: String,
    pub battle_stack_id: String,
    pub battle_x: u8,
    pub battle_y: u8,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleState {
    pub session_seed: String,
    pub battles: Vec<BattleRecord>,
    pub stacks: Vec<BattleStackRecord>,
    pub obstacles: Vec<BattleObstacleRecord>,
    pub occupancy: Vec<BattleOccupancyRecord>,
}

impl BattleState {
    pub fn battle(&self, battle_id: &str) -> Result<&BattleRecord, BattleError> {
        self.battles
            .iter()
            .find(|battle| battle.battle_id == battle_id)
            .ok_or_else(|| BattleError::BattleNotFound {
                battle_id: battle_id.to_string(),
            })
    }

    pub fn battle_mut(&mut self, battle_id: &str) -> Result<&mut BattleRecord, BattleError> {
        self.battles
            .iter_mut()
            .find(|battle| battle.battle_id == battle_id)
            .ok_or_else(|| BattleError::BattleNotFound {
                battle_id: battle_id.to_string(),
            })
    }

    pub fn stack(&self, battle_stack_id: &str) -> Result<&BattleStackRecord, BattleError> {
        self.stacks
            .iter()
            .find(|stack| stack.battle_stack_id == battle_stack_id)
            .ok_or_else(|| BattleError::StackNotFound {
                battle_stack_id: battle_stack_id.to_string(),
            })
    }

    pub fn stack_mut(
        &mut self,
        battle_stack_id: &str,
    ) -> Result<&mut BattleStackRecord, BattleError> {
        self.stacks
            .iter_mut()
            .find(|stack| stack.battle_stack_id == battle_stack_id)
            .ok_or_else(|| BattleError::StackNotFound {
                battle_stack_id: battle_stack_id.to_string(),
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleGridView {
    pub width: u8,
    pub height: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleObstacleView {
    pub battle_obstacle_id: String,
    pub obstacle_type: String,
    pub battle_x: u8,
    pub battle_y: u8,
    pub width: u8,
    pub height: u8,
    pub hp: u16,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleStackView {
    pub battle_stack_id: String,
    pub unit_id: String,
    pub side: String,
    pub owner_participant_id: Option<String>,
    pub battle_x: u8,
    pub battle_y: u8,
    pub quantity: u32,
    pub front_hp: u16,
    pub shots_remaining: u16,
    pub champion_might: i16,
    pub champion_guard: i16,
    pub attack: i16,
    pub defense: i16,
    pub damage_min: u16,
    pub damage_max: u16,
    pub max_hp: u16,
    pub speed: u8,
    pub initiative: u8,
    pub ranged: bool,
    pub flying: bool,
    pub acted_round: u16,
    pub waited_round: u16,
    pub defended_round: u16,
    pub status: String,
    pub status_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DamagePreview {
    pub target_stack_id: String,
    pub min_damage: u32,
    pub max_damage: u32,
    pub estimated_kills_min: u32,
    pub estimated_kills_max: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LegalBattleAction {
    pub action: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    pub targets: Vec<String>,
    pub path: Vec<BattleCoord>,
    pub damage_preview: Option<DamagePreview>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleEventView {
    pub event_seq: u64,
    pub event_key: String,
    pub event_type: String,
    pub subject_id_text: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleMoraleLuckPolicy {
    pub morale_enabled: bool,
    pub morale_disabled_reason: Option<String>,
    pub luck_enabled: bool,
    pub luck_disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleView {
    pub battle_id: String,
    pub state: String,
    pub battle_type: String,
    pub current_round: u16,
    pub active_stack_id: Option<String>,
    pub active_participant_id: Option<String>,
    pub action_deadline_at: Option<u64>,
    pub remaining_ms: Option<u64>,
    pub grid: BattleGridView,
    pub obstacles: Vec<BattleObstacleView>,
    pub stacks: Vec<BattleStackView>,
    pub initiative_order: Vec<String>,
    pub legal_actions_for_caller: Vec<LegalBattleAction>,
    pub events: Vec<BattleEventView>,
    pub next_event_seq: u64,
    pub morale_luck_policy: BattleMoraleLuckPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleInitiativeEntry {
    pub battle_stack_id: String,
    pub side: String,
    pub initiative: u8,
    pub speed: u8,
    pub waited: bool,
    pub tie_breaker: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleDamageOutcome {
    pub attacker_stack_id: String,
    pub target_stack_id: String,
    pub rolled_damage_per_unit: u16,
    pub final_damage: u32,
    pub killed: u32,
    pub target_quantity_after: u32,
    pub target_front_hp_after: u16,
    pub roll_audit: RollAudit,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleSmokeView {
    pub battle_id: String,
    pub active_stack_id: String,
    pub stack_count: u32,
    pub obstacle_count: u32,
    pub legal_action_count: u32,
    pub first_damage: BattleDamageOutcome,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BattleError {
    #[error("battle not found: {battle_id}")]
    BattleNotFound { battle_id: String },
    #[error("battle stack not found: {battle_stack_id}")]
    StackNotFound { battle_stack_id: String },
    #[error("unit not found: {unit_slug}")]
    UnitNotFound { unit_slug: String },
    #[error("tile is out of bounds: {x},{y}")]
    OutOfBounds { x: u8, y: u8 },
    #[error("tile is occupied in battle {battle_id}: {x},{y}")]
    OccupiedTile { battle_id: String, x: u8, y: u8 },
    #[error("stack has duplicate occupancy: {battle_stack_id}")]
    DuplicateStackOccupancy { battle_stack_id: String },
    #[error("living stack is missing occupancy: {battle_stack_id}")]
    MissingStackOccupancy { battle_stack_id: String },
    #[error("tile has duplicate occupancy in battle {battle_id}: {x},{y}")]
    DuplicateTileOccupancy { battle_id: String, x: u8, y: u8 },
    #[error(
        "occupancy cache mismatch for {battle_stack_id}: stack {stack_x},{stack_y}, occupancy {occupancy_x},{occupancy_y}"
    )]
    OccupancyCacheMismatch {
        battle_stack_id: String,
        stack_x: u8,
        stack_y: u8,
        occupancy_x: u8,
        occupancy_y: u8,
    },
    #[error("tile is blocked by an obstacle in battle {battle_id}: {x},{y}")]
    ObstacleBlocked { battle_id: String, x: u8, y: u8 },
    #[error("target stack is not an enemy: {target_stack_id}")]
    TargetNotEnemy { target_stack_id: String },
    #[error("target stack is not adjacent: {target_stack_id}")]
    TargetNotAdjacent { target_stack_id: String },
    #[error("ranged stack is blocked by adjacent enemy: {battle_stack_id}")]
    RangedBlockedByAdjacentEnemy { battle_stack_id: String },
    #[error("stack is not ranged: {battle_stack_id}")]
    StackNotRanged { battle_stack_id: String },
    #[error("stack has no shots remaining: {battle_stack_id}")]
    NoShotsRemaining { battle_stack_id: String },
    #[error(transparent)]
    Rng(#[from] RngError),
    #[error(transparent)]
    Effect(#[from] EffectError),
}
