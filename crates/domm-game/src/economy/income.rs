use super::ledger::{ResourceApplyBudget, ResourceApplyOutcome, ResourceCapMode};
use super::types::{EconomyError, EconomyState, ResourceDelta};

pub const BASE_TOWN_GOLD_INCOME: u64 = 500;
const MAX_LAZY_INCOME_TURNS: u32 = 14;

impl EconomyState {
    pub fn materialize_income(
        &mut self,
        participant_id: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<ResourceApplyOutcome, EconomyError> {
        self.materialize_income_with_budget(
            participant_id,
            current_turn,
            command_id,
            "lazy_income",
            ResourceApplyBudget::recovery_default(),
        )
    }

    pub fn materialize_income_with_budget(
        &mut self,
        participant_id: &str,
        current_turn: u32,
        command_id: &str,
        phase: &str,
        budget: ResourceApplyBudget,
    ) -> Result<ResourceApplyOutcome, EconomyError> {
        let deltas = self.income_deltas(participant_id, current_turn, phase)?;
        let outcome = self.apply_resource_deltas_with_budget(
            command_id,
            current_turn,
            deltas,
            ResourceCapMode::SaturateAtCap,
            budget,
        )?;
        if !outcome.budget_exhausted {
            self.participant_mut(participant_id)?.last_income_turn = current_turn;
        }
        Ok(outcome)
    }

    pub fn capture_income_source(
        &mut self,
        source_id: &str,
        new_owner_participant_id: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<(), EconomyError> {
        let source_index = self
            .income_sources
            .iter()
            .position(|source| source.source_id == source_id)
            .ok_or_else(|| EconomyError::IncomeSourceNotFound {
                source_id: source_id.to_string(),
            })?;
        let old_owner = self.income_sources[source_index]
            .owner_participant_id
            .clone();

        if let Some(old_owner) = old_owner.as_deref() {
            self.materialize_income_with_budget(
                old_owner,
                current_turn,
                command_id,
                "income_cutover_old_owner",
                ResourceApplyBudget::recovery_default(),
            )?;
        }
        self.materialize_income_with_budget(
            new_owner_participant_id,
            current_turn,
            command_id,
            "income_cutover_new_owner",
            ResourceApplyBudget::recovery_default(),
        )?;

        let source = &mut self.income_sources[source_index];
        source.owner_participant_id = Some(new_owner_participant_id.to_string());
        source.captured_turn = current_turn;
        source.income_started_turn = current_turn;
        Ok(())
    }

    fn income_deltas(
        &self,
        participant_id: &str,
        current_turn: u32,
        phase: &str,
    ) -> Result<Vec<ResourceDelta>, EconomyError> {
        let participant = self.participant(participant_id)?;
        let mut deltas = Vec::new();
        for source in self
            .income_sources
            .iter()
            .filter(|source| source.owner_participant_id.as_deref() == Some(participant_id))
        {
            let accrual_start = participant
                .last_income_turn
                .max(source.captured_turn)
                .max(source.income_started_turn);
            let turns = current_turn
                .saturating_sub(accrual_start)
                .min(MAX_LAZY_INCOME_TURNS);
            if turns == 0 || source.amount_per_turn == 0 {
                continue;
            }
            let amount = source.amount_per_turn.saturating_mul(u64::from(turns));
            deltas.push(ResourceDelta {
                participant_id: participant_id.to_string(),
                resource_key: source.resource_key.clone(),
                delta: amount.min(i64::MAX as u64) as i64,
                reason: "income_tick".to_string(),
                effect_key: format!("income:{}:to:{current_turn}", source.source_id),
                phase: phase.to_string(),
            });
        }
        Ok(deltas)
    }
}
