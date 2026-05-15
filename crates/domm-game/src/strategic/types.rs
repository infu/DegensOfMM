use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::champion::ChampionError;
use crate::driver::DriverError;
use crate::economy::{EconomyError, ResourceBalances};
use crate::lifecycle::LifecycleError;
use crate::map::MapError;
use crate::movement::{MoveCoord, MovementError};
use crate::neutral::NeutralError;
use crate::town::TownError;
use crate::world_object::WorldObjectError;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StrategicGameView {
    pub session_id: String,
    pub participant_id: String,
    pub current_turn: u32,
    pub sync_required: bool,
    pub champion_id: String,
    pub champion_status: String,
    pub champion_x: u16,
    pub champion_y: u16,
    pub resources: ResourceBalances,
    pub built_buildings: Vec<String>,
    pub recruit_pool_available: u32,
    pub town_garrison_quantity: u32,
    pub visible_chunk_count: u32,
    pub visible_object_count: u32,
    pub object_command_count: u32,
    pub movement_snapshot_count: u32,
    pub neutral_encounter_count: u32,
    pub pending_battle_key: Option<String>,
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub approximate_query_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StrategicCommandReceipt {
    pub command_kind: String,
    pub command_id: String,
    pub current_turn: u32,
    pub command_count: u32,
    pub event_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StrategicStepView {
    pub step_key: String,
    pub view: StrategicGameView,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StrategicGateReport {
    pub session_id: String,
    pub step_views: Vec<StrategicStepView>,
    pub final_view: StrategicGameView,
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub max_query_bytes: u32,
    pub concerns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StrategicCall {
    RegisterPlayer {
        caller: Principal,
    },
    CreateSession {
        caller: Principal,
    },
    JoinSession {
        caller: Principal,
    },
    MarkReady {
        caller: Principal,
    },
    StartSession {
        caller: Principal,
    },
    InspectView {
        caller: Principal,
    },
    SubmitMoveIntent {
        caller: Principal,
        champion_id: String,
        path: Vec<MoveCoord>,
    },
    SyncTurn {
        caller: Principal,
        now_ms: u64,
    },
    ApplyMovementObjects {
        caller: Principal,
    },
    MaterializeIncome {
        caller: Principal,
        turn_number: u32,
    },
    BuildTownStructure {
        caller: Principal,
        town_id: String,
        building_slug: String,
    },
    RecruitUnits {
        caller: Principal,
        town_id: String,
        unit_slug: String,
        quantity: u32,
    },
    ApplyNeutralEncounters {
        caller: Principal,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum StrategicError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    #[error(transparent)]
    Movement(#[from] MovementError),
    #[error(transparent)]
    WorldObject(#[from] WorldObjectError),
    #[error(transparent)]
    Economy(#[from] EconomyError),
    #[error(transparent)]
    Town(#[from] TownError),
    #[error(transparent)]
    Neutral(#[from] NeutralError),
    #[error(transparent)]
    Champion(#[from] ChampionError),
    #[error(transparent)]
    Map(#[from] MapError),
    #[error("movement sync has not produced an outcome")]
    MissingMovementSync,
    #[error("unknown strategic caller")]
    UnknownCaller,
}
