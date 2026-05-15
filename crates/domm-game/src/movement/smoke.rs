use crate::champion::build_first_playable_champion_state;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::build_first_playable_map_state;

use super::submit::submit_move_intent;
use super::sync::sync_session_turn;
use super::types::{
    MoveCoord, MovementError, MovementSmokeView, MovementSyncBudget,
    build_first_playable_movement_state,
};

pub fn run_first_playable_movement_smoke() -> Result<MovementSmokeView, MovementError> {
    let fixture = first_playable_fixture();
    let participant_id = fixture.ids.participant_one_id;
    let champion_id = "champion:west";
    let mut map = build_first_playable_map_state();
    let mut champions = build_first_playable_champion_state();
    let mut movement = build_first_playable_movement_state();

    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        champion_id,
        vec![MoveCoord::new(9, 24)],
        10_001,
        1_000,
    )?;
    sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )?;

    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        champion_id,
        vec![MoveCoord::new(10, 24)],
        10_002,
        TURN_DURATION_MS + 1_000,
    )?;
    sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS * 2,
        MovementSyncBudget::default(),
    )?;

    let champion = champions.champion(champion_id)?;
    Ok(MovementSmokeView {
        session_id: movement.session_id,
        final_turn: movement.current_turn,
        champion_id: champion_id.to_string(),
        final_x: champion.x,
        final_y: champion.y,
        snapshots: movement.snapshots.len() as u32,
        system_commands: movement.system_commands.len() as u32,
    })
}
