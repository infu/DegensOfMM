use candid::{Decode, Encode};

use super::{
    FirstPlayableMapState, MAP_FLAG_BLOCKING_TERRAIN, MAP_FLAG_PASSABLE, MAP_FLAG_ROAD, MapError,
    ObjectViewPage, SubjectViewResult, Viewport, build_first_playable_map_state,
    empty_visibility_blob, read_visibility_bit, set_visibility_bit,
};
use crate::content::{FIRST_PLAYABLE_CHUNK_SIZE, first_playable_content_manifest};
use crate::fixtures::first_playable_fixture;

#[test]
fn first_playable_chunks_are_row_major_and_match_terrain_costs() {
    let state = build_first_playable_map_state();
    let manifest = first_playable_content_manifest();

    assert_eq!(state.chunks.len(), 9);
    assert!(state.chunks.iter().all(|chunk| chunk.width == 16));
    assert!(state.chunks.iter().all(|chunk| chunk.height == 16));
    assert!(
        state
            .chunks
            .iter()
            .all(|chunk| chunk.terrain_blob.len() == 256)
    );
    assert!(
        state
            .chunks
            .iter()
            .all(|chunk| chunk.movement_blob.len() == 256)
    );
    assert!(
        state
            .chunks
            .iter()
            .all(|chunk| chunk.flags_blob.len() == 256)
    );

    assert_eq!(
        state.terrain_code_at(6, 24),
        Some(manifest.terrain("road").unwrap().terrain_code)
    );
    assert_eq!(state.movement_cost_at(6, 24), Some(5));
    assert_eq!(
        state.flags_at(6, 24).unwrap() & (MAP_FLAG_PASSABLE | MAP_FLAG_ROAD),
        MAP_FLAG_PASSABLE | MAP_FLAG_ROAD
    );

    assert_eq!(
        state.terrain_code_at(3, 6),
        Some(manifest.terrain("forest").unwrap().terrain_code)
    );
    assert_eq!(state.movement_cost_at(3, 6), Some(15));
    assert_eq!(
        state.terrain_code_at(21, 19),
        Some(manifest.terrain("swamp").unwrap().terrain_code)
    );
    assert_eq!(state.movement_cost_at(21, 19), Some(20));
    assert_eq!(
        state.terrain_code_at(0, 0),
        Some(manifest.terrain("mountain").unwrap().terrain_code)
    );
    assert_eq!(state.movement_cost_at(0, 0), Some(255));
    assert_eq!(
        state.flags_at(0, 0).unwrap() & MAP_FLAG_BLOCKING_TERRAIN,
        MAP_FLAG_BLOCKING_TERRAIN
    );
}

#[test]
fn visibility_bitsets_use_little_endian_bits() {
    let mut blob = empty_visibility_blob(4, 4);
    set_visibility_bit(&mut blob, 0);
    set_visibility_bit(&mut blob, 7);
    set_visibility_bit(&mut blob, 8);

    assert_eq!(blob, vec![0b1000_0001, 0b0000_0001]);
    assert!(read_visibility_bit(&blob, 0));
    assert!(read_visibility_bit(&blob, 7));
    assert!(read_visibility_bit(&blob, 8));
    assert!(!read_visibility_bit(&blob, 9));
}

#[test]
fn opening_visibility_materializes_all_chunks_for_each_participant() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_map_state();

    assert_eq!(state.visibility_chunks.len(), 18);
    assert!(
        state
            .visibility_chunks
            .iter()
            .all(|chunk| chunk.visible_turn == 1)
    );
    assert!(
        state
            .visibility_chunks
            .iter()
            .all(|chunk| chunk.discovered_blob.len() == 32)
    );
    assert!(
        state
            .visibility_chunks
            .iter()
            .all(|chunk| chunk.visible_blob.len() == 32)
    );

    assert!(state.is_visible_at(&fixture.ids.participant_one_id, 8, 24));
    assert!(state.is_discovered_at(&fixture.ids.participant_one_id, 12, 22));
    assert!(!state.is_visible_at(&fixture.ids.participant_one_id, 39, 24));
    assert!(state.is_visible_at(&fixture.ids.participant_two_id, 39, 24));
    assert!(!state.is_visible_at(&fixture.ids.participant_two_id, 8, 24));
}

#[test]
fn viewport_reads_use_limits_cursors_and_public_dtos() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_map_state();
    let viewport = Viewport::new(0, 16, 24, 24);

    let first_page = state.map_chunk_views(&fixture.ids.participant_one_id, &viewport, None, 2);
    assert_eq!(first_page.chunks.len(), 2);
    assert!(first_page.has_more);
    assert_eq!(first_page.next_cursor, Some(2));
    assert_eq!(first_page.chunks[0].chunk_x, 0);
    assert_eq!(first_page.chunks[0].chunk_y, 1);
    assert_eq!(first_page.chunks[0].visible_blob.len(), 32);

    let second_page = state.map_chunk_views(
        &fixture.ids.participant_one_id,
        &viewport,
        first_page.next_cursor,
        8,
    );
    assert_eq!(second_page.chunks.len(), 2);
    assert!(!second_page.has_more);
    assert_eq!(second_page.next_cursor, None);

    let objects = state.object_views(&fixture.ids.participant_one_id, &viewport, None, 3);
    assert_eq!(objects.objects.len(), 3);
    assert!(objects.has_more);
    assert!(
        objects
            .objects
            .iter()
            .all(|object| object.visibility == "visible")
    );

    let encoded = Encode!(&first_page).expect("chunk DTOs should encode as candid");
    let decoded =
        Decode!(&encoded, super::MapChunkPage).expect("chunk DTOs should decode from candid");
    assert_eq!(decoded, first_page);
}

