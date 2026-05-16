use super::types::{ArtifactView, ChampionState, ChampionView, ChampionViewResult};

impl ChampionState {
    pub fn champion_view_for(
        &self,
        viewer_participant_id: &str,
        champion_id: &str,
        currently_visible: bool,
        current_turn: u32,
    ) -> ChampionViewResult {
        let Ok(champion) = self.champion(champion_id) else {
            return ChampionViewResult::Hidden {
                champion_id: champion_id.to_string(),
                visibility: "hidden".to_string(),
            };
        };
        let own = champion.participant_id == viewer_participant_id;
        if !own && !currently_visible {
            return ChampionViewResult::Hidden {
                champion_id: champion_id.to_string(),
                visibility: "hidden".to_string(),
            };
        }
        let army_stacks = if own || currently_visible {
            self.army_stacks
                .iter()
                .filter(|stack| stack.champion_id == champion_id)
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let artifacts = if own {
            self.artifact_equipment
                .iter()
                .filter(|equipment| equipment.champion_id == champion_id)
                .filter_map(|equipment| {
                    let artifact = self
                        .artifact_instances
                        .iter()
                        .find(|artifact| artifact.artifact_id == equipment.artifact_id)?;
                    Some(ArtifactView {
                        artifact_id: artifact.artifact_id.clone(),
                        artifact_def_id: artifact.artifact_def_id.clone(),
                        slot: equipment.slot.clone(),
                        state: artifact.state.clone(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        ChampionViewResult::Visible(ChampionView {
            champion_id: champion.champion_id.clone(),
            owner_participant_id: champion.participant_id.clone(),
            name: (!(!own && !currently_visible)).then(|| champion.name.clone()),
            class_def_id: champion.class_def_id.clone(),
            class_key: champion.class_key.clone(),
            status: champion.status.clone(),
            x: champion.x,
            y: champion.y,
            effective_movement: self
                .effective_movement(champion_id, current_turn)
                .unwrap_or(champion.movement_remaining),
            movement_max: champion.movement_max,
            mana: self
                .effective_mana(champion_id, current_turn)
                .unwrap_or(champion.mana),
            mana_max: champion.mana_max,
            skill_points: champion.skill_points,
            skill_keys: if own {
                champion.skill_keys.clone()
            } else {
                Vec::new()
            },
            spell_slugs: if own {
                self.learned_spell_slugs(champion_id)
            } else {
                Vec::new()
            },
            vision_radius: champion.vision_radius,
            strength_label: strength_label(&army_stacks),
            army_stacks,
            artifacts,
            redacted: !own,
        })
    }
}

fn strength_label(stacks: &[super::types::ChampionArmyStackRecord]) -> String {
    let total = stacks.iter().map(|stack| stack.quantity).sum::<u32>();
    match total {
        0 => "none",
        1..=20 => "small",
        21..=60 => "modest",
        _ => "large",
    }
    .to_string()
}
