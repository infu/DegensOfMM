use crate::content::{first_playable_content_manifest, first_playable_scenario};
use crate::fixtures::{FixtureIds, first_playable_fixture};

use super::types::{
    ArmyStackRecord, ChampionTownRecord, TownBuildingRecord, TownRecord, TownState,
};

#[must_use]
pub fn build_first_playable_town_state() -> TownState {
    let fixture = first_playable_fixture();
    build_first_playable_town_state_for_ids(&fixture.ids)
}

#[must_use]
pub fn build_first_playable_town_state_for_ids(ids: &FixtureIds) -> TownState {
    let scenario = first_playable_scenario();
    let manifest = first_playable_content_manifest();
    let mut towns = Vec::new();
    let mut buildings = Vec::new();
    let mut champions = Vec::new();
    let mut champion_stacks = Vec::new();

    for start in &scenario.starts {
        let participant_id = match start.slot_index {
            0 => ids.participant_one_id.clone(),
            1 => ids.participant_two_id.clone(),
            _ => format!("participant:slot:{}", start.slot_index),
        };
        towns.push(TownRecord {
            town_id: start.town_key.clone(),
            session_id: ids.session_id.clone(),
            owner_participant_id: participant_id.clone(),
            faction_slug: start.faction_slug.clone(),
            name: start.town_name.clone(),
            x: start.town_x,
            y: start.town_y,
            status: "active".to_string(),
            hall_level: scenario.starting_state.town_hall_level,
            fort_level: 0,
            last_built_turn: 0,
            captured_turn: 1,
            income_started_turn: 1,
            unrest_until_turn: 0,
            last_command_id: None,
        });
        buildings.push(TownBuildingRecord {
            building_id: format!("building:{}:crumbling-hall", start.town_key),
            session_id: ids.session_id.clone(),
            town_id: start.town_key.clone(),
            building_slug: "crumbling-hall".to_string(),
            built_turn: 1,
        });
        champions.push(ChampionTownRecord {
            champion_id: start.champion_key.clone(),
            session_id: ids.session_id.clone(),
            participant_id: participant_id.clone(),
            status: "active".to_string(),
            x: start.champion_x,
            y: start.champion_y,
        });
        for stack in &start.starting_army_stacks {
            let unit = manifest
                .unit(&stack.unit_slug)
                .expect("starting stack unit should exist");
            champion_stacks.push(ArmyStackRecord {
                stack_id: format!("champion-stack:{}:{}", start.champion_key, stack.slot_index),
                session_id: ids.session_id.clone(),
                owner_kind: "champion".to_string(),
                owner_id: start.champion_key.clone(),
                unit_slug: stack.unit_slug.clone(),
                slot_index: stack.slot_index,
                quantity: u32::from(stack.quantity),
                front_hp: unit.max_hp,
                status: "active".to_string(),
                last_command_id: None,
            });
        }
    }

    TownState {
        session_id: ids.session_id.clone(),
        current_turn: 1,
        towns,
        buildings,
        recruit_pools: Vec::new(),
        garrison_stacks: Vec::new(),
        champion_stacks,
        champions,
        applied_commands: Vec::new(),
    }
}
