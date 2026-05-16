use crate::rng::{RollKey, hash64};

use super::types::{
    ArtifactCaptureResult, ArtifactEquipmentRecord, CHAMPION_LEVEL_CAP, CHAMPION_STACK_CAP,
    ChampionError, ChampionProgressionResult, ChampionState,
};

impl ChampionState {
    pub fn effective_movement(
        &self,
        champion_id: &str,
        current_turn: u32,
    ) -> Result<u16, ChampionError> {
        let champion = self.champion(champion_id)?;
        if champion.movement_turn != current_turn {
            Ok(champion.movement_max)
        } else {
            Ok(champion.movement_remaining)
        }
    }

    pub fn spend_movement(
        &mut self,
        champion_id: &str,
        current_turn: u32,
        cost: u16,
        command_id: &str,
    ) -> Result<u16, ChampionError> {
        let available = self.effective_movement(champion_id, current_turn)?;
        if cost > available {
            return Err(ChampionError::InsufficientMovement { cost, available });
        }
        let champion = self.champion_mut(champion_id)?;
        champion.movement_turn = current_turn;
        champion.movement_remaining = available - cost;
        champion.last_command_id = Some(command_id.to_string());
        Ok(champion.movement_remaining)
    }

    pub fn set_champion_status(
        &mut self,
        champion_id: &str,
        status: &str,
        turn: u32,
        command_id: &str,
    ) -> Result<(), ChampionError> {
        if !matches!(status, "active" | "defeated" | "in_battle" | "garrisoned") {
            return Err(ChampionError::InvalidStatus {
                status: status.to_string(),
            });
        }
        let champion = self.champion_mut(champion_id)?;
        champion.status = status.to_string();
        champion.last_command_id = Some(command_id.to_string());
        if status == "defeated" {
            champion.defeated_turn = turn;
        }
        Ok(())
    }

    pub fn add_to_stack(
        &mut self,
        champion_id: &str,
        slot_index: u8,
        quantity: u32,
        command_id: &str,
    ) -> Result<u32, ChampionError> {
        let stack = self
            .army_stacks
            .iter_mut()
            .find(|stack| stack.champion_id == champion_id && stack.slot_index == slot_index)
            .ok_or_else(|| ChampionError::ChampionNotFound {
                champion_id: champion_id.to_string(),
            })?;
        let attempted = stack.quantity.saturating_add(quantity);
        if attempted > CHAMPION_STACK_CAP {
            return Err(ChampionError::StackCapExceeded {
                stack_id: stack.stack_id.clone(),
                attempted,
            });
        }
        stack.quantity = attempted;
        stack.last_command_id = Some(command_id.to_string());
        Ok(stack.quantity)
    }

    pub fn equip_artifact(
        &mut self,
        champion_id: &str,
        artifact_id: &str,
        slot: &str,
        turn: u32,
        command_id: &str,
    ) -> Result<(), ChampionError> {
        self.champion(champion_id)?;
        if self
            .artifact_equipment
            .iter()
            .any(|equipment| equipment.champion_id == champion_id && equipment.slot == slot)
        {
            return Err(ChampionError::EquipmentSlotOccupied {
                champion_id: champion_id.to_string(),
                slot: slot.to_string(),
            });
        }
        if self
            .artifact_equipment
            .iter()
            .any(|equipment| equipment.artifact_id == artifact_id)
        {
            return Err(ChampionError::ArtifactAlreadyEquipped {
                artifact_id: artifact_id.to_string(),
            });
        }
        let artifact = self.artifact_mut(artifact_id)?;
        artifact.owner_champion_id = Some(champion_id.to_string());
        artifact.slot = Some(slot.to_string());
        artifact.state = "equipped".to_string();
        artifact.last_command_id = Some(command_id.to_string());
        self.artifact_equipment.push(ArtifactEquipmentRecord {
            equipment_id: format!("artifact-equipment:{champion_id}:{slot}"),
            session_id: self.session_id.clone(),
            champion_id: champion_id.to_string(),
            artifact_id: artifact_id.to_string(),
            slot: slot.to_string(),
            equipped_turn: turn,
            last_command_id: Some(command_id.to_string()),
        });
        Ok(())
    }

