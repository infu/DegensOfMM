use candid::CandidType;
use serde::{Deserialize, Serialize};

use crate::economy::ResourceBalances;

pub const OPENING_QUEST_KEY: &str = "quest:opening-ledger";
pub const OPENING_QUEST_TITLE: &str = "Opening Ledger";
pub const OPENING_QUEST_OBJECTIVE_KEY: &str = "objective:opening-ledger";
pub const OPENING_QUEST_REWARD_GOLD: u32 = 500;
pub const CENTRAL_OBJECTIVE_NORTH_KEY: &str = "objective:north";
pub const CENTRAL_OBJECTIVE_SOUTH_KEY: &str = "objective:south";

pub const MAX_OBJECTIVE_ROWS_PER_SESSION: u32 = 16;
pub const MAX_ACTIVE_QUESTS_PER_PARTICIPANT: u32 = 4;
pub const MAX_WORLD_EVENT_ROWS_PER_SESSION: u32 = 16;
pub const MAX_SCENARIO_RULE_ROWS_PER_SESSION: u32 = 16;
pub const MAX_ADVANCED_VICTORY_CHECKS_PER_UPDATE: u32 = 8;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectiveProgressRecord {
    pub objective_key: String,
    pub objective_type: String,
    pub owner_participant_id: Option<String>,
    pub object_id: Option<String>,
    pub progress_value: u32,
    pub required_value: u32,
    pub status: String,
    pub visible_to: String,
    pub last_scored_turn: u32,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectiveProgressView {
    pub session_id: String,
    pub objectives: Vec<ObjectiveProgressRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct QuestProgressView {
    pub quest_key: String,
    pub title: String,
    pub participant_id: String,
    pub objective_key: String,
    pub status: String,
    pub progress_value: u32,
    pub required_value: u32,
    pub reward_gold: Option<u32>,
    pub reward_claimed: bool,
    pub accepted_turn: u32,
    pub claimed_turn: u32,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct QuestPreview {
    pub can_accept: bool,
    pub can_claim: bool,
    pub disabled_reason: Option<String>,
    pub quest: QuestProgressView,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldEventView {
    pub event_key: String,
    pub event_type: String,
    pub event_window: String,
    pub starts_turn: u32,
    pub ends_turn: u32,
    pub status: String,
    pub payload: Option<String>,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldEventsView {
    pub session_id: String,
    pub current_turn: u32,
    pub events: Vec<WorldEventView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ScenarioRuleView {
    pub rule_key: String,
    pub rule_type: String,
    pub status: String,
    pub victory_state: String,
    pub required_value: u32,
    pub current_value: u32,
    pub owner_participant_id: Option<String>,
    pub winner_participant_id: Option<String>,
    pub disabled_reason: Option<String>,
    pub last_checked_turn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ScenarioRulesView {
    pub session_id: String,
    pub current_turn: u32,
    pub rules: Vec<ScenarioRuleView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct AdvancedScenarioReceipt {
    pub command_id: String,
    pub action: String,
    pub quest_key: Option<String>,
    pub objective_key: Option<String>,
    pub event_key: Option<String>,
    pub rule_key: Option<String>,
    pub current_turn: u32,
    pub reward_gold: u32,
    pub state: String,
    pub resources_after: Option<ResourceBalances>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestMutation {
    pub allowed: bool,
    pub next_status: String,
    pub reward_gold_delta: u32,
    pub disabled_reason: Option<String>,
}
