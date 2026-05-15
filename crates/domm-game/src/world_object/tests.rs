use crate::champion::{ChampionState, build_first_playable_champion_state};
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::economy::{EconomyState, build_first_playable_economy_state};
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::{
    FirstPlayableMapState, SubjectViewResult, build_first_playable_map_state, set_visibility_bit,
};
use crate::movement::{
    MoveCoord, MovementSyncBudget, build_first_playable_movement_state, submit_move_intent,
    sync_session_turn,
};

use super::actions::{
    apply_movement_object_interactions, interact_with_world_object, record_champion_object_visit,
    record_participant_object_visit, world_object_scoreboard,
};
use super::smoke::run_first_playable_world_object_smoke;
use super::types::{WorldObjectError, WorldObjectState, build_first_playable_world_object_state};

fn fixture_states() -> (
    WorldObjectState,
    FirstPlayableMapState,
    ChampionState,
    EconomyState,
    String,
    String,
) {
    let fixture = first_playable_fixture();
    (
        build_first_playable_world_object_state(),
        build_first_playable_map_state(),
        build_first_playable_champion_state(),
        build_first_playable_economy_state(),
        fixture.ids.participant_one_id,
        fixture.ids.participant_two_id,
    )
}

fn place_champion(
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    champion_id: &str,
    x: u16,
    y: u16,
) {
    {
        let champion = champions.champion_mut(champion_id).unwrap();
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
        Some("test:place".to_string()),
    )
    .unwrap();
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.x = x;
        subject.y = y;
        subject.chunk_x = x / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        subject.chunk_y = y / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    }
}

fn reveal_tile(map: &mut FirstPlayableMapState, participant_id: &str, x: u16, y: u16) {
    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let chunk_x = x / chunk_size;
    let chunk_y = y / chunk_size;
    let local_x = x % chunk_size;
    let local_y = y % chunk_size;
    let chunk = map
        .visibility_chunks
        .iter_mut()
        .find(|chunk| {
            chunk.participant_id == participant_id
                && chunk.chunk_x == chunk_x
                && chunk.chunk_y == chunk_y
        })
        .unwrap();
    let index = usize::from(local_y) * usize::from(chunk.width) + usize::from(local_x);
    set_visibility_bit(&mut chunk.visible_blob, index);
    set_visibility_bit(&mut chunk.discovered_blob, index);
}

fn mark_guard_defeated(map: &mut FirstPlayableMapState, guard_id: &str) {
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "neutral_army" && subject.subject_id_text == guard_id
    }) {
        subject.state = "defeated".to_string();
    }
}

#[test]
fn resource_pickup_records_once_only_visits_and_rewards_resources() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 9, 23);

    let outcome = interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "pile:west-wood-1",
        1,
        901,
        1_000,
    )
    .unwrap();
    let second = interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "pile:west-wood-1",
        1,
        902,
        1_001,
    );

    assert_eq!(outcome.visit_key, "once");
    assert_eq!(economy.participant(&west).unwrap().balances.wood, 15);
    assert_eq!(objects.participant_visits.len(), 1);
    assert_eq!(objects.champion_visits.len(), 1);
    assert!(matches!(
        second,
        Err(WorldObjectError::ObjectAlreadyVisited { .. })
    ));
}

#[test]
fn duplicate_interaction_replays_without_duplicate_ledger_or_visits() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 9, 23);

    interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "pile:west-wood-1",
        1,
        911,
        1_000,
    )
    .unwrap();
    let replay = interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "pile:west-wood-1",
        1,
        911,
        1_001,
    )
    .unwrap();

    assert!(replay.duplicate_replay);
    assert_eq!(economy.ledger_entries.len(), 1);
    assert_eq!(objects.participant_visits.len(), 1);
    assert_eq!(objects.commands.len(), 1);
}

#[test]
fn refreshable_visit_keys_allow_new_windows_only() {
    let (mut objects, _, _, _, west, _) = fixture_states();

    record_participant_object_visit(
        &mut objects,
        "object:refreshable",
        &west,
        "week:1",
        "training_bonus",
        3,
        "command:visit:week1",
    )
    .unwrap();
    let duplicate = record_participant_object_visit(
        &mut objects,
        "object:refreshable",
        &west,
        "week:1",
        "training_bonus",
        4,
        "command:visit:week1-other",
    );
    record_champion_object_visit(
        &mut objects,
        "object:refreshable",
        "champion:west",
        "week:2",
        "training_bonus",
        8,
        "command:visit:week2",
    )
    .unwrap();

    assert!(matches!(
        duplicate,
        Err(WorldObjectError::ObjectAlreadyVisited { .. })
    ));
    assert_eq!(objects.participant_visits.len(), 1);
    assert_eq!(objects.champion_visits.len(), 1);
}

