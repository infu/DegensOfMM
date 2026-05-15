use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::playable::PlayableBattleView;
use crate::strategic::StrategicGameView;

pub const AI_MAX_ACTORS_PER_UPDATE: u16 = 2;
pub const AI_MAX_CANDIDATES_PER_ACTOR: u16 = 16;
pub const AI_MAX_EMITTED_COMMANDS_PER_UPDATE: u16 = 2;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AiActorStateRecord {
    pub session_id: String,
    pub actor_id_text: String,
    pub actor_kind: String,
    pub participant_id: String,
    pub profile_key: String,
    pub cursor_json: Option<String>,
    pub last_turn_processed: u32,
    pub last_command_nonce: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AiDecisionInput {
    pub session_id: String,
    pub session_seed: String,
    pub turn_number: u32,
    pub actor: AiActorStateRecord,
    pub strategic_view: Option<StrategicGameView>,
    pub battle_view: Option<PlayableBattleView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AiCommandDraft {
    pub actor_kind: String,
    pub actor_id_text: String,
    pub command_kind: String,
    pub target_id_text: Option<String>,
    pub payload_summary: String,
    pub client_nonce: String,
    pub priority_score: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AiUpdateReport {
    pub actor_count: u16,
    pub actors_processed: u16,
    pub candidates_considered: u16,
    pub emitted_commands: Vec<AiCommandDraft>,
    pub budget_exhausted: bool,
    pub cursor_json: Option<String>,
    pub no_available_reason: Option<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AiError {
    #[error("unsupported AI actor kind: {actor_kind}")]
    UnsupportedActorKind { actor_kind: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiCandidate {
    pub command_kind: String,
    pub target_id_text: Option<String>,
    pub payload_summary: String,
    pub priority_score: i32,
    pub candidate_key: String,
}
