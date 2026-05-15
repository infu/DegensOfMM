use crate::content::{BuildingContent, ResourceCost, first_playable_content_manifest};
use crate::economy::{
    EconomyState, IncomeSourceRecord, ResourceBalances, ResourceCapMode, ResourceDelta,
    build_first_playable_economy_state,
};
use crate::fixtures::first_playable_fixture;

use super::recruitment::current_week;
use super::types::{
    BuildPreview, TownBuildingRecord, TownError, TownRecruitPoolRecord, TownSmokeView, TownState,
};

impl TownState {
    pub fn preview_build_town_structure(
        &self,
        economy: &EconomyState,
        participant_id: &str,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
    ) -> Result<BuildPreview, TownError> {
        let manifest = first_playable_content_manifest();
        let Some(building) = manifest.building(building_slug) else {
            return Err(TownError::BuildingNotFound {
                building_slug: building_slug.to_string(),
            });
        };
        let cost = ResourceBalances::from_cost(&building.cost);
        let town = self.town(town_id)?;
        if town.owner_participant_id != participant_id {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "not_owner",
            ));
        }
        if self.has_building(town_id, building_slug) {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "already_built",
            ));
        }
        if town.last_built_turn >= current_turn {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "already_built_this_turn",
            ));
        }
        if building
            .faction_slug
            .as_deref()
            .is_some_and(|faction| faction != town.faction_slug)
        {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "wrong_faction",
            ));
        }
        if let Some(missing) = building
            .requires_building_slugs
            .iter()
            .find(|required| !self.has_building(town_id, required))
        {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                format!("missing_prerequisite:{missing}"),
            ));
        }
        let participant = economy.participant(participant_id)?;
        if !can_afford(&participant.balances, &building.cost)? {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "insufficient_resources",
            ));
        }
        Ok(BuildPreview {
            allowed: true,
            disabled_reason: None,
            town_id: town_id.to_string(),
            building_slug: building_slug.to_string(),
            cost,
        })
    }

    pub fn submit_build_town_structure(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<BuildPreview, TownError> {
        self.submit_build_town_structure_inner(
            economy,
            participant_id,
            town_id,
            building_slug,
            current_turn,
            command_id,
            false,
        )
    }

    pub fn submit_build_town_structure_with_interruption(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<BuildPreview, TownError> {
        self.submit_build_town_structure_inner(
            economy,
            participant_id,
            town_id,
            building_slug,
            current_turn,
            command_id,
            true,
        )
    }

    fn submit_build_town_structure_inner(
        &mut self,
        economy: &mut EconomyState,
        participant_id: &str,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
        command_id: &str,
        interrupt_after_spend: bool,
    ) -> Result<BuildPreview, TownError> {
        if self.applied_commands.iter().any(|id| id == command_id) {
            return self.preview_build_receipt(town_id, building_slug);
        }
        economy.materialize_income(participant_id, current_turn, command_id)?;
        let spend_already_applied = economy.ledger_entries.iter().any(|entry| {
            entry.command_id == command_id
                && entry.reason == "build_spend"
                && entry.status == "applied"
        });
        let preview = if spend_already_applied {
            self.preview_build_without_affordability(town_id, building_slug, current_turn)?
        } else {
            self.preview_build_town_structure(
                economy,
                participant_id,
                town_id,
                building_slug,
                current_turn,
            )?
        };
        if !preview.allowed {
            return Err(TownError::Disabled {
                reason: preview
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| "disabled".to_string()),
            });
        }

        if !spend_already_applied {
            let cost_deltas =
                negative_cost_deltas(participant_id, building_slug, "build", &preview.cost);
            economy.apply_resource_deltas(
                command_id,
                current_turn,
                cost_deltas,
                ResourceCapMode::RejectOnOverflow,
            )?;
        }
        if interrupt_after_spend {
            return Err(TownError::InterruptedAfterSpend);
        }
        self.insert_building(town_id, building_slug, current_turn, command_id)?;
        self.apply_building_side_effects(economy, town_id, building_slug, current_turn)?;
        self.applied_commands.push(command_id.to_string());
        Ok(preview)
    }

    pub fn repair_town_caches(&mut self, town_id: &str) -> Result<(), TownError> {
        self.town(town_id)?;
        let hall_level = u8::from(self.has_building(town_id, "crumbling-hall"));
        let town = self.town_mut(town_id)?;
        town.hall_level = hall_level.max(1);
        town.fort_level = 0;
        Ok(())
    }

    fn preview_build_without_affordability(
        &self,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
    ) -> Result<BuildPreview, TownError> {
        let manifest = first_playable_content_manifest();
        let building =
            manifest
                .building(building_slug)
                .ok_or_else(|| TownError::BuildingNotFound {
                    building_slug: building_slug.to_string(),
                })?;
        let town = self.town(town_id)?;
        let cost = ResourceBalances::from_cost(&building.cost);
        if self.has_building(town_id, building_slug) {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "already_built",
            ));
        }
        if town.last_built_turn >= current_turn {
            return Ok(BuildPreview::disabled(
                town_id,
                building_slug,
                cost,
                "already_built_this_turn",
            ));
        }
        Ok(BuildPreview {
            allowed: true,
            disabled_reason: None,
            town_id: town_id.to_string(),
            building_slug: building_slug.to_string(),
            cost,
        })
    }

    fn preview_build_receipt(
        &self,
        town_id: &str,
        building_slug: &str,
    ) -> Result<BuildPreview, TownError> {
        let manifest = first_playable_content_manifest();
        let building =
            manifest
                .building(building_slug)
                .ok_or_else(|| TownError::BuildingNotFound {
                    building_slug: building_slug.to_string(),
                })?;
        Ok(BuildPreview {
            allowed: true,
            disabled_reason: None,
            town_id: town_id.to_string(),
            building_slug: building_slug.to_string(),
            cost: ResourceBalances::from_cost(&building.cost),
        })
    }

    fn insert_building(
        &mut self,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<(), TownError> {
        if !self.has_building(town_id, building_slug) {
            self.buildings.push(TownBuildingRecord {
                building_id: format!("building:{town_id}:{building_slug}"),
                session_id: self.session_id.clone(),
                town_id: town_id.to_string(),
                building_slug: building_slug.to_string(),
                built_turn: current_turn,
            });
        }
        let town = self.town_mut(town_id)?;
        town.last_built_turn = current_turn;
        town.last_command_id = Some(command_id.to_string());
        Ok(())
    }

    fn apply_building_side_effects(
        &mut self,
        economy: &mut EconomyState,
        town_id: &str,
        building_slug: &str,
        current_turn: u32,
    ) -> Result<(), TownError> {
        let manifest = first_playable_content_manifest();
        let building = manifest
            .building(building_slug)
            .expect("building was validated before side effects");
        if let Some(unit_slug) = building.unlocks_unit_slug.as_deref() {
            self.ensure_recruit_pool(town_id, unit_slug, current_week(current_turn));
        }
        if building.effect_key.as_deref() == Some("town_income_gold_250") {
            let town = self.town(town_id)?;
            economy.income_sources.push(IncomeSourceRecord {
                source_id: format!("{town_id}:{building_slug}"),
                session_id: economy.session_id.clone(),
                source_kind: "building".to_string(),
                owner_participant_id: Some(town.owner_participant_id.clone()),
                resource_key: "gold".to_string(),
                amount_per_turn: 250,
                captured_turn: current_turn,
                income_started_turn: current_turn,
            });
        }
        self.repair_town_caches(town_id)?;
        Ok(())
    }

    fn ensure_recruit_pool(&mut self, town_id: &str, unit_slug: &str, week: u32) {
        if self
            .recruit_pools
            .iter()
            .any(|pool| pool.town_id == town_id && pool.unit_slug == unit_slug)
        {
            return;
        }
        self.recruit_pools.push(TownRecruitPoolRecord {
            pool_id: format!("recruit-pool:{town_id}:{unit_slug}"),
            session_id: self.session_id.clone(),
            town_id: town_id.to_string(),
            unit_slug: unit_slug.to_string(),
            available: 0,
            last_growth_week: week,
            last_command_id: None,
        });
    }

    pub(crate) fn town(&self, town_id: &str) -> Result<&super::types::TownRecord, TownError> {
        self.towns
            .iter()
            .find(|town| town.town_id == town_id)
            .ok_or_else(|| TownError::TownNotFound {
                town_id: town_id.to_string(),
            })
    }

    pub(crate) fn town_mut(
        &mut self,
        town_id: &str,
    ) -> Result<&mut super::types::TownRecord, TownError> {
        self.towns
            .iter_mut()
            .find(|town| town.town_id == town_id)
            .ok_or_else(|| TownError::TownNotFound {
                town_id: town_id.to_string(),
            })
    }

    pub(crate) fn has_building(&self, town_id: &str, building_slug: &str) -> bool {
        self.buildings
            .iter()
            .any(|building| building.town_id == town_id && building.building_slug == building_slug)
    }
}

