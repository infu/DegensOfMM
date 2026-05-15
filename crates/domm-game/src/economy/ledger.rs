use super::types::{
    EconomyError, EconomyParticipantRecord, EconomyState, ResourceDelta, ResourceLedgerEntryRecord,
    resource_cap,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCapMode {
    RejectOnOverflow,
    SaturateAtCap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceApplyBudget {
    pub max_ledger_rows: usize,
}

impl ResourceApplyBudget {
    #[must_use]
    pub const fn recovery_default() -> Self {
        Self {
            max_ledger_rows: 32,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceApplyOutcome {
    pub ledger_rows_touched: usize,
    pub balance_updates: usize,
    pub skipped_applied_rows: usize,
    pub budget_exhausted: bool,
}

impl EconomyState {
    pub fn apply_resource_deltas(
        &mut self,
        command_id: &str,
        turn_number: u32,
        deltas: Vec<ResourceDelta>,
        cap_mode: ResourceCapMode,
    ) -> Result<ResourceApplyOutcome, EconomyError> {
        self.apply_resource_deltas_with_budget(
            command_id,
            turn_number,
            deltas,
            cap_mode,
            ResourceApplyBudget::recovery_default(),
        )
    }

    pub fn apply_resource_deltas_with_budget(
        &mut self,
        command_id: &str,
        turn_number: u32,
        mut deltas: Vec<ResourceDelta>,
        cap_mode: ResourceCapMode,
        budget: ResourceApplyBudget,
    ) -> Result<ResourceApplyOutcome, EconomyError> {
        let mut outcome = ResourceApplyOutcome::default();
        deltas.sort_by_key(ResourceDelta::ledger_key);

        for delta in deltas {
            let ledger_key = delta.ledger_key();
            if self.ledger_entry_applied(command_id, &ledger_key)? {
                outcome.skipped_applied_rows += 1;
                continue;
            }
            if outcome.ledger_rows_touched >= budget.max_ledger_rows {
                outcome.budget_exhausted = true;
                break;
            }
            self.apply_one_delta(command_id, turn_number, &delta, &ledger_key, cap_mode)?;
            outcome.ledger_rows_touched += 1;
            outcome.balance_updates += 1;
        }

        Ok(outcome)
    }

    fn apply_one_delta(
        &mut self,
        command_id: &str,
        turn_number: u32,
        delta: &ResourceDelta,
        ledger_key: &str,
        cap_mode: ResourceCapMode,
    ) -> Result<(), EconomyError> {
        let entry_index = self
            .ledger_entries
            .iter()
            .position(|entry| entry.command_id == command_id && entry.ledger_key == ledger_key);
        let entry = if let Some(index) = entry_index {
            self.verify_pending_entry(index, command_id, turn_number, delta, ledger_key)?;
            self.ledger_entries[index].clone()
        } else {
            let participant = self.participant(&delta.participant_id)?;
            let current = participant.balances.get(&delta.resource_key)?;
            let (balance_after, actual_delta) =
                project_balance(&delta.resource_key, current, delta.delta, cap_mode)?;
            if actual_delta == 0 {
                return Ok(());
            }
            let entry = ResourceLedgerEntryRecord {
                id: format!("ledger:{command_id}:{ledger_key}"),
                session_id: self.session_id.clone(),
                participant_id: delta.participant_id.clone(),
                command_id: command_id.to_string(),
                ledger_key: ledger_key.to_string(),
                turn_number,
                resource_key: delta.resource_key.clone(),
                delta: actual_delta,
                balance_after,
                reason: delta.reason.clone(),
                status: "pending".to_string(),
            };
            self.ledger_entries.push(entry.clone());
            entry
        };

        self.reconcile_balance(command_id, &entry)?;
        let index = self
            .ledger_entries
            .iter()
            .position(|row| row.command_id == command_id && row.ledger_key == ledger_key)
            .expect("ledger entry should exist before marking applied");
        self.ledger_entries[index].status = "applied".to_string();
        Ok(())
    }

    fn verify_pending_entry(
        &self,
        index: usize,
        command_id: &str,
        turn_number: u32,
        delta: &ResourceDelta,
        ledger_key: &str,
    ) -> Result<(), EconomyError> {
        let entry = &self.ledger_entries[index];
        if entry.status == "applied" {
            return Ok(());
        }
        if entry.command_id != command_id
            || entry.ledger_key != ledger_key
            || entry.turn_number != turn_number
            || entry.participant_id != delta.participant_id
            || entry.resource_key != delta.resource_key
            || entry.reason != delta.reason
        {
            return Err(EconomyError::ResourceLedgerPayloadMismatch {
                ledger_key: ledger_key.to_string(),
            });
        }
        Ok(())
    }

    fn ledger_entry_applied(
        &self,
        command_id: &str,
        ledger_key: &str,
    ) -> Result<bool, EconomyError> {
        let Some(entry) = self
            .ledger_entries
            .iter()
            .find(|entry| entry.command_id == command_id && entry.ledger_key == ledger_key)
        else {
            return Ok(false);
        };
        if entry.status == "applied" {
            return Ok(true);
        }
        Ok(false)
    }

    fn reconcile_balance(
        &mut self,
        command_id: &str,
        entry: &ResourceLedgerEntryRecord,
    ) -> Result<(), EconomyError> {
        let participant = self.participant_mut(&entry.participant_id)?;
        let current = participant.balances.get(&entry.resource_key)?;
        let before = balance_before(entry)?;
        if current == before {
            participant
                .balances
                .set(&entry.resource_key, entry.balance_after)?;
            participant.last_resource_command_id = Some(command_id.to_string());
            return Ok(());
        }
        if current == entry.balance_after {
            participant.last_resource_command_id = Some(command_id.to_string());
            return Ok(());
        }
        Err(EconomyError::ResourceLedgerBalanceMismatch {
            ledger_key: entry.ledger_key.clone(),
        })
    }

    pub(crate) fn participant(
        &self,
        participant_id: &str,
    ) -> Result<&EconomyParticipantRecord, EconomyError> {
        self.participants
            .iter()
            .find(|participant| participant.participant_id == participant_id)
            .ok_or_else(|| EconomyError::ParticipantNotFound {
                participant_id: participant_id.to_string(),
            })
    }

    pub(crate) fn participant_mut(
        &mut self,
        participant_id: &str,
    ) -> Result<&mut EconomyParticipantRecord, EconomyError> {
        self.participants
            .iter_mut()
            .find(|participant| participant.participant_id == participant_id)
            .ok_or_else(|| EconomyError::ParticipantNotFound {
                participant_id: participant_id.to_string(),
            })
    }
}

fn project_balance(
    resource_key: &str,
    current: u64,
    delta: i64,
    cap_mode: ResourceCapMode,
) -> Result<(u64, i64), EconomyError> {
    if delta < 0 {
        let required = delta.unsigned_abs();
        if current < required {
            return Err(EconomyError::InsufficientResources {
                resource_key: resource_key.to_string(),
                available: current,
                required,
            });
        }
        return Ok((current - required, delta));
    }

    let cap = resource_cap(resource_key)?;
    let attempted = current.saturating_add(delta as u64);
    if attempted > cap {
        return match cap_mode {
            ResourceCapMode::RejectOnOverflow => Err(EconomyError::ValueCapExceeded {
                resource_key: resource_key.to_string(),
                attempted,
                cap,
            }),
            ResourceCapMode::SaturateAtCap => Ok((cap, (cap - current) as i64)),
        };
    }
    Ok((attempted, delta))
}

fn balance_before(entry: &ResourceLedgerEntryRecord) -> Result<u64, EconomyError> {
    if entry.delta >= 0 {
        return entry
            .balance_after
            .checked_sub(entry.delta as u64)
            .ok_or_else(|| EconomyError::ResourceLedgerBalanceMismatch {
                ledger_key: entry.ledger_key.clone(),
            });
    }
    entry
        .balance_after
        .checked_add(entry.delta.unsigned_abs())
        .ok_or_else(|| EconomyError::ResourceLedgerBalanceMismatch {
            ledger_key: entry.ledger_key.clone(),
        })
}
