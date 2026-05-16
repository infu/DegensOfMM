use candid::CandidType;
use serde::{Deserialize, Serialize};

use super::types::{
    EconomyError, RESOURCE_AETHER, RESOURCE_CRYSTAL, RESOURCE_EMBER, RESOURCE_GOLD, RESOURCE_STONE,
    RESOURCE_WOOD, ResourceBalances,
};
use crate::rng::{RollKey, hash64};

pub const TAVERN_OFFERS_PER_WEEK: usize = 2;
pub const TAVERN_HIRE_COST_GOLD: u32 = 2_500;
pub const MARKET_TRADE_MAX_INPUT: u64 = 25_000;
pub const DWELLING_POOL_CAP: u32 = 99;
pub const DWELLING_GROWTH_PER_WEEK: u16 = 4;
pub const DWELLING_RECRUIT_MAX_QUANTITY: u32 = 25;

const TAVERN_NAMES: [&str; 8] = [
    "Nix of Bad Terms",
    "Vara Due-at-Dusk",
    "Old Pell of the Ledger",
    "Brann Tollscar",
    "Sable Marrow",
    "Ketch Underbridge",
    "Mirel Coppergrin",
    "Dorn of No Refunds",
];

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TavernOfferView {
    pub offer_key: String,
    pub town_id: String,
    pub week_number: u32,
    pub offer_slot: u8,
    pub champion_class_slug: String,
    pub candidate_name: String,
    pub cost_gold: u32,
    pub status: String,
    pub hired_champion_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TavernOffersView {
    pub town_id: String,
    pub week_number: u32,
    pub offers: Vec<TavernOfferView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionHirePreview {
    pub allowed: bool,
    pub disabled_reason: Option<String>,
    pub town_id: String,
    pub offer_key: String,
    pub champion_class_slug: String,
    pub candidate_name: String,
    pub cost: ResourceBalances,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MarketTradePreview {
    pub allowed: bool,
    pub disabled_reason: Option<String>,
    pub from_resource: String,
    pub to_resource: String,
    pub amount_in: u64,
    pub amount_out: u64,
    pub rate_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DwellingPoolView {
    pub object_id: String,
    pub owner_participant_id: Option<String>,
    pub unit_slug: String,
    pub available: u32,
    pub last_growth_week: u32,
    pub growth_per_week: u16,
    pub direct_recruit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DwellingRecruitPreview {
    pub allowed: bool,
    pub disabled_reason: Option<String>,
    pub object_id: String,
    pub unit_slug: String,
    pub quantity: u32,
    pub target_champion_id: String,
    pub total_cost: ResourceBalances,
    pub available: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ExpandedEconomyReceipt {
    pub command_id: String,
    pub action: String,
    pub town_id: Option<String>,
    pub object_id: Option<String>,
    pub champion_id: Option<String>,
    pub offer_key: Option<String>,
    pub from_resource: Option<String>,
    pub to_resource: Option<String>,
    pub amount_in: u64,
    pub amount_out: u64,
    pub unit_slug: Option<String>,
    pub quantity: u32,
    pub resources_after: ResourceBalances,
}

#[must_use]
pub fn week_for_turn(turn_number: u32) -> u32 {
    turn_number.saturating_sub(1) / 7 + 1
}

#[must_use]
pub fn tavern_offer_key(town_id: &str, week_number: u32, offer_slot: u8) -> String {
    format!("tavern:{town_id}:week:{week_number}:slot:{offer_slot}")
}

#[must_use]
pub fn deterministic_tavern_offer(
    session_seed: &str,
    town_id: &str,
    week_number: u32,
    offer_slot: u8,
    class_slugs: &[String],
) -> TavernOfferView {
    let class_index = deterministic_index(
        session_seed,
        "tavern_class",
        week_number,
        town_id,
        offer_slot,
        class_slugs.len(),
    );
    let name_index = deterministic_index(
        session_seed,
        "tavern_name",
        week_number,
        town_id,
        offer_slot,
        TAVERN_NAMES.len(),
    );
    TavernOfferView {
        offer_key: tavern_offer_key(town_id, week_number, offer_slot),
        town_id: town_id.to_string(),
        week_number,
        offer_slot,
        champion_class_slug: class_slugs.get(class_index).cloned().unwrap_or_default(),
        candidate_name: TAVERN_NAMES[name_index].to_string(),
        cost_gold: TAVERN_HIRE_COST_GOLD.saturating_add(u32::from(offer_slot) * 500),
        status: "available".to_string(),
        hired_champion_id: None,
    }
}

pub fn market_trade_quote(
    from_resource: &str,
    to_resource: &str,
    amount_in: u64,
) -> Result<MarketTradePreview, EconomyError> {
    if amount_in == 0 {
        return Err(EconomyError::InvalidTradeAmount { amount: amount_in });
    }
    if amount_in > MARKET_TRADE_MAX_INPUT {
        return Err(EconomyError::TradeAmountTooLarge {
            amount: amount_in,
            max: MARKET_TRADE_MAX_INPUT,
        });
    }
    let (denominator, numerator, rate_key) =
        trade_rate(from_resource, to_resource).ok_or_else(|| EconomyError::InvalidTradePair {
            from_resource: from_resource.to_string(),
            to_resource: to_resource.to_string(),
        })?;
    if amount_in % denominator != 0 {
        return Err(EconomyError::InvalidTradeAmount { amount: amount_in });
    }
    let amount_out = amount_in / denominator * numerator;
    if amount_out == 0 {
        return Err(EconomyError::InvalidTradeAmount { amount: amount_in });
    }
    Ok(MarketTradePreview {
        allowed: true,
        disabled_reason: None,
        from_resource: from_resource.to_string(),
        to_resource: to_resource.to_string(),
        amount_in,
        amount_out,
        rate_key: rate_key.to_string(),
    })
}

#[must_use]
pub fn dwelling_effective_available(
    available: u32,
    last_growth_week: u32,
    current_week: u32,
    growth_per_week: u16,
) -> u32 {
    let elapsed = current_week.saturating_sub(last_growth_week);
    available
        .saturating_add(elapsed.saturating_mul(u32::from(growth_per_week)))
        .min(DWELLING_POOL_CAP)
}

#[must_use]
pub fn dwelling_recruit_cost(unit_gold_cost: u32, quantity: u32) -> ResourceBalances {
    ResourceBalances {
        gold: u64::from(unit_gold_cost).saturating_mul(u64::from(quantity)),
        wood: 0,
        stone: 0,
        iron: 0,
        crystal: 0,
        ember: 0,
        aether: 0,
    }
}

fn deterministic_index(
    session_seed: &str,
    domain: &str,
    week_number: u32,
    town_id: &str,
    offer_slot: u8,
    len: usize,
) -> usize {
    if len == 0 {
        return 0;
    }
    let key = RollKey::new(
        session_seed,
        domain,
        week_number,
        format!("week:{week_number}:slot:{offer_slot}"),
        town_id,
        "tavern",
        u32::from(offer_slot),
    );
    (hash64(&key) % len as u64) as usize
}

fn trade_rate(from_resource: &str, to_resource: &str) -> Option<(u64, u64, &'static str)> {
    match (from_resource, to_resource) {
        (RESOURCE_WOOD, RESOURCE_CRYSTAL) => Some((10, 1, "wood_to_crystal_10_1")),
        (RESOURCE_STONE, RESOURCE_CRYSTAL) => Some((10, 1, "stone_to_crystal_10_1")),
        (RESOURCE_CRYSTAL | RESOURCE_EMBER | RESOURCE_AETHER, RESOURCE_GOLD) => {
            Some((5, 1_000, "rare_to_gold_5_1000"))
        }
        (RESOURCE_GOLD, RESOURCE_CRYSTAL | RESOURCE_EMBER | RESOURCE_AETHER) => {
            Some((2_500, 1, "gold_to_rare_2500_1"))
        }
        _ => None,
    }
}
