use std::fmt::Write as _;

use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::champion::ChampionError;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::limits::{
    MAX_MOVE_CHUNKS_TOUCHED_LIMIT, MAX_MOVE_PATH_STEPS_LIMIT, MAX_MOVEMENT_MICROSTEPS_PER_SYNC,
    RECOVERY_COMMAND_EFFECTS_PER_UPDATE, RECOVERY_COMMANDS_ADVANCED_PER_UPDATE,
    RECOVERY_COMMANDS_INSPECTED_PER_UPDATE, RECOVERY_GAME_EVENTS_PER_UPDATE,
    RECOVERY_GAMEPLAY_ROWS_PER_UPDATE,
};
use crate::map::MapError;

pub const MAX_MOVE_PATH_STEPS: usize = MAX_MOVE_PATH_STEPS_LIMIT;
pub const MAX_MOVE_CHUNKS_TOUCHED: usize = MAX_MOVE_CHUNKS_TOUCHED_LIMIT;
pub const DEFAULT_RECOVERY_COMMANDS_INSPECTED: u32 = RECOVERY_COMMANDS_INSPECTED_PER_UPDATE;
pub const DEFAULT_RECOVERY_COMMANDS_ADVANCED: u32 = RECOVERY_COMMANDS_ADVANCED_PER_UPDATE;
pub const DEFAULT_RECOVERY_EFFECTS: u32 = RECOVERY_COMMAND_EFFECTS_PER_UPDATE;
pub const DEFAULT_RECOVERY_EVENTS: u32 = RECOVERY_GAME_EVENTS_PER_UPDATE;
pub const DEFAULT_GAMEPLAY_ROWS: u32 = RECOVERY_GAMEPLAY_ROWS_PER_UPDATE;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, CandidType, Serialize, Deserialize,
)]
pub struct MoveCoord {
    pub x: u16,
    pub y: u16,
}

impl MoveCoord {
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn is_adjacent_to(self, other: Self) -> bool {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y) == 1
    }
}