#[test]
fn mine_capture_changes_owner_and_starts_income_next_turn() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 14, 30);
    reveal_tile(&mut map, &west, 14, 30);

    let outcome = interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "mine:west-crystal",
        3,
        921,
        3_000,
    )
    .unwrap();
    let source = economy
        .income_sources
        .iter()
        .find(|source| source.source_id == "mine:west-crystal")
        .unwrap();

    assert_eq!(
        outcome.captured_source_id.as_deref(),
        Some("mine:west-crystal")
    );
    assert_eq!(source.owner_participant_id.as_deref(), Some(west.as_str()));
    assert_eq!(source.income_started_turn, 3);
    assert_eq!(
        map.world_objects
            .iter()
            .find(|object| object.object_id == "mine:west-crystal")
            .unwrap()
            .owner_participant_id
            .as_deref(),
        Some(west.as_str())
    );
}

#[test]
fn central_objective_capture_updates_scoring() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 24, 20);
    reveal_tile(&mut map, &west, 24, 20);
    mark_guard_defeated(&mut map, "neutral:north-objective");

    interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "objective:north",
        4,
        931,
        4_000,
    )
    .unwrap();
    let scores = world_object_scoreboard(&map);

    assert!(scores.iter().any(|score| {
        score.scoring_kind == "central_objective"
            && score.owner_participant_id.as_deref() == Some(west.as_str())
            && score.object_count == 1
    }));
}

#[test]
fn guarded_object_blocks_interaction_until_guard_is_defeated() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 12, 22);
    reveal_tile(&mut map, &west, 12, 22);

    let guarded = interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "mine:west-gold",
        2,
        941,
        2_000,
    );

    assert!(matches!(
        guarded,
        Err(WorldObjectError::ObjectGuarded { .. })
    ));
    assert_eq!(objects.commands[0].status, "failed");
}

#[test]
fn object_redaction_uses_existing_visibility_contract_after_state_change() {
    let (mut objects, mut map, mut champions, mut economy, west, east) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:west", 9, 23);

    interact_with_world_object(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &west,
        "champion:west",
        "pile:west-wood-1",
        1,
        951,
        1_000,
    )
    .unwrap();
    let visible = map.subject_view(&west, "world_object", "pile:west-wood-1");
    let hidden = map.subject_view(&east, "world_object", "pile:west-wood-1");

    let SubjectViewResult::Visible(view) = visible else {
        panic!("west pile should be visible to west");
    };
    assert!(
        view.details_json
            .contains("\"scoring_kind\":\"resource_pile\"")
    );
    assert!(view.details_json.contains("\"state\":\"collected\""));
    assert!(matches!(hidden, SubjectViewResult::NotVisible { .. }));
}

#[test]
fn movement_object_stop_applies_interaction_with_system_idempotency() {
    let (mut objects, mut map, mut champions, mut economy, west, _) = fixture_states();
    let mut movement = build_first_playable_movement_state();
    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        961,
        1_000,
    )
    .unwrap();
    let movement_outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    let first = apply_movement_object_interactions(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &movement_outcome,
        1,
        TURN_DURATION_MS,
    )
    .unwrap();
    let replay = apply_movement_object_interactions(
        &mut objects,
        &mut map,
        &mut economy,
        &champions,
        &movement_outcome,
        1,
        TURN_DURATION_MS + 1,
    )
    .unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(replay.len(), 1);
    assert!(replay[0].duplicate_replay);
    assert_eq!(economy.participant(&west).unwrap().balances.wood, 15);
    assert_eq!(economy.ledger_entries.len(), 1);
}

#[test]
fn first_playable_world_object_smoke_covers_pickup_mine_and_objective() {
    let smoke = run_first_playable_world_object_smoke().unwrap();

    assert_eq!(smoke.after_pickup.wood, 15);
    assert_eq!(smoke.captured_mine_id, "mine:west-crystal");
    assert_eq!(smoke.mine_income_started_turn, 3);
    assert_eq!(smoke.captured_objective_id, "objective:north");
    assert_eq!(smoke.central_objectives_owned, 1);
    assert_eq!(smoke.object_commands, 3);
}
