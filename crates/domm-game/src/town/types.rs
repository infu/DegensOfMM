use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::economy::{EconomyError, ResourceBalances};

pub const MAX_ARMY_SLOTS: u8 = 7;
pub const RECRUIT_POOL_CAP: u32 = 9_999;
pub const ARMY_STACK_CAP: u32 = 99_999;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TownRecord {
    pub town_id: String,
    pub session_id: String,
    pub owner_participant_id: String,
    pub faction_slug: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub status: String,
    pub hall_level: u8,
    pub fort_level: u8,
    pub last_built_turn: u32,
    pub captured_turn: u32,
    pub income_started_turn: u32,
    pub unrest_until_turn: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TownBuildingRecord {
    pub building_id: String,
    pub session_id: String,
    pub town_id: String,
    pub building_slug: String,
    pub built_turn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TownRecruitPoolRecord {
    pub pool_id: String,
    pub session_id: String,
    pub town_id: String,
    pub unit_slug: String,
    pub available: u32,
    pub last_growth_week: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArmyStackRecord {
    pub stack_id: String,
    pub session_id: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub unit_slug: String,
    pub slot_index: u8,
    pub quantity: u32,
    pub front_hp: u16,
    pub status: String,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionTownRecord {
    pub champion_id: String,
    pub session_id: String,
    pub participant_id: String,
    pub status: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum RecruitTarget {
    TownGarrison {
        slot_index: Option<u8>,
    },
    Champion {
        champion_id: String,
        slot_index: Option<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BuildPreview {
    pub allowed: bool,
    pub disabled_reason: Option<String>,
    pub town_id: String,
    pub building_slug: String,
    pub cost: ResourceBalances,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RecruitPreview {
    pub allowed: bool,
    pub disabled_reason: Option<String>,
    pub town_id: String,
    pub unit_slug: String,
    pub quantity: u32,
    pub target_slot_index: Option<u8>,
    pub total_cost: ResourceBalances,
    pub available: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TownState {
    pub session_id: String,
    pub current_turn: u32,
    pub towns: Vec<TownRecord>,
    pub buildings: Vec<TownBuildingRecord>,
    pub recruit_pools: Vec<TownRecruitPoolRecord>,
    pub garrison_stacks: Vec<ArmyStackRecord>,
    pub champion_stacks: Vec<ArmyStackRecord>,
    pub champions: Vec<ChampionTownRecord>,
    pub applied_commands: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TownSmokeView {
    pub town_id: String,
    pub built_building_slug: String,
    pub recruited_unit_slug: String,
    pub recruited_quantity: u32,
    pub final_resources: ResourceBalances,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TownError {
    #[error(transparent)]
    Economy(#[from] EconomyError),
    #[error("town not found: {town_id}")]
    TownNotFound { town_id: String },
    #[error("building not found: {building_slug}")]
    BuildingNotFound { building_slug: String },
    #[error("unit not found: {unit_slug}")]
    UnitNotFound { unit_slug: String },
    #[error("champion not found: {champion_id}")]
    ChampionNotFound { champion_id: String },
    #[error("action is disabled: {reason}")]
    Disabled { reason: String },
    #[error("recruit target is full")]
    RecruitTargetFull,
    #[error("unit stack is incompatible")]
    UnitStackIncompatible,
    #[error("interrupted after resource spend")]
    InterruptedAfterSpend,
}

impl BuildPreview {
    pub(crate) fn disabled(
        town_id: &str,
        building_slug: &str,
        cost: ResourceBalances,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            allowed: false,
            disabled_reason: Some(reason.into()),
            town_id: town_id.to_string(),
            building_slug: building_slug.to_string(),
            cost,
        }
    }
}

impl RecruitPreview {
    pub(crate) fn disabled(
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        total_cost: ResourceBalances,
        available: u32,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            allowed: false,
            disabled_reason: Some(reason.into()),
            town_id: town_id.to_string(),
            unit_slug: unit_slug.to_string(),
            quantity,
            target_slot_index: None,
            total_cost,
            available,
        }
    }
}
