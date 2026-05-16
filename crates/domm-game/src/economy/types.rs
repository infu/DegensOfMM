use std::fmt::Write as _;

use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::content::{ResourceCost, ResourcePileSeed, first_playable_scenario};
use crate::fixtures::{FixtureIds, first_playable_fixture};

pub const RESOURCE_GOLD: &str = "gold";
pub const RESOURCE_WOOD: &str = "wood";
pub const RESOURCE_STONE: &str = "stone";
pub const RESOURCE_IRON: &str = "iron";
pub const RESOURCE_CRYSTAL: &str = "crystal";
pub const RESOURCE_EMBER: &str = "ember";
pub const RESOURCE_AETHER: &str = "aether";
pub const RESOURCE_KEYS: [&str; 7] = [
    RESOURCE_GOLD,
    RESOURCE_WOOD,
    RESOURCE_STONE,
    RESOURCE_IRON,
    RESOURCE_CRYSTAL,
    RESOURCE_EMBER,
    RESOURCE_AETHER,
];
pub const GOLD_CAP: u64 = 1_000_000;
pub const STANDARD_RESOURCE_CAP: u64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourceBalances {
    pub gold: u64,
    pub wood: u32,
    pub stone: u32,
    pub iron: u32,
    pub crystal: u32,
    pub ember: u32,
    pub aether: u32,
}