impl From<(u16, u16)> for MoveCoord {
    fn from(value: (u16, u16)) -> Self {
        Self::new(value.0, value.1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementPathStop {
    pub reason: String,
    pub subject_kind: String,
    pub subject_id_text: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementPreview {
    pub champion_id: String,
    pub participant_id: String,
    pub turn_number: u32,
    pub path: Vec<MoveCoord>,
    pub total_cost: u16,
    pub available_movement: u16,
    pub chunks_touched: u32,
    pub stop: Option<MovementPathStop>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementIntentRecord {
    pub intent_id: String,
    pub session_id: String,
    pub participant_id: String,
    pub champion_id: String,
    pub turn_number: u32,
    pub client_nonce: u64,
    pub payload_hash: String,
    pub path: Vec<MoveCoord>,
    pub status: String,
    pub submitted_at_ms: u64,
    pub superseded_by_intent_id: Option<String>,
    pub resolved_command_id: Option<String>,
    pub last_command_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementIntentSubmitOutcome {
    pub intent: MovementIntentRecord,
    pub preview: MovementPreview,
    pub replaced_intent_ids: Vec<String>,
    pub command_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementSnapshotRecord {
    pub snapshot_id: String,
    pub session_id: String,
    pub command_id: String,
    pub turn_number: u32,
    pub champion_id: String,
    pub participant_id: String,
    pub intent_id: String,
    pub step_index: u16,
    pub from_x: u16,
    pub from_y: u16,
    pub to_x: u16,
    pub to_y: u16,
    pub movement_cost: u16,
    pub remaining_after: u16,
    pub outcome: String,
    pub interaction_kind: Option<String>,
    pub interaction_id_text: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementResolutionCursor {
    pub session_id: String,
    pub turn_number: u32,
    pub command_id: String,
    pub next_step_index: u16,
    pub gameplay_rows_written: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleStartDraft {
    pub battle_key: String,
    pub battle_type: String,
    pub attacker_champion_id: String,
    pub defender_kind: String,
    pub defender_id_text: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectStopDraft {
    pub champion_id: String,
    pub object_id: String,
    pub interaction_key: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementSystemCommandRecord {
    pub command_id: String,
    pub session_id: String,
    pub turn_number: u32,
    pub idempotency_key: String,
    pub status: String,
    pub created_at_ms: u64,
    pub applied_at_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementState {
    pub session_id: String,
    pub session_seed: String,
    pub current_turn: u32,
    pub turn_started_at_ms: u64,
    pub turn_duration_ms: u64,
    pub session_status: String,
    pub intents: Vec<MovementIntentRecord>,
    pub snapshots: Vec<MovementSnapshotRecord>,
    pub system_commands: Vec<MovementSystemCommandRecord>,
    pub partial_cursor: Option<MovementResolutionCursor>,
    pub recovery_checks: u32,
}

impl MovementState {
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        session_seed: impl Into<String>,
        current_turn: u32,
        turn_started_at_ms: u64,
        turn_duration_ms: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            session_seed: session_seed.into(),
            current_turn,
            turn_started_at_ms,
            turn_duration_ms,
            session_status: "active".to_string(),
            intents: Vec::new(),
            snapshots: Vec::new(),
            system_commands: Vec::new(),
            partial_cursor: None,
            recovery_checks: 0,
        }
    }

    #[must_use]
    pub fn turn_deadline_ms(&self) -> u64 {
        self.turn_started_at_ms
            .saturating_add(self.turn_duration_ms)
    }

    #[must_use]
    pub fn accepts_intents_at(&self, now_ms: u64) -> bool {
        self.session_status == "active"
            && now_ms >= self.turn_started_at_ms
            && now_ms < self.turn_deadline_ms()
            && self.partial_cursor.is_none()
    }

    #[must_use]
    pub fn time_view(&self, now_ms: u64) -> MovementTimeView {
        MovementTimeView {
            session_id: self.session_id.clone(),
            current_turn: self.current_turn,
            turn_started_at_ms: self.turn_started_at_ms,
            turn_deadline_ms: self.turn_deadline_ms(),
            now_ms,
            turn_expired: now_ms >= self.turn_deadline_ms(),
            sync_required: self.session_status == "active" && now_ms >= self.turn_deadline_ms(),
            partial_cursor: self.partial_cursor.clone(),
        }
    }

    #[must_use]
    pub fn pending_intent_count_for_turn(&self, turn_number: u32) -> usize {
        self.intents
            .iter()
            .filter(|intent| intent.turn_number == turn_number && intent.status == "pending")
            .count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementTimeView {
    pub session_id: String,
    pub current_turn: u32,
    pub turn_started_at_ms: u64,
    pub turn_deadline_ms: u64,
    pub now_ms: u64,
    pub turn_expired: bool,
    pub sync_required: bool,
    pub partial_cursor: Option<MovementResolutionCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementSyncBudget {
    pub max_commands_inspected: u32,
    pub max_commands_advanced: u32,
    pub max_effects: u32,
    pub max_events: u32,
    pub max_gameplay_rows: u32,
    pub max_microsteps: u32,
}

impl Default for MovementSyncBudget {
    fn default() -> Self {
        Self {
            max_commands_inspected: DEFAULT_RECOVERY_COMMANDS_INSPECTED,
            max_commands_advanced: DEFAULT_RECOVERY_COMMANDS_ADVANCED,
            max_effects: DEFAULT_RECOVERY_EFFECTS,
            max_events: DEFAULT_RECOVERY_EVENTS,
            max_gameplay_rows: DEFAULT_GAMEPLAY_ROWS,
            max_microsteps: MAX_MOVEMENT_MICROSTEPS_PER_SYNC,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementSyncOutcome {
    pub session_id: String,
    pub command_id: String,
    pub from_turn: u32,
    pub current_turn: u32,
    pub advanced_turn: bool,
    pub resolved_intent_ids: Vec<String>,
    pub superseded_intent_ids: Vec<String>,
    pub snapshots: Vec<MovementSnapshotRecord>,
    pub battle_starts: Vec<BattleStartDraft>,
    pub object_stops: Vec<ObjectStopDraft>,
    pub budget_exhausted: bool,
    pub recovery_checked: bool,
    pub recovered_commands_inspected: u32,
    pub recovered_commands_advanced: u32,
    pub gameplay_rows_written: u32,
    pub cursor: Option<MovementResolutionCursor>,
}

impl MovementSyncOutcome {
    #[must_use]
    pub fn idle(state: &MovementState, now_ms: u64) -> Self {
        let view = state.time_view(now_ms);
        Self {
            session_id: state.session_id.clone(),
            command_id: String::new(),
            from_turn: state.current_turn,
            current_turn: state.current_turn,
            advanced_turn: false,
            resolved_intent_ids: Vec::new(),
            superseded_intent_ids: Vec::new(),
            snapshots: Vec::new(),
            battle_starts: Vec::new(),
            object_stops: Vec::new(),
            budget_exhausted: false,
            recovery_checked: false,
            recovered_commands_inspected: 0,
            recovered_commands_advanced: 0,
            gameplay_rows_written: 0,
            cursor: view.partial_cursor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementSmokeView {
    pub session_id: String,
    pub final_turn: u32,
    pub champion_id: String,
    pub final_x: u16,
    pub final_y: u16,
    pub snapshots: u32,
    pub system_commands: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MovementError {
    #[error("session is not active")]
    SessionNotActive,
    #[error("turn {turn_number} is not accepting movement intents at {now_ms}")]
    SubmissionWindowClosed { turn_number: u32, now_ms: u64 },
    #[error("champion not found: {champion_id}")]
    ChampionNotFound { champion_id: String },
    #[error("participant {participant_id} does not own champion {champion_id}")]
    ChampionNotOwned {
        participant_id: String,
        champion_id: String,
    },
    #[error("champion {champion_id} is not active: {status}")]
    ChampionNotActive { champion_id: String, status: String },
    #[error("move path is empty")]
    PathEmpty,
    #[error("move path has {path_len} steps, exceeding {max_len}")]
    PathTooLong { path_len: usize, max_len: usize },
    #[error("path step {step_index} from ({from_x},{from_y}) to ({to_x},{to_y}) is not adjacent")]
    PathStepNotAdjacent {
        step_index: usize,
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
    },
    #[error("tile ({x},{y}) is outside map bounds")]
    OutOfBounds { x: u16, y: u16 },
    #[error("tile ({x},{y}) is impassable")]
    ImpassableTerrain { x: u16, y: u16 },
    #[error("tile ({x},{y}) is hidden for participant {participant_id}")]
    HiddenTile {
        participant_id: String,
        x: u16,
        y: u16,
    },
    #[error("movement cost {cost} exceeds available movement {available}")]
    MovementTooExpensive { cost: u16, available: u16 },
    #[error("path touches {chunk_count} chunks, exceeding {max_chunks}")]
    TooManyChunks {
        chunk_count: usize,
        max_chunks: usize,
    },
    #[error(
        "duplicate nonce {client_nonce} reused with different payload hash for champion {champion_id}"
    )]
    DuplicateNoncePayloadMismatch {
        champion_id: String,
        client_nonce: u64,
    },
    #[error("unresolved movement intent limit exceeded for turn {turn_number}: {max_intents}")]
    UnresolvedIntentLimitExceeded { turn_number: u32, max_intents: u32 },
    #[error("movement sync budget is exhausted before another microstep can be applied")]
    BudgetExhausted,
    #[error("simulated movement trap after command {command_id} at next step {next_step_index}")]
    SimulatedTrapAfterPartialApply {
        command_id: String,
        next_step_index: u16,
    },
    #[error("champion state error: {source}")]
    Champion { source: ChampionError },
    #[error("map state error: {source}")]
    Map { source: MapError },
}

impl From<ChampionError> for MovementError {
    fn from(source: ChampionError) -> Self {
        match source {
            ChampionError::ChampionNotFound { champion_id } => {
                Self::ChampionNotFound { champion_id }
            }
            other => Self::Champion { source: other },
        }
    }
}

impl From<MapError> for MovementError {
    fn from(source: MapError) -> Self {
        Self::Map { source }
    }
}

#[must_use]
pub fn build_first_playable_movement_state() -> MovementState {
    let fixture = first_playable_fixture();
    MovementState::new(
        fixture.ids.session_id,
        fixture.scenario_seed,
        1,
        0,
        TURN_DURATION_MS,
    )
}

pub(crate) fn movement_intent_command_id(
    session_id: &str,
    champion_id: &str,
    turn_number: u32,
    client_nonce: u64,
) -> String {
    format!("command:move-intent:{session_id}:{champion_id}:{turn_number}:{client_nonce}")
}

pub(crate) fn movement_intent_id(
    session_id: &str,
    champion_id: &str,
    turn_number: u32,
    client_nonce: u64,
) -> String {
    format!("move-intent:{session_id}:{champion_id}:{turn_number}:{client_nonce}")
}

pub(crate) fn movement_resolution_command_id(session_id: &str, turn_number: u32) -> String {
    format!("system:movement-resolution:{session_id}:turn:{turn_number}")
}

pub(crate) fn movement_payload_hash(
    session_id: &str,
    participant_id: &str,
    champion_id: &str,
    turn_number: u32,
    client_nonce: u64,
    path: &[MoveCoord],
) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "domain", "domm.move_intent.v1");
    hash_text(&mut hasher, "session_id", session_id);
    hash_text(&mut hasher, "participant_id", participant_id);
    hash_text(&mut hasher, "champion_id", champion_id);
    hash_u32(&mut hasher, "turn_number", turn_number);
    hash_u64(&mut hasher, "client_nonce", client_nonce);
    hash_u32(&mut hasher, "path_len", path.len() as u32);
    for coord in path {
        hash_u16(&mut hasher, "x", coord.x);
        hash_u16(&mut hasher, "y", coord.y);
    }
    hex_digest(hasher.finalize().as_slice())
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update((label.len() as u32).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update((value.len() as u32).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_u16(hasher: &mut Sha256, label: &str, value: u16) {
    hasher.update((label.len() as u32).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(value.to_le_bytes());
}

fn hash_u32(hasher: &mut Sha256, label: &str, value: u32) {
    hasher.update((label.len() as u32).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(value.to_le_bytes());
}

fn hash_u64(hasher: &mut Sha256, label: &str, value: u64) {
    hasher.update((label.len() as u32).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update(value.to_le_bytes());
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
