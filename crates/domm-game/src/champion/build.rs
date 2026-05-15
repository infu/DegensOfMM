use crate::content::{first_playable_content_manifest, first_playable_scenario};
use crate::fixtures::{FixtureIds, first_playable_fixture};

use super::types::{
    ArtifactEquipmentRecord, ArtifactInstanceRecord, ChampionArmyStackRecord, ChampionRecord,
    ChampionState,
};

#[must_use]
pub fn build_first_playable_champion_state() -> ChampionState {
    let fixture = first_playable_fixture();
    build_first_playable_champion_state_for_ids(&fixture.ids)
}

#[must_use]
pub fn build_first_playable_champion_state_for_ids(ids: &FixtureIds) -> ChampionState {
    let manifest = first_playable_content_manifest();
    let scenario = first_playable_scenario();
    let mut champions = Vec::new();
    let mut army_stacks = Vec::new();

    for start in &scenario.starts {
        let participant_id = match start.slot_index {
            0 => ids.participant_one_id.clone(),
            1 => ids.participant_two_id.clone(),
            _ => format!("participant:slot:{}", start.slot_index),
        };
        champions.push(ChampionRecord {
            champion_id: start.champion_key.clone(),
            session_id: ids.session_id.clone(),
            participant_id,
            class_def_id: format!("class:{}", start.champion_class_slug),
            name: start.champion_name.clone(),
            class_key: start.champion_class_slug.clone(),
            status: "active".to_string(),
            x: start.champion_x,
            y: start.champion_y,
            level: scenario.starting_state.champion_level.into(),
            experience: 0,
            might: 1,
            guard: 1,
            wisdom: 1,
            command: 1,
            mana: 10,
            movement_max: scenario.starting_state.champion_movement,
            movement_remaining: scenario.starting_state.champion_movement,
            movement_turn: 1,
            vision_radius: scenario.starting_state.champion_vision,
            defeated_turn: 0,
            last_command_id: None,
        });
        for stack in &start.starting_army_stacks {
            let unit = manifest
                .unit(&stack.unit_slug)
                .expect("starting stack unit should exist");
            army_stacks.push(ChampionArmyStackRecord {
                stack_id: format!("champion-stack:{}:{}", start.champion_key, stack.slot_index),
                session_id: ids.session_id.clone(),
                champion_id: start.champion_key.clone(),
                unit_slug: stack.unit_slug.clone(),
                slot_index: stack.slot_index,
                quantity: u32::from(stack.quantity),
                front_hp: unit.max_hp,
                status: "active".to_string(),
                last_command_id: None,
            });
        }
    }

    let artifact = manifest
        .artifact("bent-banner")
        .expect("first playable artifact should exist");
    let artifact_id = "artifact-instance:bent-banner:west".to_string();
    ChampionState {
        session_id: ids.session_id.clone(),
        champions,
        army_stacks,
        artifact_instances: vec![ArtifactInstanceRecord {
            artifact_id: artifact_id.clone(),
            session_id: ids.session_id.clone(),
            artifact_def_id: artifact.id.clone(),
            owner_champion_id: Some("champion:west".to_string()),
            slot: Some(artifact.slot.clone()),
            x: 0,
            y: 0,
            state: "equipped".to_string(),
            last_command_id: Some("setup".to_string()),
        }],
        artifact_equipment: vec![ArtifactEquipmentRecord {
            equipment_id: "artifact-equipment:champion:west:banner".to_string(),
            session_id: ids.session_id.clone(),
            champion_id: "champion:west".to_string(),
            artifact_id,
            slot: artifact.slot.clone(),
            equipped_turn: 1,
            last_command_id: Some("setup".to_string()),
        }],
    }
}
