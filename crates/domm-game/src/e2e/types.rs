use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::champion::ChampionError;
use crate::map::MapError;
use crate::movement::MovementError;
use crate::playable::{PlayableError, PlayableGateReport};

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EndToEndFirstPlayableReport {
    pub session_id: String,
    pub coverage: EndToEndCoverage,
    pub measurements: EndToEndMeasurements,
    pub movement_conflict: MovementConflictReport,
    pub backend_gate: PlayableGateReport,
    pub spec_audit: Vec<SpecAuditRow>,
    pub manual_smoke_commands: Vec<ManualSmokeCommand>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EndToEndCoverage {
    pub exploration: bool,
    pub pickup: bool,
    pub building: bool,
    pub recruitment: bool,
    pub movement_conflict: bool,
    pub battle: bool,
    pub town_capture: bool,
    pub victory: bool,
}

impl EndToEndCoverage {
    #[must_use]
    pub fn complete(&self) -> bool {
        self.exploration
            && self.pickup
            && self.building
            && self.recruitment
            && self.movement_conflict
            && self.battle
            && self.town_capture
            && self.victory
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EndToEndMeasurements {
    pub command_count: u32,
    pub event_count: u32,
    pub query_count: u32,
    pub storage_row_count: u32,
    pub max_query_bytes: u32,
    pub estimated_response_bytes: u32,
    pub recovery_retry_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MovementConflictReport {
    pub snapshot_count: u32,
    pub stopped_tile_conflict: bool,
    pub west_final_x: u16,
    pub west_final_y: u16,
    pub east_final_x: u16,
    pub east_final_y: u16,
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SpecAuditRow {
    pub area: String,
    pub status: SpecAuditStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum SpecAuditStatus {
    Implemented,
    Deferred,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ManualSmokeCommand {
    pub label: String,
    pub command: String,
    pub expected: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EndToEndError {
    #[error(transparent)]
    Playable(#[from] PlayableError),
    #[error(transparent)]
    Movement(#[from] MovementError),
    #[error(transparent)]
    Champion(#[from] ChampionError),
    #[error(transparent)]
    Map(#[from] MapError),
}
