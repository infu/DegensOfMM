use crate::champion::{ChampionState, build_first_playable_champion_state};
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::{FirstPlayableMapState, build_first_playable_map_state, set_visibility_bit};

use super::preview::preview_move_path;
use super::smoke::run_first_playable_movement_smoke;
use super::submit::submit_move_intent;
use super::sync::{sync_session_turn, sync_session_turn_with_trap_after_microsteps};
use super::types::{
    MoveCoord, MovementError, MovementState, MovementSyncBudget,
    build_first_playable_movement_state,
};

fn fixture_states() -> (
    MovementState,
    FirstPlayableMapState,
    ChampionState,
    String,
    String,
) {
    let fixture = first_playable_fixture();
    (
        build_first_playable_movement_state(),
        build_first_playable_map_state(),
        build_first_playable_champion_state(),
        fixture.ids.participant_one_id,
        fixture.ids.participant_two_id,
    )
}

fn place_champion(
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    champion_id: &str,
    participant_id: &str,
    x: u16,
    y: u16,
) {
    {
        let champion = champions.champion_mut(champion_id).unwrap();
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
        Some("test:place".to_string()),
    )
    .unwrap();
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.owner_participant_id = Some(participant_id.to_string());
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

fn submit_west_path(
    movement: &mut MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    path: Vec<MoveCoord>,
    nonce: u64,
) {
    submit_move_intent(
        movement,
        map,
        champions,
        participant_id,
        "champion:west",
        path,
        nonce,
        1_000,
    )
    .unwrap();
}

#[test]
fn preview_validates_without_writes() {
    let (movement, map, champions, west, _) = fixture_states();
    let preview = preview_move_path(
        &movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        1_000,
    )
    .unwrap();

    assert_eq!(preview.path.last().copied(), Some(MoveCoord::new(9, 23)));
    assert_eq!(
        preview.stop.as_ref().unwrap().subject_id_text,
        "pile:west-wood-1"
    );
    assert_eq!(movement.intents.len(), 0);
    assert_eq!(champions.champion("champion:west").unwrap().x, 8);
}

#[test]
fn submit_replaces_pending_intent_and_rejects_nonce_mismatch() {
    let (mut movement, map, champions, west, _) = fixture_states();
    let first = submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24)],
        77,
        1_000,
    )
    .unwrap();
    let duplicate = submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24)],
        77,
        1_001,
    )
    .unwrap();
    let replaced = submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(10, 24)],
        78,
        1_002,
    )
    .unwrap();
    let mismatch = submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        77,
        1_003,
    );

    assert_eq!(first.intent.intent_id, duplicate.intent.intent_id);
    assert_eq!(replaced.replaced_intent_ids, vec![first.intent.intent_id]);
    assert!(matches!(
        mismatch,
        Err(MovementError::DuplicateNoncePayloadMismatch { .. })
    ));
    assert_eq!(movement.pending_intent_count_for_turn(1), 1);
}

#[test]
fn validation_rejects_hidden_tiles_and_impassable_terrain() {
    let (movement, mut map, mut champions, west, _) = fixture_states();
    let hidden = preview_move_path(
        &movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(13, 24),
            MoveCoord::new(14, 24),
        ],
        1_000,
    );
    assert!(matches!(hidden, Err(MovementError::HiddenTile { .. })));

    place_champion(&mut map, &mut champions, "champion:west", &west, 0, 1);
    reveal_tile(&mut map, &west, 0, 1);
    reveal_tile(&mut map, &west, 0, 0);
    let impassable = preview_move_path(
        &movement,
        &map,
        &champions,
        &west,
        "champion:west",
        vec![MoveCoord::new(0, 0)],
        1_000,
    );
    assert!(matches!(
        impassable,
        Err(MovementError::ImpassableTerrain { .. })
    ));
}

#[test]
fn hidden_dynamic_blocker_is_resolved_at_turn_finalization() {
    let (mut movement, mut map, mut champions, west, east) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:east", &east, 9, 24);
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24)],
        101,
    );

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert_eq!(outcome.battle_starts.len(), 1);
    assert_eq!(outcome.snapshots[0].outcome, "started_champion_battle");
    assert_eq!(champions.champion("champion:west").unwrap().x, 8);
    assert_eq!(
        champions.champion("champion:west").unwrap().status,
        "in_battle"
    );
}

