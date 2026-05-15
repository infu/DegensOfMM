mod actions;
mod income;
mod ledger;
#[cfg(test)]
mod tests;
mod types;

pub use actions::run_first_playable_economy_smoke;
pub use income::BASE_TOWN_GOLD_INCOME;
pub use ledger::{ResourceApplyBudget, ResourceApplyOutcome, ResourceCapMode};
pub use types::{
    EconomyError, EconomyParticipantRecord, EconomySmokeView, EconomyState, IncomeSourceRecord,
    ResourceBalances, ResourceDelta, ResourceLedgerEntryRecord, ResourceLedgerTurnSummaryRecord,
    ResourcePileEconomyRecord, build_first_playable_economy_state,
};
