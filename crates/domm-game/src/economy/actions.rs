use super::ledger::{ResourceApplyOutcome, ResourceCapMode};
use super::types::{
    EconomyError, EconomySmokeView, EconomyState, ResourceBalances,
    ResourceLedgerTurnSummaryRecord, build_first_playable_economy_state,
};
use crate::fixtures::first_playable_fixture;

impl EconomyState {
    pub fn collect_resource_pile(
        &mut self,
        participant_id: &str,
        pile_id: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<ResourceApplyOutcome, EconomyError> {
        self.materialize_income(participant_id, current_turn, command_id)?;
        let pile_index = self
            .resource_piles
            .iter()
            .position(|pile| pile.pile_id == pile_id)
            .ok_or_else(|| EconomyError::ResourcePileNotFound {
                pile_id: pile_id.to_string(),
            })?;

        if self.resource_piles[pile_index].state == "collected" {
            if self.resource_piles[pile_index]
                .collected_by_participant_id
                .as_deref()
                == Some(participant_id)
                && self.resource_piles[pile_index]
                    .collected_command_id
                    .as_deref()
                    == Some(command_id)
            {
                return Ok(ResourceApplyOutcome::default());
            }
            return Err(EconomyError::ResourcePileAlreadyCollected {
                pile_id: pile_id.to_string(),
            });
        }

        let reward = self.resource_piles[pile_index].reward.clone();
        let deltas = reward.nonzero_deltas(participant_id, pile_id, "reward", "object_reward");
        let outcome = self.apply_resource_deltas(
            command_id,
            current_turn,
            deltas,
            ResourceCapMode::RejectOnOverflow,
        )?;
        if !outcome.budget_exhausted {
            let pile = &mut self.resource_piles[pile_index];
            pile.state = "collected".to_string();
            pile.collected_by_participant_id = Some(participant_id.to_string());
            pile.collected_command_id = Some(command_id.to_string());
        }
        Ok(outcome)
    }

    pub fn write_turn_summary(
        &mut self,
        participant_id: &str,
        turn_number: u32,
    ) -> Result<ResourceLedgerTurnSummaryRecord, EconomyError> {
        self.participant(participant_id)?;
        if let Some(summary) = self.turn_summaries.iter().find(|summary| {
            summary.participant_id == participant_id && summary.turn_number == turn_number
        }) {
            return Ok(summary.clone());
        }

        let mut totals = ResourceBalances::zero();
        let mut entry_count = 0u32;
        for entry in self.ledger_entries.iter().filter(|entry| {
            entry.participant_id == participant_id
                && entry.turn_number == turn_number
                && entry.status == "applied"
        }) {
            entry_count += 1;
            let current = totals.get(&entry.resource_key)?;
            let next = if entry.delta >= 0 {
                current.saturating_add(entry.delta as u64)
            } else {
                current.saturating_sub(entry.delta.unsigned_abs())
            };
            totals.set(&entry.resource_key, next)?;
        }

        let summary_json = format!(
            "{{\"turn\":{},\"entry_count\":{},\"gold\":{},\"wood\":{},\"stone\":{},\"iron\":{},\"crystal\":{},\"ember\":{},\"aether\":{}}}",
            turn_number,
            entry_count,
            totals.gold,
            totals.wood,
            totals.stone,
            totals.iron,
            totals.crystal,
            totals.ember,
            totals.aether
        );
        let summary = ResourceLedgerTurnSummaryRecord {
            id: format!(
                "resource-summary:{}:{participant_id}:{turn_number}",
                self.session_id
            ),
            session_id: self.session_id.clone(),
            participant_id: participant_id.to_string(),
            turn_number,
            summary_json,
        };
        self.turn_summaries.push(summary.clone());
        Ok(summary)
    }
}

pub fn run_first_playable_economy_smoke() -> Result<EconomySmokeView, EconomyError> {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = fixture.ids.participant_one_id;

    state.collect_resource_pile(
        &participant_id,
        "pile:west-wood-1",
        1,
        "command:economy:pickup:west-wood",
    )?;
    let after_pickup = state.participant(&participant_id)?.balances.clone();
    state.capture_income_source(
        "mine:west-gold",
        &participant_id,
        2,
        "command:economy:capture:west-gold",
    )?;
    state.materialize_income(&participant_id, 3, "command:economy:income:turn3")?;
    let after_income = state.participant(&participant_id)?.balances.clone();

    Ok(EconomySmokeView {
        participant_id,
        after_pickup,
        after_income,
        ledger_entries: state.ledger_entries.len(),
        captured_source_id: "mine:west-gold".to_string(),
    })
}