#[test]
fn occupancy_supports_single_and_multi_tile_rows_with_cleanup() {
    let mut state = build_first_playable_map_state();
    let baseline = state.occupancy_rows.len();

    let rows = state
        .insert_occupancy_footprint(
            4,
            34,
            2,
            2,
            "structure",
            "town",
            "town:test-multitile",
            true,
            Some("command:test".to_string()),
        )
        .expect("empty footprint should insert");

    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows.iter()
            .map(|row| row.occupant_cell_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(state.occupancy_rows.len(), baseline + 4);

    let removed = state.cleanup_occupancy_by_subject("town", "town:test-multitile");
    assert_eq!(removed, 4);
    assert_eq!(state.occupancy_rows.len(), baseline);
}

#[test]
fn occupancy_rejects_duplicate_tile_layers_and_occupant_cells() {
    let mut state = build_first_playable_map_state();

    let tile_collision = state
        .insert_occupancy_footprint(6, 24, 1, 1, "town", "marker", "marker:town", false, None)
        .expect_err("existing town layer should reject duplicate occupancy");
    assert!(matches!(
        tile_collision,
        MapError::OccupancyTileCollision { x: 6, y: 24, .. }
    ));

    state
        .insert_occupancy_footprint(4, 34, 1, 1, "marker", "marker", "marker:one", false, None)
        .expect("first marker should insert");
    let cell_collision = state
        .insert_occupancy_footprint(5, 34, 1, 1, "marker", "marker", "marker:one", false, None)
        .expect_err("same occupant cell index should reject duplicates");
    assert!(matches!(
        cell_collision,
        MapError::OccupancyCellCollision {
            occupant_cell_index: 0,
            ..
        }
    ));
}

#[test]
fn hidden_subjects_return_not_visible_without_payload_leaks() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_map_state();

    let hidden = state.subject_view(&fixture.ids.participant_one_id, "champion", "champion:east");
    assert!(matches!(
        hidden,
        SubjectViewResult::NotVisible {
            visibility,
            subject_id_text,
            ..
        } if visibility == "hidden" && subject_id_text == "champion:east"
    ));

    let visible = state.subject_view(&fixture.ids.participant_two_id, "champion", "champion:east");
    let SubjectViewResult::Visible(view) = visible else {
        panic!("own champion should be visible");
    };
    assert_eq!(view.display_name.as_deref(), Some("Korrin of Receipts"));
    assert!(
        view.details_json
            .contains("\"strength_label\":\"starting\"")
    );
}

#[test]
fn opening_viewport_snapshot_is_stable_and_candid_roundtrips() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_map_state();
    let snapshot = state.opening_viewport_snapshot(&fixture.ids.participant_one_id);

    assert_eq!(snapshot.viewport, Viewport::new(0, 16, 24, 24));
    assert_eq!(snapshot.chunks.len(), 4);
    assert!(!snapshot.objects.is_empty());
    assert_eq!(snapshot.snapshot_hash, snapshot.computed_snapshot_hash());
    assert_eq!(
        snapshot.snapshot_hash,
        "41df63f9bdd248984764b5c8e7bc8bcf65391f317fc0521096e2acfe118044ad"
    );

    let encoded = Encode!(&snapshot).expect("opening viewport should encode as candid");
    let decoded = Decode!(&encoded, super::OpeningViewportSnapshot)
        .expect("opening viewport should decode from candid");
    assert_eq!(decoded, snapshot);
}

#[test]
fn public_opening_view_does_not_expose_private_storage_rows() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_map_state();
    let snapshot = state.opening_viewport_snapshot(&fixture.ids.participant_one_id);

    let first_visible = snapshot
        .objects
        .iter()
        .find(|object| object.subject_id_text == "champion:west")
        .expect("own champion should render in opening viewport");
    assert_eq!(first_visible.visibility, "visible");
    assert_eq!(first_visible.redaction_level, "none");
    assert!(
        snapshot
            .objects
            .iter()
            .all(|object| object.subject_id_text != "champion:east")
    );

    let page = ObjectViewPage {
        objects: snapshot.objects,
        next_cursor: None,
        has_more: false,
    };
    let encoded = Encode!(&page).expect("object DTOs should encode as candid");
    let decoded = Decode!(&encoded, ObjectViewPage).expect("object DTOs should decode");
    assert_eq!(decoded.has_more, page.has_more);
}

#[test]
fn map_state_public_counts_match_setup_requirements() {
    let state: FirstPlayableMapState = build_first_playable_map_state();
    let fixture = first_playable_fixture();

    assert_eq!(state.session_id, fixture.ids.session_id);
    assert_eq!(state.chunks.len(), 9);
    assert_eq!(
        state.visibility_chunks.len(),
        state.participant_ids.len() * state.chunks.len()
    );
    assert_eq!(u16::from(FIRST_PLAYABLE_CHUNK_SIZE), 16);
    assert_eq!(state.world_objects.len(), 19);
    assert!(state.known_objects.len() >= 8);
}
