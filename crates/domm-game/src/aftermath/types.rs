use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::battle::{BattleError, BattleState};
use crate::champion::{ChampionError, ChampionState};
use crate::economy::EconomyError;
use crate::lifecycle::MatchHistoryEntry;
use crate::map::{FirstPlayableMapState, MapError};
use crate::neutral::{NeutralError, NeutralState};
use crate::town::TownError;
use crate::{EconomyState, TownState};

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MatchSessionRecord {
    pub session_id: String,
    pub state: String,
    pub current_turn: u32,
    pub max_turns: u32,
    pub winner_participant_id: Option<String>,
    pub finish_reason: Option<String>,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayerMatchSummaryRecord {
    pub summary_id: String,
    pub player_id: String,
    pub session_id: String,
    pub result: String,
    pub opponent_name: Option<String>,
    pub turns_played: u32,
    pub summary_json: Option<String>,
    pub finished_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct VictoryScore {
    pub participant_id: String,
    pub town_count: u32,
    pub mine_count: u32,
    pub army_power_score: u64,
    pub tie_break_score: u64,
    pub total_score: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct VictoryCheck {
    pub finalized: bool,
    pub winner_participant_id: Option<String>,
    pub finish_reason: Option<String>,
    pub scores: Vec<VictoryScore>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RetreatSurrenderPolicy {
    pub retreat_allowed: bool,
    pub retreat_disabled_reason: Option<String>,
    pub surrender_allowed: bool,
    pub surrender_disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleAftermathReport {
    pub battle_id: String,
    pub battle_type: String,
    pub winner_participant_id: Option<String>,
    pub victor_champion_id: Option<String>,
    pub defeated_champion_id: Option<String>,
    pub defeated_neutral_army_id: Option<String>,
    pub captured_town_id: Option<String>,
    pub captured_artifacts: Vec<String>,
    pub victory: VictoryCheck,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AftermathEventRecord {
    pub event_id: String,
    pub sequence: u64,
    pub command_id: String,
    pub event_type: String,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AftermathState {
    pub session: MatchSessionRecord,
    pub battle: BattleState,
    pub champions: ChampionState,
    pub town: TownState,
    pub economy: EconomyState,
    pub map: FirstPlayableMapState,
    pub neutral: NeutralState,
    pub player_match_summaries: Vec<PlayerMatchSummaryRecord>,
    pub match_history: Vec<(String, MatchHistoryEntry)>,
    pub aftermath_reports: Vec<(String, BattleAftermathReport)>,
    pub aftermath_events: Vec<AftermathEventRecord>,
    pub applied_commands: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AftermathSmokeView {
    pub final_session_state: String,
    pub winner_participant_id: Option<String>,
    pub defeated_neutral_state: String,
    pub captured_town_owner: String,
    pub defeated_champion_status: String,
    pub match_summary_count: u32,
    pub match_history_count: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AftermathError {
    #[error(transparent)]
    Battle(#[from] BattleError),
    #[error(transparent)]
    Champion(#[from] ChampionError),
    #[error(transparent)]
    Town(#[from] TownError),
    #[error(transparent)]
    Economy(#[from] EconomyError),
    #[error(transparent)]
    Map(#[from] MapError),
    #[error(transparent)]
    Neutral(#[from] NeutralError),
    #[error("battle is not resolved: {battle_id}")]
    BattleNotResolved { battle_id: String },
    #[error("battle has no winner: {battle_id}")]
    MissingBattleWinner { battle_id: String },
    #[error("participant not found: {participant_id}")]
    ParticipantNotFound { participant_id: String },
    #[error("retreat and surrender are disabled for v1: {reason}")]
    RetreatSurrenderDisabled { reason: String },
}

impl BattleAftermathReport {
    #[must_use]
    pub fn empty(battle_id: &str, battle_type: &str) -> Self {
        Self {
            battle_id: battle_id.to_string(),
            battle_type: battle_type.to_string(),
            winner_participant_id: None,
            victor_champion_id: None,
            defeated_champion_id: None,
            defeated_neutral_army_id: None,
            captured_town_id: None,
            captured_artifacts: Vec::new(),
            victory: VictoryCheck {
                finalized: false,
                winner_participant_id: None,
                finish_reason: None,
                scores: Vec::new(),
            },
        }
    }
}
