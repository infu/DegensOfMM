mod actions;
mod expanded;
mod income;
mod ledger;
#[cfg(test)]
mod tests;
mod types;

pub use actions::run_first_playable_economy_smoke;
pub use expanded::{
    ChampionHirePreview, DWELLING_GROWTH_PER_WEEK, DWELLING_POOL_CAP,
    DWELLING_RECRUIT_MAX_QUANTITY, DwellingPoolView, DwellingRecruitPreview,
    ExpandedEconomyReceipt, MARKET_TRADE_MAX_INPUT, MarketTradePreview, TAVERN_HIRE_COST_GOLD,
    TAVERN_OFFERS_PER_WEEK, TavernOfferView, TavernOffersView, deterministic_tavern_offer,
    dwelling_effective_available, dwelling_recruit_cost, market_trade_quote, tavern_offer_key,
    week_for_turn,
};
pub use income::BASE_TOWN_GOLD_INCOME;
pub use ledger::{ResourceApplyBudget, ResourceApplyOutcome, ResourceCapMode};
pub use types::{
    EconomyError, EconomyParticipantRecord, EconomySmokeView, EconomyState, IncomeSourceRecord,
    ResourceBalances, ResourceDelta, ResourceLedgerEntryRecord, ResourceLedgerTurnSummaryRecord,
    ResourcePileEconomyRecord, build_first_playable_economy_state,
};