#[test]
fn simultaneous_tile_conflict_has_deterministic_winner() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:east", &west, 10, 24);
    champions
        .champion_mut("champion:east")
        .unwrap()
        .movement_remaining = 200;
    champions
        .champion_mut("champion:west")
        .unwrap()
        .movement_remaining = 240;
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24)],
        201,
    );
    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:east",
        vec![MoveCoord::new(9, 24)],
        202,
        1_000,
    )
    .unwrap();

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert_eq!(outcome.snapshots.len(), 2);
    assert_eq!(champions.champion("champion:west").unwrap().x, 9);
    assert_eq!(champions.champion("champion:east").unwrap().x, 10);
    assert!(
        outcome
            .snapshots
            .iter()
            .any(|snapshot| snapshot.outcome == "stopped_tile_conflict")
    );
}

#[test]
fn crossing_conflict_stops_both_movers() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    place_champion(&mut map, &mut champions, "champion:east", &west, 9, 24);
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24)],
        301,
    );
    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &west,
        "champion:east",
        vec![MoveCoord::new(8, 24)],
        302,
        1_000,
    )
    .unwrap();

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert_eq!(outcome.battle_starts.len(), 0);
    assert_eq!(champions.champion("champion:west").unwrap().x, 8);
    assert_eq!(champions.champion("champion:east").unwrap().x, 9);
    assert_eq!(
        outcome
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.outcome == "stopped_crossing_conflict")
            .count(),
        2
    );
}

#[test]
fn object_interaction_moves_onto_object_tile_and_stops() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        401,
    );

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert_eq!(outcome.object_stops.len(), 1);
    assert_eq!(outcome.object_stops[0].object_id, "pile:west-wood-1");
    assert_eq!(champions.champion("champion:west").unwrap().x, 9);
    assert_eq!(champions.champion("champion:west").unwrap().y, 23);
    assert_eq!(movement.current_turn, 2);
}

#[test]
fn partial_sync_uses_cursor_and_resumes_without_duplicate_snapshots() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
        ],
        501,
    );
    let limited = MovementSyncBudget {
        max_microsteps: 1,
        ..MovementSyncBudget::default()
    };

    let first = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        limited,
    )
    .unwrap();
    let second = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert!(first.budget_exhausted);
    assert_eq!(first.cursor.unwrap().next_step_index, 1);
    assert!(second.advanced_turn);
    assert_eq!(movement.snapshots.len(), 3);
    assert_eq!(champions.champion("champion:west").unwrap().x, 11);
}

#[test]
fn exhausted_budget_can_park_without_writes() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24)],
        601,
    );
    let exhausted = MovementSyncBudget {
        max_microsteps: 0,
        ..MovementSyncBudget::default()
    };

    let outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        exhausted,
    )
    .unwrap();

    assert!(outcome.budget_exhausted);
    assert_eq!(movement.snapshots.len(), 0);
    assert_eq!(movement.current_turn, 1);
    assert_eq!(movement.partial_cursor.unwrap().next_step_index, 0);
}

#[test]
fn recovery_after_trap_continues_applying_system_command() {
    let (mut movement, mut map, mut champions, west, _) = fixture_states();
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24), MoveCoord::new(10, 24)],
        701,
    );

    let trapped = sync_session_turn_with_trap_after_microsteps(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
        1,
    );
    assert!(matches!(
        trapped,
        Err(MovementError::SimulatedTrapAfterPartialApply { .. })
    ));
    assert_eq!(movement.snapshots.len(), 1);
    assert_eq!(movement.partial_cursor.as_ref().unwrap().next_step_index, 1);

    let recovered = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();

    assert!(recovered.recovery_checked);
    assert!(recovered.advanced_turn);
    assert_eq!(movement.snapshots.len(), 2);
    assert_eq!(movement.system_commands[0].status, "applied");
    assert_eq!(movement.system_commands[0].last_error, None);
}

#[test]
fn time_view_is_query_only_and_does_not_finalize_movement() {
    let (mut movement, map, champions, west, _) = fixture_states();
    submit_west_path(
        &mut movement,
        &map,
        &champions,
        &west,
        vec![MoveCoord::new(9, 24)],
        801,
    );

    let view = movement.time_view(TURN_DURATION_MS);

    assert!(view.sync_required);
    assert_eq!(movement.current_turn, 1);
    assert_eq!(movement.snapshots.len(), 0);
}

#[test]
fn first_playable_movement_smoke_crosses_two_turn_windows() {
    let smoke = run_first_playable_movement_smoke().unwrap();

    assert_eq!(smoke.final_turn, 3);
    assert_eq!(smoke.final_x, 10);
    assert_eq!(smoke.final_y, 24);
    assert_eq!(smoke.snapshots, 2);
    assert_eq!(smoke.system_commands, 2);
}
