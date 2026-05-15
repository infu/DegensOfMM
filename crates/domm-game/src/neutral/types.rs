use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::champion::ChampionError;
use crate::map::MapError;
use crate::movement::MovementError;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralArmyRecord {
    pub neutral_army_id: String,
    pub session_id: String,
    pub scenario_strength_band: String,
    pub x: u16,
    pub y: u16,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub state: String,
    pub aggression: String,
    pub growth_rule_key: String,
    pub last_growth_week: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralArmyStackRecord {
    pub stack_id: String,
    pub session_id: String,
    pub neutral_army_id: String,
    pub unit_slug: String,
    pub slot_index: u8,
    pub quantity: u32,
    pub front_hp: u16,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralArmyEncounterRecord {
    pub encounter_id: String,
    pub session_id: String,
    pub command_id: String,
    pub battle_key: String,
    pub neutral_army_id: String,
    pub attacker_champion_id: String,
    pub source_kind: String,
    pub source_id_text: String,
    pub turn_number: u32,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralState {
    pub session_id: String,
    pub armies: Vec<NeutralArmyRecord>,
    pub stacks: Vec<NeutralArmyStackRecord>,
    pub encounters: Vec<NeutralArmyEncounterRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralArmyView {
    pub neutral_army_id: String,
    pub visibility: String,
    pub x: u16,
    pub y: u16,
    pub state: String,
    pub aggression: String,
    pub strength_label: String,
    pub exact_stacks: Vec<NeutralArmyStackRecord>,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum NeutralArmyViewResult {
    Visible(NeutralArmyView),
    LastKnown(NeutralArmyView),
    Hidden {
        neutral_army_id: String,
        visibility: String,
    },
    NotFound {
        neutral_army_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralBehaviorPolicy {
    pub aggression: String,
    pub roaming_enabled: bool,
    pub join_enabled: bool,
    pub bribe_enabled: bool,
    pub disabled_reasons: Vec<String>,
}

impl Default for NeutralBehaviorPolicy {
    fn default() -> Self {
        Self {
            aggression: "guard".to_string(),
            roaming_enabled: false,
            join_enabled: false,
            bribe_enabled: false,
            disabled_reasons: vec![
                "neutral_roaming_deferred_v1".to_string(),
                "neutral_join_deferred_v1".to_string(),
                "neutral_bribe_deferred_v1".to_string(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralGrowthOutcome {
    pub current_week: u32,
    pub armies_checked: u32,
    pub stacks_changed: u32,
    pub materialized: bool,
    pub disabled_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralSmokeView {
    pub neutral_army_id: String,
    pub strength_label: String,
    pub encounter_id: String,
    pub battle_key: String,
    pub defeated_state: String,
    pub occupancy_rows_after_defeat: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum NeutralError {
    #[error("neutral army not found: {neutral_army_id}")]
    NeutralArmyNotFound { neutral_army_id: String },
    #[error("champion not found: {champion_id}")]
    ChampionNotFound { champion_id: String },
    #[error("guarded object not found or unguarded: {object_id}")]
    GuardedObjectNotFound { object_id: String },
    #[error("neutral army {neutral_army_id} is not active: {state}")]
    NeutralArmyNotActive {
        neutral_army_id: String,
        state: String,
    },
    #[error("champion state error: {source}")]
    Champion { source: ChampionError },
    #[error("map state error: {source}")]
    Map { source: MapError },
    #[error("movement error: {source}")]
    Movement { source: MovementError },
}

impl From<ChampionError> for NeutralError {
    fn from(source: ChampionError) -> Self {
        match source {
            ChampionError::ChampionNotFound { champion_id } => {
                Self::ChampionNotFound { champion_id }
            }
            other => Self::Champion { source: other },
        }
    }
}

impl From<MapError> for NeutralError {
    fn from(source: MapError) -> Self {
        Self::Map { source }
    }
}

impl From<MovementError> for NeutralError {
    fn from(source: MovementError) -> Self {
        Self::Movement { source }
    }
}

impl NeutralState {
    pub(crate) fn army(&self, neutral_army_id: &str) -> Result<&NeutralArmyRecord, NeutralError> {
        self.armies
            .iter()
            .find(|army| army.neutral_army_id == neutral_army_id)
            .ok_or_else(|| NeutralError::NeutralArmyNotFound {
                neutral_army_id: neutral_army_id.to_string(),
            })
    }

    pub(crate) fn army_mut(
        &mut self,
        neutral_army_id: &str,
    ) -> Result<&mut NeutralArmyRecord, NeutralError> {
        self.armies
            .iter_mut()
            .find(|army| army.neutral_army_id == neutral_army_id)
            .ok_or_else(|| NeutralError::NeutralArmyNotFound {
                neutral_army_id: neutral_army_id.to_string(),
            })
    }

    #[must_use]
    pub fn stacks_for(&self, neutral_army_id: &str) -> Vec<NeutralArmyStackRecord> {
        let mut stacks = self
            .stacks
            .iter()
            .filter(|stack| stack.neutral_army_id == neutral_army_id)
            .cloned()
            .collect::<Vec<_>>();
        stacks.sort_by_key(|stack| stack.slot_index);
        stacks
    }

    #[must_use]
    pub fn quantity_for(&self, neutral_army_id: &str) -> u32 {
        self.stacks
            .iter()
            .filter(|stack| stack.neutral_army_id == neutral_army_id)
            .map(|stack| stack.quantity)
            .sum()
    }
}

#[must_use]
pub fn strength_label_for_quantity(quantity: u32) -> &'static str {
    match quantity {
        0 => "None",
        1..=9 => "Few",
        10..=24 => "Pack",
        25..=49 => "Group",
        50..=99 => "Company",
        100..=249 => "Host",
        _ => "Legion",
    }
}
