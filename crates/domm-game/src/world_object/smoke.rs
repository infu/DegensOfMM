use crate::champion::{ChampionState, build_first_playable_champion_state};
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::economy::build_first_playable_economy_state;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::{FirstPlayableMapState, build_first_playable_map_state, set_visibility_bit};
use crate::movement::{
    MoveCoord, MovementSyncBudget, build_first_playable_movement_state, submit_move_intent,
    sync_session_turn,
};

use super::actions::{
    apply_movement_object_interactions, interact_with_world_object, world_object_scoreboard,
};
use super::types::{
    WorldObjectError, WorldObjectSmokeView, build_first_playable_world_object_state,
};

pub fn run_first_playable_world_object_smoke() -> Result<WorldObjectSmokeView, WorldObjectError> {
    let fixture = first_playable_fixture();
    let participant_id = fixture.ids.participant_one_id;
    let champion_id = "champion:west";
    let mut map = build_first_playable_map_state();
    let mut champions = build_first_playable_champion_state();
    let mut economy = build_first_playable_economy_state();
    let mut movement = build_first_playable_movement_state();
    let mut objects = build_first_playable_world_object_state();

    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &participant_id,
        champion_id,
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        11_001,
        1_000,
    )?;
    let movement_outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )?;
    apply_movement_object_interactions(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &movement_outcome,
        1,
        TURN_DURATION_MS,
    )?;
    let after_pickup = economy.participant(&participant_id)?.balances.clone();

    place_champion(&mut map, &mut champions, champion_id, 14, 30)?;
    reveal_tile(&mut map, &participant_id, 14, 30);
    interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &participant_id,
        champion_id,
        "mine:west-crystal",
        3,
        11_002,
        TURN_DURATION_MS * 3,
    )?;

    place_champion(&mut map, &mut champions, champion_id, 24, 20)?;
    reveal_tile(&mut map, &participant_id, 24, 20);
    mark_guard_defeated(&mut map, "neutral:north-objective");
    interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &participant_id,
        champion_id,
        "objective:north",
        4,
        11_003,
        TURN_DURATION_MS * 4,
    )?;

    let mine = economy
        .income_sources
        .iter()
        .find(|source| source.source_id == "mine:west-crystal")
        .expect("first playable mine should exist");
    let central_objectives_owned = world_object_scoreboard(&map)
        .into_iter()
        .find(|score| {
            score.scoring_kind == "central_objective"
                && score.owner_participant_id.as_deref() == Some(&participant_id)
        })
        .map_or(0, |score| score.object_count);

    Ok(WorldObjectSmokeView {
        participant_id,
        after_pickup,
        captured_mine_id: "mine:west-crystal".to_string(),
        mine_income_started_turn: mine.income_started_turn,
        captured_objective_id: "objective:north".to_string(),
        central_objectives_owned,
        object_commands: objects.commands.len() as u32,
        participant_visits: objects.participant_visits.len() as u32,
        champion_visits: objects.champion_visits.len() as u32,
    })
}

fn place_champion(
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    champion_id: &str,
    x: u16,
    y: u16,
) -> Result<(), WorldObjectError> {
    {
        let champion = champions.champion_mut(champion_id)?;
        champion.x = x;
        champion.y = y;
        champion.status = "active".to_string();
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
        Some("world-object-smoke:place".to_string()),
    )?;
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.x = x;
        subject.y = y;
        subject.chunk_x = x / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        subject.chunk_y = y / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    }
    Ok(())
}

fn reveal_tile(map: &mut FirstPlayableMapState, participant_id: &str, x: u16, y: u16) {
    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let chunk_x = x / chunk_size;
    let chunk_y = y / chunk_size;
    let local_x = x % chunk_size;
    let local_y = y % chunk_size;
    if let Some(chunk) = map.visibility_chunks.iter_mut().find(|chunk| {
        chunk.participant_id == participant_id
            && chunk.chunk_x == chunk_x
            && chunk.chunk_y == chunk_y
    }) {
        let index = usize::from(local_y) * usize::from(chunk.width) + usize::from(local_x);
        set_visibility_bit(&mut chunk.visible_blob, index);
        set_visibility_bit(&mut chunk.discovered_blob, index);
    }
}

fn mark_guard_defeated(map: &mut FirstPlayableMapState, guard_id: &str) {
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "neutral_army" && subject.subject_id_text == guard_id
    }) {
        subject.state = "defeated".to_string();
    }
    map.cleanup_occupancy_by_subject("neutral_army", guard_id);
}
