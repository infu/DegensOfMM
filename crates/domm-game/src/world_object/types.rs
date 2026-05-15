use std::fmt::Write as _;

use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::champion::ChampionError;
use crate::economy::{EconomyError, ResourceBalances};
use crate::fixtures::first_playable_fixture;
use crate::map::MapError;
use crate::movement::MovementError;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ParticipantObjectVisitRecord {
    pub visit_id: String,
    pub session_id: String,
    pub object_id: String,
    pub participant_id: String,
    pub visit_key: String,
    pub visit_kind: String,
    pub visited_turn: u32,
    pub command_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionObjectVisitRecord {
    pub visit_id: String,
    pub session_id: String,
    pub object_id: String,
    pub champion_id: String,
    pub visit_key: String,
    pub visit_kind: String,
    pub visited_turn: u32,
    pub command_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectInteractionCommandRecord {
    pub command_id: String,
    pub session_id: String,
    pub participant_id: String,
    pub champion_id: String,
    pub object_id: String,
    pub client_nonce: u64,
    pub payload_hash: String,
    pub status: String,
    pub created_at_ms: u64,
    pub applied_at_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectCommandEffectRecord {
    pub effect_id: String,
    pub command_id: String,
    pub effect_key: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldObjectState {
    pub session_id: String,
    pub participant_visits: Vec<ParticipantObjectVisitRecord>,
    pub champion_visits: Vec<ChampionObjectVisitRecord>,
    pub commands: Vec<ObjectInteractionCommandRecord>,
    pub effects: Vec<ObjectCommandEffectRecord>,
}

impl WorldObjectState {
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            participant_visits: Vec::new(),
            champion_visits: Vec::new(),
            commands: Vec::new(),
            effects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectInteractionOutcome {
    pub command_id: String,
    pub object_id: String,
    pub interaction_kind: String,
    pub visit_key: String,
    pub duplicate_replay: bool,
    pub participant_visit: Option<ParticipantObjectVisitRecord>,
    pub champion_visit: Option<ChampionObjectVisitRecord>,
    pub resource_outcome: Option<ObjectResourceOutcome>,
    pub captured_source_id: Option<String>,
    pub scores: Vec<ObjectScoreRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectResourceOutcome {
    pub ledger_rows_touched: u32,
    pub balance_updates: u32,
    pub skipped_applied_rows: u32,
    pub budget_exhausted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectScoreRecord {
    pub scoring_kind: String,
    pub owner_participant_id: Option<String>,
    pub object_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldObjectSmokeView {
    pub participant_id: String,
    pub after_pickup: ResourceBalances,
    pub captured_mine_id: String,
    pub mine_income_started_turn: u32,
    pub captured_objective_id: String,
    pub central_objectives_owned: u32,
    pub object_commands: u32,
    pub participant_visits: u32,
    pub champion_visits: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum WorldObjectError {
    #[error("object not found: {object_id}")]
    ObjectNotFound { object_id: String },
    #[error("champion not found: {champion_id}")]
    ChampionNotFound { champion_id: String },
    #[error("participant {participant_id} does not own champion {champion_id}")]
    ChampionNotOwned {
        participant_id: String,
        champion_id: String,
    },
    #[error("champion {champion_id} is not on object {object_id}")]
    ChampionNotOnObject {
        champion_id: String,
        object_id: String,
    },
    #[error("object {object_id} is not visible to participant {participant_id}")]
    ObjectNotVisible {
        participant_id: String,
        object_id: String,
    },
    #[error("object {object_id} is guarded by {guard_id}")]
    ObjectGuarded { object_id: String, guard_id: String },
    #[error("object {object_id} was already visited for key {visit_key}")]
    ObjectAlreadyVisited {
        object_id: String,
        visit_key: String,
    },
    #[error("object {object_id} cannot be interacted with as {scoring_kind}")]
    UnsupportedInteraction {
        object_id: String,
        scoring_kind: String,
    },
    #[error("object command nonce {client_nonce} was reused with a different payload")]
    DuplicateNoncePayloadMismatch { client_nonce: u64 },
    #[error("economy error: {source}")]
    Economy { source: EconomyError },
    #[error("champion error: {source}")]
    Champion { source: ChampionError },
    #[error("map error: {source}")]
    Map { source: MapError },
    #[error("movement error: {source}")]
    Movement { source: MovementError },
}

impl From<EconomyError> for WorldObjectError {
    fn from(source: EconomyError) -> Self {
        Self::Economy { source }
    }
}

impl From<ChampionError> for WorldObjectError {
    fn from(source: ChampionError) -> Self {
        match source {
            ChampionError::ChampionNotFound { champion_id } => {
                Self::ChampionNotFound { champion_id }
            }
            other => Self::Champion { source: other },
        }
    }
}

impl From<MapError> for WorldObjectError {
    fn from(source: MapError) -> Self {
        Self::Map { source }
    }
}

impl From<MovementError> for WorldObjectError {
    fn from(source: MovementError) -> Self {
        Self::Movement { source }
    }
}

#[must_use]
pub fn build_first_playable_world_object_state() -> WorldObjectState {
    let fixture = first_playable_fixture();
    WorldObjectState::new(fixture.ids.session_id)
}

pub(crate) fn object_command_id(
    session_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    client_nonce: u64,
) -> String {
    format!("command:object:{session_id}:{champion_id}:{object_id}:{current_turn}:{client_nonce}")
}

pub(crate) fn movement_object_command_id(
    movement_command_id: &str,
    champion_id: &str,
    object_id: &str,
) -> String {
    format!("system:movement-object:{movement_command_id}:{champion_id}:{object_id}")
}

pub(crate) fn object_payload_hash(
    session_id: &str,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    client_nonce: u64,
) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "domain", "domm.object_interaction.v1");
    hash_text(&mut hasher, "session_id", session_id);
    hash_text(&mut hasher, "participant_id", participant_id);
    hash_text(&mut hasher, "champion_id", champion_id);
    hash_text(&mut hasher, "object_id", object_id);
    hash_u32(&mut hasher, "turn", current_turn);
    hash_u64(&mut hasher, "nonce", client_nonce);
    to_hex(&hasher.finalize())
}

pub(crate) fn visit_key_for(scoring_kind: &str, current_turn: u32) -> String {
    match scoring_kind {
        "resource_pile" => "once".to_string(),
        "mine" | "central_objective" | "objective" => format!("turn:{current_turn}"),
        other if other.starts_with("week") => {
            let week = current_turn.saturating_sub(1) / 7 + 1;
            format!("week:{week}")
        }
        _ => "once".to_string(),
    }
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update((label.len() as u32).to_le_bytes());
    hasher.update(label.as_bytes());
    hasher.update((value.len() as u32).to_le_bytes());
    hasher.update(value.as_bytes());
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

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