impl ResourceBalances {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            gold: 0,
            wood: 0,
            stone: 0,
            iron: 0,
            crystal: 0,
            ember: 0,
            aether: 0,
        }
    }

    #[must_use]
    pub fn starting() -> Self {
        Self::from_cost(&ResourceCost::starting_resources())
    }

    #[must_use]
    pub fn from_cost(cost: &ResourceCost) -> Self {
        Self {
            gold: u64::from(cost.gold),
            wood: cost.wood,
            stone: cost.stone,
            iron: cost.iron,
            crystal: cost.crystal,
            ember: cost.ember,
            aether: cost.aether,
        }
    }

    pub fn get(&self, resource_key: &str) -> Result<u64, EconomyError> {
        match resource_key {
            RESOURCE_GOLD => Ok(self.gold),
            RESOURCE_WOOD => Ok(u64::from(self.wood)),
            RESOURCE_STONE => Ok(u64::from(self.stone)),
            RESOURCE_IRON => Ok(u64::from(self.iron)),
            RESOURCE_CRYSTAL => Ok(u64::from(self.crystal)),
            RESOURCE_EMBER => Ok(u64::from(self.ember)),
            RESOURCE_AETHER => Ok(u64::from(self.aether)),
            _ => Err(EconomyError::UnknownResource {
                resource_key: resource_key.to_string(),
            }),
        }
    }

    pub fn set(&mut self, resource_key: &str, value: u64) -> Result<(), EconomyError> {
        match resource_key {
            RESOURCE_GOLD => self.gold = value,
            RESOURCE_WOOD => self.wood = value as u32,
            RESOURCE_STONE => self.stone = value as u32,
            RESOURCE_IRON => self.iron = value as u32,
            RESOURCE_CRYSTAL => self.crystal = value as u32,
            RESOURCE_EMBER => self.ember = value as u32,
            RESOURCE_AETHER => self.aether = value as u32,
            _ => {
                return Err(EconomyError::UnknownResource {
                    resource_key: resource_key.to_string(),
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn nonzero_deltas(
        &self,
        participant_id: &str,
        effect_key: &str,
        phase: &str,
        reason: &str,
    ) -> Vec<ResourceDelta> {
        RESOURCE_KEYS
            .iter()
            .filter_map(|resource_key| {
                let amount = self.get(resource_key).ok()?;
                (amount > 0).then(|| ResourceDelta {
                    participant_id: participant_id.to_string(),
                    resource_key: (*resource_key).to_string(),
                    delta: amount as i64,
                    reason: reason.to_string(),
                    effect_key: effect_key.to_string(),
                    phase: phase.to_string(),
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EconomyParticipantRecord {
    pub participant_id: String,
    pub session_id: String,
    pub balances: ResourceBalances,
    pub last_income_turn: u32,
    pub last_resource_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct IncomeSourceRecord {
    pub source_id: String,
    pub session_id: String,
    pub source_kind: String,
    pub owner_participant_id: Option<String>,
    pub resource_key: String,
    pub amount_per_turn: u64,
    pub captured_turn: u32,
    pub income_started_turn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourcePileEconomyRecord {
    pub pile_id: String,
    pub session_id: String,
    pub reward: ResourceBalances,
    pub x: u16,
    pub y: u16,
    pub state: String,
    pub collected_by_participant_id: Option<String>,
    pub collected_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourceDelta {
    pub participant_id: String,
    pub resource_key: String,
    pub delta: i64,
    pub reason: String,
    pub effect_key: String,
    pub phase: String,
}

impl ResourceDelta {
    #[must_use]
    pub fn ledger_key(&self) -> String {
        let mut hasher = Sha256::new();
        hash_text(&mut hasher, "participant", &self.participant_id);
        hash_text(&mut hasher, "resource", &self.resource_key);
        hash_text(&mut hasher, "effect", &self.effect_key);
        hash_text(&mut hasher, "phase", &self.phase);
        let digest = to_hex(&hasher.finalize());
        format!("{}:{}:{}", self.resource_key, self.phase, &digest[..16])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourceLedgerEntryRecord {
    pub id: String,
    pub session_id: String,
    pub participant_id: String,
    pub command_id: String,
    pub ledger_key: String,
    pub turn_number: u32,
    pub resource_key: String,
    pub delta: i64,
    pub balance_after: u64,
    pub reason: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourceLedgerTurnSummaryRecord {
    pub id: String,
    pub session_id: String,
    pub participant_id: String,
    pub turn_number: u32,
    pub summary_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EconomyState {
    pub session_id: String,
    pub current_turn: u32,
    pub participants: Vec<EconomyParticipantRecord>,
    pub income_sources: Vec<IncomeSourceRecord>,
    pub resource_piles: Vec<ResourcePileEconomyRecord>,
    pub ledger_entries: Vec<ResourceLedgerEntryRecord>,
    pub turn_summaries: Vec<ResourceLedgerTurnSummaryRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EconomySmokeView {
    pub participant_id: String,
    pub after_pickup: ResourceBalances,
    pub after_income: ResourceBalances,
    pub ledger_entries: usize,
    pub captured_source_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EconomyError {
    #[error("participant not found: {participant_id}")]
    ParticipantNotFound { participant_id: String },
    #[error("resource key is unknown: {resource_key}")]
    UnknownResource { resource_key: String },
    #[error("insufficient {resource_key}: available {available}, required {required}")]
    InsufficientResources {
        resource_key: String,
        available: u64,
        required: u64,
    },
    #[error("{resource_key} would exceed cap {cap} with value {attempted}")]
    ValueCapExceeded {
        resource_key: String,
        attempted: u64,
        cap: u64,
    },
    #[error("resource ledger balance mismatch for {ledger_key}")]
    ResourceLedgerBalanceMismatch { ledger_key: String },
    #[error("resource ledger deterministic payload mismatch for {ledger_key}")]
    ResourceLedgerPayloadMismatch { ledger_key: String },
    #[error("resource ledger retention limit exceeded: {max_rows}")]
    ResourceLedgerRetentionLimitExceeded { max_rows: usize },
    #[error("income source not found: {source_id}")]
    IncomeSourceNotFound { source_id: String },
    #[error("resource pile not found: {pile_id}")]
    ResourcePileNotFound { pile_id: String },
    #[error("resource pile {pile_id} was already collected")]
    ResourcePileAlreadyCollected { pile_id: String },
    #[error("invalid trade pair: {from_resource} to {to_resource}")]
    InvalidTradePair {
        from_resource: String,
        to_resource: String,
    },
    #[error("invalid trade amount: {amount}")]
    InvalidTradeAmount { amount: u64 },
    #[error("trade amount {amount} exceeds cap {max}")]
    TradeAmountTooLarge { amount: u64, max: u64 },
}

#[must_use]
pub fn build_first_playable_economy_state() -> EconomyState {
    let fixture = first_playable_fixture();
    build_first_playable_economy_state_for_ids(&fixture.ids)
}

#[must_use]
pub fn build_first_playable_economy_state_for_ids(ids: &FixtureIds) -> EconomyState {
    let scenario = first_playable_scenario();
    let mut participants = Vec::with_capacity(scenario.starts.len());
    let mut income_sources = Vec::new();

    for start in &scenario.starts {
        let participant_id = match start.slot_index {
            0 => ids.participant_one_id.clone(),
            1 => ids.participant_two_id.clone(),
            _ => format!("participant:slot:{}", start.slot_index),
        };
        participants.push(EconomyParticipantRecord {
            participant_id: participant_id.clone(),
            session_id: ids.session_id.clone(),
            balances: ResourceBalances::from_cost(&scenario.starting_state.resources),
            last_income_turn: 1,
            last_resource_command_id: None,
        });
        income_sources.push(IncomeSourceRecord {
            source_id: start.town_key.clone(),
            session_id: ids.session_id.clone(),
            source_kind: "town".to_string(),
            owner_participant_id: Some(participant_id),
            resource_key: RESOURCE_GOLD.to_string(),
            amount_per_turn: super::income::BASE_TOWN_GOLD_INCOME,
            captured_turn: 1,
            income_started_turn: 1,
        });
    }

    for mine in &scenario.mines {
        income_sources.push(IncomeSourceRecord {
            source_id: mine.key.clone(),
            session_id: ids.session_id.clone(),
            source_kind: "mine".to_string(),
            owner_participant_id: mine.owner_slot_index.map(|slot| match slot {
                0 => ids.participant_one_id.clone(),
                1 => ids.participant_two_id.clone(),
                _ => format!("participant:slot:{slot}"),
            }),
            resource_key: mine_income_resource_key(&mine.object_slug).to_string(),
            amount_per_turn: mine_income_amount(&mine.object_slug),
            captured_turn: 1,
            income_started_turn: 1,
        });
    }

    let resource_piles = scenario
        .resource_piles
        .iter()
        .map(|pile| resource_pile_record(&ids.session_id, pile))
        .collect();

    EconomyState {
        session_id: ids.session_id.clone(),
        current_turn: 1,
        participants,
        income_sources,
        resource_piles,
        ledger_entries: Vec::new(),
        turn_summaries: Vec::new(),
    }
}

pub(crate) fn resource_cap(resource_key: &str) -> Result<u64, EconomyError> {
    match resource_key {
        RESOURCE_GOLD => Ok(GOLD_CAP),
        RESOURCE_WOOD | RESOURCE_STONE | RESOURCE_IRON | RESOURCE_CRYSTAL | RESOURCE_EMBER
        | RESOURCE_AETHER => Ok(STANDARD_RESOURCE_CAP),
        _ => Err(EconomyError::UnknownResource {
            resource_key: resource_key.to_string(),
        }),
    }
}

fn resource_pile_record(session_id: &str, pile: &ResourcePileSeed) -> ResourcePileEconomyRecord {
    ResourcePileEconomyRecord {
        pile_id: pile.key.clone(),
        session_id: session_id.to_string(),
        reward: ResourceBalances::from_cost(&pile.reward),
        x: pile.x,
        y: pile.y,
        state: "available".to_string(),
        collected_by_participant_id: None,
        collected_command_id: None,
    }
}

fn mine_income_resource_key(object_slug: &str) -> &'static str {
    match object_slug {
        "crystal-mine" => RESOURCE_CRYSTAL,
        _ => RESOURCE_GOLD,
    }
}

fn mine_income_amount(object_slug: &str) -> u64 {
    match object_slug {
        "crystal-mine" => 1,
        _ => 250,
    }
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update(b":");
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    hasher.update(b"\n");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
