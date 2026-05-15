use crate::content::{
    FIRST_PLAYABLE_CHUNK_SIZE, first_playable_content_manifest, first_playable_scenario,
};
use crate::fixtures::{FixtureIds, first_playable_fixture};

use super::types::{NeutralArmyRecord, NeutralArmyStackRecord, NeutralState};

#[must_use]
pub fn build_first_playable_neutral_state() -> NeutralState {
    let fixture = first_playable_fixture();
    build_first_playable_neutral_state_for_ids(&fixture.ids)
}

#[must_use]
pub fn build_first_playable_neutral_state_for_ids(ids: &FixtureIds) -> NeutralState {
    let scenario = first_playable_scenario();
    let manifest = first_playable_content_manifest();
    let mut armies = Vec::with_capacity(scenario.neutral_armies.len());
    let mut stacks = Vec::new();

    for neutral in &scenario.neutral_armies {
        armies.push(NeutralArmyRecord {
            neutral_army_id: neutral.key.clone(),
            session_id: ids.session_id.clone(),
            scenario_strength_band: neutral.strength_band.clone(),
            x: neutral.x,
            y: neutral.y,
            chunk_x: chunk_coord(neutral.x),
            chunk_y: chunk_coord(neutral.y),
            state: "active".to_string(),
            aggression: "guard".to_string(),
            growth_rule_key: "none".to_string(),
            last_growth_week: 1,
            last_command_id: Some("setup".to_string()),
        });
        for stack in &neutral.stacks {
            let unit = manifest
                .unit(&stack.unit_slug)
                .expect("neutral stack unit should exist in manifest");
            stacks.push(NeutralArmyStackRecord {
                stack_id: format!("neutral-stack:{}:{}", neutral.key, stack.slot_index),
                session_id: ids.session_id.clone(),
                neutral_army_id: neutral.key.clone(),
                unit_slug: stack.unit_slug.clone(),
                slot_index: stack.slot_index,
                quantity: u32::from(stack.quantity),
                front_hp: unit.max_hp,
                last_command_id: Some("setup".to_string()),
            });
        }
    }

    NeutralState {
        session_id: ids.session_id.clone(),
        armies,
        stacks,
        encounters: Vec::new(),
    }
}

fn chunk_coord(value: u16) -> u16 {
    value / u16::from(FIRST_PLAYABLE_CHUNK_SIZE)
}
