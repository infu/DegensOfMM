use crate::champion::ChampionState;
use crate::map::FirstPlayableMapState;
use crate::movement::MovementSyncOutcome;

use super::types::{NeutralArmyEncounterRecord, NeutralError, NeutralGrowthOutcome, NeutralState};

pub fn apply_neutral_encounters_from_movement(
    neutral: &mut NeutralState,
    map: &mut FirstPlayableMapState,
    champions: &ChampionState,
    movement_outcome: &MovementSyncOutcome,
) -> Result<Vec<NeutralArmyEncounterRecord>, NeutralError> {
    let mut encounters = Vec::new();
    for battle in movement_outcome
        .battle_starts
        .iter()
        .filter(|battle| battle.defender_kind == "neutral_army")
    {
        encounters.push(start_neutral_encounter(
            neutral,
            map,
            champions,
            &battle.attacker_champion_id,
            &battle.defender_id_text,
            movement_outcome.from_turn,
            &movement_outcome.command_id,
            "movement",
            &battle.defender_id_text,
            &battle.battle_key,
        )?);
    }
    Ok(encounters)
}

pub fn start_guarded_object_encounter(
    neutral: &mut NeutralState,
    map: &mut FirstPlayableMapState,
    champions: &ChampionState,
    champion_id: &str,
    object_id: &str,
    turn_number: u32,
    command_id: &str,
) -> Result<NeutralArmyEncounterRecord, NeutralError> {
    let object = map
        .world_objects
        .iter()
        .find(|object| object.object_id == object_id)
        .ok_or_else(|| NeutralError::GuardedObjectNotFound {
            object_id: object_id.to_string(),
        })?;
    let guard_id = object.guarded_neutral_army_id.clone().ok_or_else(|| {
        NeutralError::GuardedObjectNotFound {
            object_id: object_id.to_string(),
        }
    })?;
    let battle_key = format!(
        "battle:{}:{turn_number}:{champion_id}:{guard_id}",
        neutral.session_id
    );
    start_neutral_encounter(
        neutral,
        map,
        champions,
        champion_id,
        &guard_id,
        turn_number,
        command_id,
        "guarded_object",
        object_id,
        &battle_key,
    )
}

pub fn start_neutral_encounter(
    neutral: &mut NeutralState,
    map: &mut FirstPlayableMapState,
    champions: &ChampionState,
    champion_id: &str,
    neutral_army_id: &str,
    turn_number: u32,
    command_id: &str,
    source_kind: &str,
    source_id_text: &str,
    battle_key: &str,
) -> Result<NeutralArmyEncounterRecord, NeutralError> {
    champions.champion(champion_id)?;
    if let Some(existing) = neutral.encounters.iter().find(|encounter| {
        encounter.command_id == command_id && encounter.neutral_army_id == neutral_army_id
    }) {
        return Ok(existing.clone());
    }
    let army = neutral.army(neutral_army_id)?;
    if army.state != "active" {
        return Err(NeutralError::NeutralArmyNotActive {
            neutral_army_id: neutral_army_id.to_string(),
            state: army.state.clone(),
        });
    }

    let encounter = NeutralArmyEncounterRecord {
        encounter_id: format!("neutral-encounter:{command_id}:{neutral_army_id}"),
        session_id: neutral.session_id.clone(),
        command_id: command_id.to_string(),
        battle_key: battle_key.to_string(),
        neutral_army_id: neutral_army_id.to_string(),
        attacker_champion_id: champion_id.to_string(),
        source_kind: source_kind.to_string(),
        source_id_text: source_id_text.to_string(),
        turn_number,
        status: "battle_pending".to_string(),
    };
    neutral.encounters.push(encounter.clone());
    let army = neutral.army_mut(neutral_army_id)?;
    army.state = "in_battle".to_string();
    army.last_command_id = Some(command_id.to_string());
    update_map_neutral_state(map, neutral_army_id, "in_battle");
    Ok(encounter)
}

pub fn defeat_neutral_army(
    neutral: &mut NeutralState,
    map: &mut FirstPlayableMapState,
    neutral_army_id: &str,
    command_id: &str,
) -> Result<(), NeutralError> {
    let army = neutral.army_mut(neutral_army_id)?;
    army.state = "defeated".to_string();
    army.last_command_id = Some(command_id.to_string());
    update_map_neutral_state(map, neutral_army_id, "defeated");
    map.cleanup_occupancy_by_subject("neutral_army", neutral_army_id);
    Ok(())
}

pub fn materialize_neutral_growth(
    neutral: &mut NeutralState,
    current_week: u32,
    command_id: &str,
) -> NeutralGrowthOutcome {
    let mut armies_checked = 0_u32;
    for army in &mut neutral.armies {
        armies_checked = armies_checked.saturating_add(1);
        army.last_growth_week = army.last_growth_week.max(current_week);
        army.last_command_id = Some(command_id.to_string());
    }
    NeutralGrowthOutcome {
        current_week,
        armies_checked,
        stacks_changed: 0,
        materialized: false,
        disabled_reason: "neutral_growth_noop_v1_guard_armies_do_not_grow".to_string(),
    }
}

fn update_map_neutral_state(map: &mut FirstPlayableMapState, neutral_army_id: &str, state: &str) {
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "neutral_army" && subject.subject_id_text == neutral_army_id
    }) {
        subject.state = state.to_string();
        subject.public_json = neutral_public_json(neutral_army_id, state, &subject.display_name);
        subject.redacted_json = neutral_redacted_json(neutral_army_id, &subject.display_name);
    }
}

fn neutral_public_json(neutral_army_id: &str, state: &str, strength_label: &str) -> String {
    format!(
        "{{\"type\":\"neutral_army\",\"army_id\":\"{}\",\"strength_label\":\"{}\",\"state\":\"{}\"}}",
        escape_json(neutral_army_id),
        escape_json(strength_label),
        escape_json(state)
    )
}

fn neutral_redacted_json(neutral_army_id: &str, strength_label: &str) -> String {
    format!(
        "{{\"type\":\"neutral_army\",\"army_id\":\"{}\",\"strength_label\":\"{}\"}}",
        escape_json(neutral_army_id),
        escape_json(strength_label)
    )
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
