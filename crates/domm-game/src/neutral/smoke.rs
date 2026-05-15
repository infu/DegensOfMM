use crate::champion::build_first_playable_champion_state;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::build_first_playable_map_state;
use crate::movement::{
    MoveCoord, MovementSyncBudget, build_first_playable_movement_state, submit_move_intent,
    sync_session_turn,
};

use super::actions::{apply_neutral_encounters_from_movement, defeat_neutral_army};
use super::build::build_first_playable_neutral_state;
use super::types::{NeutralError, NeutralSmokeView, strength_label_for_quantity};

pub fn run_first_playable_neutral_smoke() -> Result<NeutralSmokeView, NeutralError> {
    let fixture = first_playable_fixture();
    let participant_id = fixture.ids.participant_one_id;
    let mut map = build_first_playable_map_state();
    let mut champions = build_first_playable_champion_state();
    let mut movement = build_first_playable_movement_state();
    let mut neutral = build_first_playable_neutral_state();

    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        "champion:west",
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        12_001,
        1_000,
    )?;
    let movement_outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )?;
    let encounters = apply_neutral_encounters_from_movement(
        &mut neutral,
        &mut map,
        &champions,
        &movement_outcome,
    )?;
    let encounter = encounters
        .first()
        .expect("first playable neutral smoke should create encounter")
        .clone();
    defeat_neutral_army(
        &mut neutral,
        &mut map,
        "neutral:west-mine",
        "command:neutral:defeat:west-mine",
    )?;

    Ok(NeutralSmokeView {
        neutral_army_id: "neutral:west-mine".to_string(),
        strength_label: strength_label_for_quantity(neutral.quantity_for("neutral:west-mine"))
            .to_string(),
        encounter_id: encounter.encounter_id,
        battle_key: encounter.battle_key,
        defeated_state: neutral.army("neutral:west-mine")?.state.clone(),
        occupancy_rows_after_defeat: map
            .occupancy_rows
            .iter()
            .filter(|row| {
                row.occupant_kind == "neutral_army" && row.occupant_id_text == "neutral:west-mine"
            })
            .count(),
    })
}
