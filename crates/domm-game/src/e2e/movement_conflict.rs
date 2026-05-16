use crate::champion::{ChampionState, build_first_playable_champion_state};
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::{FirstPlayableMapState, build_first_playable_map_state};
use crate::movement::{
    MoveCoord, MovementState, MovementSyncBudget, build_first_playable_movement_state,
    submit_move_intent, sync_session_turn,
};

use super::types::{EndToEndError, MovementConflictReport};

pub fn run_e2e_movement_conflict_probe() -> Result<MovementConflictReport, EndToEndError> {
    let fixture = first_playable_fixture();
    let participant_id = fixture.ids.participant_one_id;
    let mut movement = build_first_playable_movement_state();
    let mut map = build_first_playable_map_state();
    let mut champions = build_first_playable_champion_state();

    place_champion(
        &mut map,
        &mut champions,
        "champion:east",
        &participant_id,
        10,
        24,
    )?;
    {
        let east = champions.champion_mut("champion:east")?;
        east.movement_remaining = 200;
    }
    {
        let west = champions.champion_mut("champion:west")?;
        west.movement_remaining = 240;
    }

    submit_move(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        "champion:west",
        vec![MoveCoord::new(9, 24)],
        19_001,
    )?;
    submit_move(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        "champion:east",
        vec![MoveCoord::new(9, 24)],
        19_002,
    )?;

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )?;
    let west = champions.champion("champion:west")?;
    let east = champions.champion("champion:east")?;
    let outcomes = outcome
        .snapshots
        .iter()
        .map(|snapshot| snapshot.outcome.clone())
        .collect::<Vec<_>>();

    Ok(MovementConflictReport {
        snapshot_count: outcome.snapshots.len() as u32,
        stopped_tile_conflict: outcomes
            .iter()
            .any(|outcome| outcome == "stopped_tile_conflict"),
        west_final_x: west.x,
        west_final_y: west.y,
        east_final_x: east.x,
        east_final_y: east.y,
        outcomes,
    })
}

fn submit_move(
    movement: &mut MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    nonce: u64,
) -> Result<(), EndToEndError> {
    submit_move_intent(
        movement,
        map,
        champions,
        participant_id,
        champion_id,
        path,
        nonce,
        1_000,
    )?;
    Ok(())
}

fn place_champion(
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    champion_id: &str,
    participant_id: &str,
    x: u16,
    y: u16,
) -> Result<(), EndToEndError> {
    {
        let champion = champions.champion_mut(champion_id)?;
        champion.participant_id = participant_id.to_string();
        champion.status = "active".to_string();
        champion.x = x;
        champion.y = y;
        champion.movement_turn = 1;
        champion.movement_remaining = champion.movement_max;
    }
    map.cleanup_occupancy_by_subject("champion", champion_id);
    map.insert_occupancy_footprint(
        x,
        y,
        1,
        1,
        "champion",
        "champion",
        champion_id,
        true,
        Some("e2e:place-conflict-champion".to_string()),
    )?;
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.owner_participant_id = Some(participant_id.to_string());
        subject.x = x;
        subject.y = y;
        subject.chunk_x = x / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        subject.chunk_y = y / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    }
    Ok(())
}