    pub fn capture_artifacts(
        &mut self,
        victor_champion_id: &str,
        defeated_champion_id: &str,
        eliminated: bool,
        command_id: &str,
        roll_key: &RollKey,
    ) -> Result<ArtifactCaptureResult, ChampionError> {
        self.champion(victor_champion_id)?;
        self.champion(defeated_champion_id)?;
        let mut equipped = self
            .artifact_equipment
            .iter()
            .filter(|equipment| equipment.champion_id == defeated_champion_id)
            .cloned()
            .collect::<Vec<_>>();
        equipped.sort_by_key(|equipment| equipment.artifact_id.clone());
        if equipped.is_empty() {
            return Ok(ArtifactCaptureResult {
                victor_champion_id: victor_champion_id.to_string(),
                defeated_champion_id: defeated_champion_id.to_string(),
                captured_artifact_ids: Vec::new(),
            });
        }
        let captured = if eliminated {
            equipped
        } else {
            let index = (hash64(roll_key) as usize) % equipped.len();
            vec![equipped[index].clone()]
        };
        let captured_artifact_ids = captured
            .iter()
            .map(|equipment| equipment.artifact_id.clone())
            .collect::<Vec<_>>();
        self.artifact_equipment
            .retain(|equipment| !captured_artifact_ids.contains(&equipment.artifact_id));
        for artifact_id in &captured_artifact_ids {
            let artifact = self.artifact_mut(artifact_id)?;
            artifact.owner_champion_id = Some(victor_champion_id.to_string());
            artifact.slot = None;
            artifact.state = "stored".to_string();
            artifact.last_command_id = Some(command_id.to_string());
        }
        Ok(ArtifactCaptureResult {
            victor_champion_id: victor_champion_id.to_string(),
            defeated_champion_id: defeated_champion_id.to_string(),
            captured_artifact_ids,
        })
    }

    pub fn grant_experience(
        &mut self,
        champion_id: &str,
        amount: u64,
        command_id: &str,
    ) -> Result<ChampionProgressionResult, ChampionError> {
        let champion = self.champion_mut(champion_id)?;
        let experience_before = champion.experience;
        let level_before = champion.level;
        champion.experience = champion.experience.saturating_add(amount);
        let computed_level =
            1 + (champion.experience / 1_000).min(u64::from(CHAMPION_LEVEL_CAP - 1));
        champion.level = (computed_level as u16).min(CHAMPION_LEVEL_CAP);
        if champion.level > level_before {
            champion.skill_points = champion
                .skill_points
                .saturating_add(champion.level.saturating_sub(level_before));
        }
        champion.last_command_id = Some(command_id.to_string());
        Ok(ChampionProgressionResult {
            champion_id: champion_id.to_string(),
            experience_before,
            experience_after: champion.experience,
            level_before,
            level_after: champion.level,
            skill_points_after: champion.skill_points,
            skill_choice_status: if champion.skill_points > 0 {
                "pending_skill_choice".to_string()
            } else {
                "no_skill_choice_pending".to_string()
            },
        })
    }

    pub fn active_or_recoverable_champions(&self, participant_id: &str) -> Vec<String> {
        self.champions
            .iter()
            .filter(|champion| champion.participant_id == participant_id)
            .filter(|champion| {
                matches!(
                    champion.status.as_str(),
                    "active" | "in_battle" | "garrisoned"
                )
            })
            .map(|champion| champion.champion_id.clone())
            .collect()
    }

    pub(crate) fn champion(
        &self,
        champion_id: &str,
    ) -> Result<&super::types::ChampionRecord, ChampionError> {
        self.champions
            .iter()
            .find(|champion| champion.champion_id == champion_id)
            .ok_or_else(|| ChampionError::ChampionNotFound {
                champion_id: champion_id.to_string(),
            })
    }

    pub(crate) fn champion_mut(
        &mut self,
        champion_id: &str,
    ) -> Result<&mut super::types::ChampionRecord, ChampionError> {
        self.champions
            .iter_mut()
            .find(|champion| champion.champion_id == champion_id)
            .ok_or_else(|| ChampionError::ChampionNotFound {
                champion_id: champion_id.to_string(),
            })
    }

    fn artifact_mut(
        &mut self,
        artifact_id: &str,
    ) -> Result<&mut super::types::ArtifactInstanceRecord, ChampionError> {
        self.artifact_instances
            .iter_mut()
            .find(|artifact| artifact.artifact_id == artifact_id)
            .ok_or_else(|| ChampionError::ArtifactNotFound {
                artifact_id: artifact_id.to_string(),
            })
    }
}
