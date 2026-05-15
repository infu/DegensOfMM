use crate::content::{UnitContent, first_playable_content_manifest};
use crate::economy::{EconomyState, ResourceBalances, ResourceCapMode};

use super::actions::negative_cost_deltas;
use super::types::{
    ARMY_STACK_CAP, ArmyStackRecord, MAX_ARMY_SLOTS, RECRUIT_POOL_CAP, RecruitPreview,
    RecruitTarget, TownError, TownRecruitPoolRecord, TownState,
};

impl TownState {
    pub fn preview_recruit_units(
        &self,
        economy: &EconomyState,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        current_turn: u32,
    ) -> Result<RecruitPreview, TownError> {
        self.preview_recruit_units_inner(
            economy,
            participant_id,
            town_id,
            unit_slug,
            quantity,
            target,
            current_turn,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn preview_recruit_units_inner(
        &self,
        economy: &EconomyState,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        current_turn: u32,
        check_affordability: bool,
    ) -> Result<RecruitPreview, TownError> {
        let manifest = first_playable_content_manifest();
        let unit = manifest
            .unit(unit_slug)
            .ok_or_else(|| TownError::UnitNotFound {
                unit_slug: unit_slug.to_string(),
            })?;
        let total_cost = unit_cost(unit, quantity);
        let town = self.town(town_id)?;
        if town.owner_participant_id != participant_id {
            return Ok(RecruitPreview::disabled(
                town_id,
                unit_slug,
                quantity,
                total_cost,
                0,
                "not_owner",
            ));
        }
        let Some(pool) = self
            .recruit_pools
            .iter()
            .find(|pool| pool.town_id == town_id && pool.unit_slug == unit_slug)
        else {
            return Ok(RecruitPreview::disabled(
                town_id,
                unit_slug,
                quantity,
                total_cost,
                0,
                "recruit_pool_empty",
            ));
        };
        let available = effective_available(pool, unit, current_turn);
        if quantity == 0 || quantity > available {
            return Ok(RecruitPreview::disabled(
                town_id,
                unit_slug,
                quantity,
                total_cost,
                available,
                "recruit_pool_empty",
            ));
        }
        let participant = economy.participant(participant_id)?;
        if check_affordability && participant.balances.gold < total_cost.gold {
            return Ok(RecruitPreview::disabled(
                town_id,
                unit_slug,
                quantity,
                total_cost,
                available,
                "insufficient_resources",
            ));
        }
        let target_slot_index =
            self.resolve_recruit_slot(participant_id, town_id, unit_slug, quantity, target)?;
        Ok(RecruitPreview {
            allowed: true,
            disabled_reason: None,
            town_id: town_id.to_string(),
            unit_slug: unit_slug.to_string(),
            quantity,
            target_slot_index: Some(target_slot_index),
            total_cost,
            available,
        })
    }

    pub fn submit_recruit_units(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        current_turn: u32,
        command_id: &str,
    ) -> Result<RecruitPreview, TownError> {
        self.submit_recruit_units_inner(
            economy,
            participant_id,
            town_id,
            unit_slug,
            quantity,
            target,
            current_turn,
            command_id,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_recruit_units_with_interruption(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        current_turn: u32,
        command_id: &str,
    ) -> Result<RecruitPreview, TownError> {
        self.submit_recruit_units_inner(
            economy,
            participant_id,
            town_id,
            unit_slug,
            quantity,
            target,
            current_turn,
            command_id,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_recruit_units_inner(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        current_turn: u32,
        command_id: &str,
        interrupt_after_spend: bool,
    ) -> Result<RecruitPreview, TownError> {
        if self.applied_commands.iter().any(|id| id == command_id) {
            return self.preview_recruit_receipt(town_id, unit_slug, quantity);
        }
        economy.materialize_income(participant_id, current_turn, command_id)?;
        self.materialize_recruit_pool_growth(town_id, unit_slug, current_turn, command_id)?;
        let spend_already_applied = economy.ledger_entries.iter().any(|entry| {
            entry.command_id == command_id
                && entry.reason == "recruit_spend"
                && entry.status == "applied"
        });
        let preview = self.preview_recruit_units_inner(
            economy,
            participant_id,
            town_id,
            unit_slug,
            quantity,
            &target,
            current_turn,
            !spend_already_applied,
        )?;
        if !preview.allowed {
            return Err(TownError::Disabled {
                reason: preview
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| "disabled".to_string()),
            });
        }
        if !spend_already_applied {
            let deltas =
                negative_cost_deltas(participant_id, unit_slug, "recruit", &preview.total_cost);
            economy.apply_resource_deltas(
                command_id,
                current_turn,
                deltas,
                ResourceCapMode::RejectOnOverflow,
            )?;
        }
        if interrupt_after_spend {
            return Err(TownError::InterruptedAfterSpend);
        }
        self.decrement_pool(town_id, unit_slug, quantity, command_id)?;
        self.merge_or_insert_stack(
            &target,
            town_id,
            unit_slug,
            quantity,
            preview.target_slot_index.unwrap(),
            command_id,
        )?;
        self.applied_commands.push(command_id.to_string());
        Ok(preview)
    }

    pub fn materialize_recruit_pool_growth(
        &mut self,
        town_id: &str,
        unit_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<(), TownError> {
        let manifest = first_playable_content_manifest();
        let unit = manifest
            .unit(unit_slug)
            .ok_or_else(|| TownError::UnitNotFound {
                unit_slug: unit_slug.to_string(),
            })?;
        let pool = self.pool_mut(town_id, unit_slug)?;
        let week = current_week(current_turn);
        let growth_weeks = week.saturating_sub(pool.last_growth_week).min(2);
        if growth_weeks > 0 {
            let growth = u32::from(unit.weekly_growth).saturating_mul(growth_weeks);
            pool.available = pool.available.saturating_add(growth).min(RECRUIT_POOL_CAP);
            pool.last_growth_week = week;
            pool.last_command_id = Some(command_id.to_string());
        }
        Ok(())
    }

    fn resolve_recruit_slot(
        &self,
        participant_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
    ) -> Result<u8, TownError> {
        match target {
            RecruitTarget::TownGarrison { slot_index } => resolve_slot(
                &self.garrison_stacks,
                "town",
                town_id,
                unit_slug,
                quantity,
                *slot_index,
            ),
            RecruitTarget::Champion {
                champion_id,
                slot_index,
            } => {
                let town = self.town(town_id)?;
                let champion = self
                    .champions
                    .iter()
                    .find(|champion| champion.champion_id == *champion_id)
                    .ok_or_else(|| TownError::ChampionNotFound {
                        champion_id: champion_id.clone(),
                    })?;
                if champion.participant_id != participant_id || champion.status != "active" {
                    return Err(TownError::Disabled {
                        reason: "not_active_stack".to_string(),
                    });
                }
                if champion.x != town.x || champion.y != town.y {
                    return Err(TownError::Disabled {
                        reason: "champion_not_at_town".to_string(),
                    });
                }
                resolve_slot(
                    &self.champion_stacks,
                    "champion",
                    champion_id,
                    unit_slug,
                    quantity,
                    *slot_index,
                )
            }
        }
    }

    fn decrement_pool(
        &mut self,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        command_id: &str,
    ) -> Result<(), TownError> {
        let pool = self.pool_mut(town_id, unit_slug)?;
        pool.available = pool.available.saturating_sub(quantity);
        pool.last_command_id = Some(command_id.to_string());
        Ok(())
    }

    fn merge_or_insert_stack(
        &mut self,
        target: &RecruitTarget,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        slot_index: u8,
        command_id: &str,
    ) -> Result<(), TownError> {
        let (owner_kind, owner_id) = match target {
            RecruitTarget::TownGarrison { .. } => ("town", town_id),
            RecruitTarget::Champion { champion_id, .. } => ("champion", champion_id.as_str()),
        };
        let stacks = if owner_kind == "town" {
            &mut self.garrison_stacks
        } else {
            &mut self.champion_stacks
        };
        if let Some(stack) = stacks
            .iter_mut()
            .find(|stack| stack.owner_id == owner_id && stack.slot_index == slot_index)
        {
            if stack.unit_slug != unit_slug {
                return Err(TownError::UnitStackIncompatible);
            }
            stack.quantity = stack.quantity.saturating_add(quantity).min(ARMY_STACK_CAP);
            stack.last_command_id = Some(command_id.to_string());
            return Ok(());
        }
        let manifest = first_playable_content_manifest();
        let unit = manifest
            .unit(unit_slug)
            .ok_or_else(|| TownError::UnitNotFound {
                unit_slug: unit_slug.to_string(),
            })?;
        stacks.push(ArmyStackRecord {
            stack_id: format!("{owner_kind}-stack:{owner_id}:{slot_index}"),
            session_id: self.session_id.clone(),
            owner_kind: owner_kind.to_string(),
            owner_id: owner_id.to_string(),
            unit_slug: unit_slug.to_string(),
            slot_index,
            quantity,
            front_hp: unit.max_hp,
            status: "active".to_string(),
            last_command_id: Some(command_id.to_string()),
        });
        Ok(())
    }

    fn pool_mut(
        &mut self,
        town_id: &str,
        unit_slug: &str,
    ) -> Result<&mut TownRecruitPoolRecord, TownError> {
        self.recruit_pools
            .iter_mut()
            .find(|pool| pool.town_id == town_id && pool.unit_slug == unit_slug)
            .ok_or_else(|| TownError::Disabled {
                reason: "recruit_pool_empty".to_string(),
            })
    }

    fn preview_recruit_receipt(
        &self,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
    ) -> Result<RecruitPreview, TownError> {
        let manifest = first_playable_content_manifest();
        let unit = manifest
            .unit(unit_slug)
            .ok_or_else(|| TownError::UnitNotFound {
                unit_slug: unit_slug.to_string(),
            })?;
        Ok(RecruitPreview {
            allowed: true,
            disabled_reason: None,
            town_id: town_id.to_string(),
            unit_slug: unit_slug.to_string(),
            quantity,
            target_slot_index: None,
            total_cost: unit_cost(unit, quantity),
            available: 0,
        })
    }
}

pub(crate) fn current_week(turn: u32) -> u32 {
    ((turn.saturating_sub(1)) / 7) + 1
}

fn effective_available(pool: &TownRecruitPoolRecord, unit: &UnitContent, turn: u32) -> u32 {
    let growth_weeks = current_week(turn)
        .saturating_sub(pool.last_growth_week)
        .min(2);
    pool.available
        .saturating_add(u32::from(unit.weekly_growth).saturating_mul(growth_weeks))
        .min(RECRUIT_POOL_CAP)
}

fn unit_cost(unit: &UnitContent, quantity: u32) -> ResourceBalances {
    ResourceBalances {
        gold: u64::from(unit.cost.gold).saturating_mul(u64::from(quantity)),
        wood: unit.cost.wood.saturating_mul(quantity),
        stone: unit.cost.stone.saturating_mul(quantity),
        iron: unit.cost.iron.saturating_mul(quantity),
        crystal: unit.cost.crystal.saturating_mul(quantity),
        ember: unit.cost.ember.saturating_mul(quantity),
        aether: unit.cost.aether.saturating_mul(quantity),
    }
}

fn resolve_slot(
    stacks: &[ArmyStackRecord],
    owner_kind: &str,
    owner_id: &str,
    unit_slug: &str,
    quantity: u32,
    requested: Option<u8>,
) -> Result<u8, TownError> {
    if let Some(slot) = requested {
        if slot >= MAX_ARMY_SLOTS {
            return Err(TownError::RecruitTargetFull);
        }
        return validate_slot(stacks, owner_kind, owner_id, unit_slug, quantity, slot)
            .map(|()| slot);
    }
    for slot in 0..MAX_ARMY_SLOTS {
        if validate_slot(stacks, owner_kind, owner_id, unit_slug, quantity, slot).is_ok() {
            return Ok(slot);
        }
    }
    Err(TownError::RecruitTargetFull)
}

fn validate_slot(
    stacks: &[ArmyStackRecord],
    owner_kind: &str,
    owner_id: &str,
    unit_slug: &str,
    quantity: u32,
    slot: u8,
) -> Result<(), TownError> {
    let Some(stack) = stacks.iter().find(|stack| {
        stack.owner_kind == owner_kind && stack.owner_id == owner_id && stack.slot_index == slot
    }) else {
        return Ok(());
    };
    if stack.unit_slug != unit_slug {
        return Err(TownError::UnitStackIncompatible);
    }
    if stack.quantity.saturating_add(quantity) > ARMY_STACK_CAP {
        return Err(TownError::RecruitTargetFull);
    }
    Ok(())
}
