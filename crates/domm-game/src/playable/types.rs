use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::aftermath::AftermathError;
use crate::battle::{BattleCoord, BattleError};
use crate::driver::DriverError;
use crate::strategic::{StrategicCall, StrategicError, StrategicGateReport};

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayableBattleView {
    pub battle_id: String,
    pub battle_state: String,
    pub active_stack_id: Option<String>,
    pub legal_action_count: u32,
    pub event_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayableCommandReceipt {
    pub command_kind: String,
    pub command_id: String,
    pub battle_id: Option<String>,
    pub current_round: u16,
    pub active_stack_id: Option<String>,
    pub replayed: bool,
    pub event_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayableEventPage {
    pub cursor: u32,
    pub next_cursor: Option<u32>,
    pub events_returned: u32,
    pub total_event_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayableMatchView {
    pub session_id: String,
    pub current_turn: u32,
    pub final_session_state: String,
    pub winner_participant_id: Option<String>,
    pub champion_status: String,
    pub captured_town_owner: String,
    pub defeated_neutral_state: String,
    pub defeated_champion_status: String,
    pub match_summary_count: u32,
    pub match_history_count: u32,
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub max_query_bytes: u32,
    pub storage_row_count: u32,
    pub recovery_retry_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayableGateReport {
    pub session_id: String,
    pub strategic: StrategicGateReport,
    pub final_view: PlayableMatchView,
    pub event_page: PlayableEventPage,
    pub recovery_retry_count: u32,
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub storage_row_count: u32,
    pub max_query_bytes: u32,
    pub concerns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayableCall {
    Strategic(StrategicCall),
    PrepareBattle {
        caller: Principal,
    },
    InspectBattle {
        caller: Principal,
        battle_id: String,
    },
    SubmitBattleAction {
        caller: Principal,
        battle_id: String,
        battle_stack_id: String,
        action: String,
    },
    SyncBattle {
        caller: Principal,
        battle_id: String,
    },
    ResolveNeutralBattle {
        caller: Principal,
        battle_id: String,
    },
    ApplyBattleAftermath {
        caller: Principal,
        battle_id: String,
    },
    ResolveTownCapture {
        caller: Principal,
    },
    ResolveChampionDefeat {
        caller: Principal,
    },
    RefreshEvents {
        caller: Principal,
    },
    InspectMatch {
        caller: Principal,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlayableError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Strategic(#[from] StrategicError),
    #[error(transparent)]
    Battle(#[from] BattleError),
    #[error(transparent)]
    Aftermath(#[from] AftermathError),
    #[error("battle state has not been prepared")]
    MissingAftermathState,
    #[error("unknown playable caller")]
    UnknownCaller,
    #[error("public retry did not replay the original command")]
    RetryDidNotReplay,
}

#[allow(dead_code)]
fn _candid_markers(_coord: BattleCoord) {}