fn can_afford(balances: &ResourceBalances, cost: &ResourceCost) -> Result<bool, TownError> {
    Ok(balances.gold >= u64::from(cost.gold)
        && balances.wood >= cost.wood
        && balances.stone >= cost.stone
        && balances.iron >= cost.iron
        && balances.crystal >= cost.crystal
        && balances.ember >= cost.ember
        && balances.aether >= cost.aether)
}

pub(crate) fn negative_cost_deltas(
    participant_id: &str,
    effect_key: &str,
    phase: &str,
    cost: &ResourceBalances,
) -> Vec<ResourceDelta> {
    let mut deltas = Vec::new();
    for (resource_key, amount) in [
        ("gold", cost.gold),
        ("wood", u64::from(cost.wood)),
        ("stone", u64::from(cost.stone)),
        ("iron", u64::from(cost.iron)),
        ("crystal", u64::from(cost.crystal)),
        ("ember", u64::from(cost.ember)),
        ("aether", u64::from(cost.aether)),
    ] {
        if amount > 0 {
            deltas.push(ResourceDelta {
                participant_id: participant_id.to_string(),
                resource_key: resource_key.to_string(),
                delta: -(amount as i64),
                reason: format!("{phase}_spend"),
                effect_key: effect_key.to_string(),
                phase: phase.to_string(),
            });
        }
    }
    deltas
}

pub fn run_first_playable_town_smoke() -> Result<TownSmokeView, TownError> {
    let fixture = first_playable_fixture();
    let mut economy = build_first_playable_economy_state();
    let mut town = super::build::build_first_playable_town_state();
    let participant_id = fixture.ids.participant_one_id;

    town.submit_build_town_structure(
        &mut economy,
        &participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:town:build:training-yard",
    )?;
    town.submit_recruit_units(
        &mut economy,
        &participant_id,
        "town:west",
        "mudhook-levy",
        4,
        super::types::RecruitTarget::TownGarrison { slot_index: None },
        8,
        "command:town:recruit:mudhook",
    )?;
    let final_resources = economy.participant(&participant_id)?.balances.clone();

    Ok(TownSmokeView {
        town_id: "town:west".to_string(),
        built_building_slug: "freehold-training-yard".to_string(),
        recruited_unit_slug: "mudhook-levy".to_string(),
        recruited_quantity: 4,
        final_resources,
    })
}

#[allow(dead_code)]
fn _building_content_for_docs(_building: &BuildingContent) {}
