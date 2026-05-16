use std::collections::BTreeMap;

use super::bitset::{empty_visibility_blob, set_visibility_bit};
use super::occupancy::chunk_coord;
use super::types::{
    FirstPlayableMapState, MAP_FLAG_BLOCKING_TERRAIN, MAP_FLAG_PASSABLE, MAP_FLAG_ROAD,
    MapChunkRecord, MapError, MapSubjectRecord, ParticipantKnownObjectRecord,
    VisibilityChunkRecord, WorldObjectRecord,
};
use super::viewport::local_index;
use crate::content::{
    FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH,
    FirstPlayableScenario, MapObjectContent, ObjectSeed, ResourcePileSeed, TerrainContent,
    TileCoord, first_playable_content_manifest, first_playable_scenario,
};
use crate::fixtures::{FixtureIds, first_playable_fixture};

#[must_use]
pub fn build_first_playable_map_state() -> FirstPlayableMapState {
    let fixture = first_playable_fixture();
    build_first_playable_map_state_for_ids(&fixture.ids)
        .expect("canonical first playable map should be valid")
}

pub fn build_first_playable_map_state_for_ids(
    ids: &FixtureIds,
) -> Result<FirstPlayableMapState, MapError> {
    let manifest = first_playable_content_manifest();
    let scenario = first_playable_scenario();
    let participant_ids = vec![
        ids.participant_one_id.clone(),
        ids.participant_two_id.clone(),
    ];
    let terrain_grid = terrain_grid(&scenario);
    let chunks = encode_chunks(&ids.session_id, &manifest.terrain, &terrain_grid);
    let visibility_chunks = visibility_chunks(&ids.session_id, &participant_ids, &scenario);

    let mut state = FirstPlayableMapState {
        session_id: ids.session_id.clone(),
        participant_ids,
        chunks,
        visibility_chunks,
        occupancy_rows: Vec::new(),
        world_objects: Vec::new(),
        known_objects: Vec::new(),
        subjects: Vec::new(),
    };

    seed_starting_subjects(&mut state, &scenario)?;
    seed_world_objects(&mut state, &scenario, &manifest.map_objects)?;
    seed_known_objects(&mut state);
    Ok(state)
}

fn terrain_grid(scenario: &FirstPlayableScenario) -> Vec<String> {
    let width = usize::from(scenario.map.width);
    let height = usize::from(scenario.map.height);
    let mut grid = vec![scenario.map.default_terrain_key.clone(); width * height];

    for patch in &scenario.map.terrain_patches {
        for y in patch.y..patch.y.saturating_add(patch.height) {
            for x in patch.x..patch.x.saturating_add(patch.width) {
                if x < scenario.map.width && y < scenario.map.height {
                    grid[grid_index(x, y, scenario.map.width)] = patch.terrain_key.clone();
                }
            }
        }
    }

    for road in &scenario.map.road_paths {
        for coord in expanded_road(&road.waypoints) {
            if coord.x < scenario.map.width && coord.y < scenario.map.height {
                grid[grid_index(coord.x, coord.y, scenario.map.width)] = "road".to_string();
            }
        }
    }

    grid
}

fn expanded_road(waypoints: &[TileCoord]) -> Vec<TileCoord> {
    let mut coords = Vec::new();
    for window in waypoints.windows(2) {
        let mut x = window[0].x;
        let mut y = window[0].y;
        let target_x = window[1].x;
        let target_y = window[1].y;
        coords.push(TileCoord { x, y });
        while x != target_x {
            x = step_toward(x, target_x);
            coords.push(TileCoord { x, y });
        }
        while y != target_y {
            y = step_toward(y, target_y);
            coords.push(TileCoord { x, y });
        }
    }
    if let Some(last) = waypoints.last() {
        coords.push(last.clone());
    }
    coords
}

fn step_toward(value: u16, target: u16) -> u16 {
    match value.cmp(&target) {
        std::cmp::Ordering::Less => value + 1,
        std::cmp::Ordering::Equal => value,
        std::cmp::Ordering::Greater => value - 1,
    }
}

fn encode_chunks(
    session_id: &str,
    terrain: &[TerrainContent],
    terrain_grid: &[String],
) -> Vec<MapChunkRecord> {
    let mut terrain_by_key = BTreeMap::new();
    for item in terrain {
        terrain_by_key.insert(item.terrain_key.as_str(), item);
    }

    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let chunks_x = FIRST_PLAYABLE_MAP_WIDTH.div_ceil(chunk_size);
    let chunks_y = FIRST_PLAYABLE_MAP_HEIGHT.div_ceil(chunk_size);
    let mut chunks = Vec::with_capacity(usize::from(chunks_x) * usize::from(chunks_y));

    for chunk_y in 0..chunks_y {
        for chunk_x in 0..chunks_x {
            let origin_x = chunk_x * chunk_size;
            let origin_y = chunk_y * chunk_size;
            let width = (FIRST_PLAYABLE_MAP_WIDTH - origin_x).min(chunk_size);
            let height = (FIRST_PLAYABLE_MAP_HEIGHT - origin_y).min(chunk_size);
            let cell_count = usize::from(width) * usize::from(height);
            let mut terrain_blob = Vec::with_capacity(cell_count);
            let mut movement_blob = Vec::with_capacity(cell_count);
            let mut flags_blob = Vec::with_capacity(cell_count);

            for local_y in 0..height {
                for local_x in 0..width {
                    let x = origin_x + local_x;
                    let y = origin_y + local_y;
                    let terrain_key = &terrain_grid[grid_index(x, y, FIRST_PLAYABLE_MAP_WIDTH)];
                    let terrain = terrain_by_key
                        .get(terrain_key.as_str())
                        .expect("terrain grid should use manifest terrain keys");
                    terrain_blob.push(terrain.terrain_code);
                    movement_blob.push(terrain.movement_cost.min(u16::from(u8::MAX)) as u8);
                    flags_blob.push(terrain_flags(terrain));
                }
            }

            chunks.push(MapChunkRecord {
                chunk_id: format!("chunk:{session_id}:{chunk_x}:{chunk_y}"),
                session_id: session_id.to_string(),
                chunk_x,
                chunk_y,
                width,
                height,
                terrain_blob,
                movement_blob,
                flags_blob,
            });
        }
    }

    chunks
}

fn terrain_flags(terrain: &TerrainContent) -> u8 {
    let mut flags = 0;
    if terrain.passable {
        flags |= MAP_FLAG_PASSABLE;
    } else {
        flags |= MAP_FLAG_BLOCKING_TERRAIN;
    }
    if terrain.terrain_key == "road" {
        flags |= MAP_FLAG_ROAD;
    }
    flags
}

fn visibility_chunks(
    session_id: &str,
    participant_ids: &[String],
    scenario: &FirstPlayableScenario,
) -> Vec<VisibilityChunkRecord> {
    let mut sources: BTreeMap<&str, Vec<(u16, u16, u8)>> = BTreeMap::new();
    for start in &scenario.starts {
        let participant_id = &participant_ids[usize::from(start.slot_index)];
        sources.entry(participant_id.as_str()).or_default().extend([
            (
                start.town_x,
                start.town_y,
                scenario.starting_state.champion_vision,
            ),
            (
                start.champion_x,
                start.champion_y,
                scenario.starting_state.champion_vision,
            ),
        ]);
    }

    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let chunks_x = FIRST_PLAYABLE_MAP_WIDTH.div_ceil(chunk_size);
    let chunks_y = FIRST_PLAYABLE_MAP_HEIGHT.div_ceil(chunk_size);
    let mut rows = Vec::with_capacity(participant_ids.len() * usize::from(chunks_x * chunks_y));

    for participant_id in participant_ids {
        let participant_sources = sources
            .get(participant_id.as_str())
            .expect("every participant should have setup vision sources");
        for chunk_y in 0..chunks_y {
            for chunk_x in 0..chunks_x {
                let origin_x = chunk_x * chunk_size;
                let origin_y = chunk_y * chunk_size;
                let width = (FIRST_PLAYABLE_MAP_WIDTH - origin_x).min(chunk_size);
                let height = (FIRST_PLAYABLE_MAP_HEIGHT - origin_y).min(chunk_size);
                let mut visible_blob = empty_visibility_blob(width, height);
                let mut discovered_blob = empty_visibility_blob(width, height);
                for local_y in 0..height {
                    for local_x in 0..width {
                        let x = origin_x + local_x;
                        let y = origin_y + local_y;
                        if tile_visible_from_sources(x, y, participant_sources) {
                            let index = local_index(local_x, local_y, width)
                                .expect("chunk local coordinates are in bounds");
                            set_visibility_bit(&mut visible_blob, index);
                            set_visibility_bit(&mut discovered_blob, index);
                        }
                    }
                }
                rows.push(VisibilityChunkRecord {
                    visibility_chunk_id: format!(
                        "vis:{session_id}:{participant_id}:{chunk_x}:{chunk_y}"
                    ),
                    session_id: session_id.to_string(),
                    participant_id: participant_id.clone(),
                    chunk_x,
                    chunk_y,
                    width,
                    height,
                    discovered_blob,
                    visible_blob,
                    visible_turn: 1,
                });
            }
        }
    }

    rows
}

fn tile_visible_from_sources(x: u16, y: u16, sources: &[(u16, u16, u8)]) -> bool {
    sources.iter().any(|(source_x, source_y, radius)| {
        x.abs_diff(*source_x).max(y.abs_diff(*source_y)) <= u16::from(*radius)
    })
}

fn seed_starting_subjects(
    state: &mut FirstPlayableMapState,
    scenario: &FirstPlayableScenario,
) -> Result<(), MapError> {
    for start in &scenario.starts {
        let participant_id = state.participant_ids[usize::from(start.slot_index)].clone();
        state.subjects.push(MapSubjectRecord {
            subject_kind: "town".to_string(),
            subject_id_text: start.town_key.clone(),
            display_name: start.town_name.clone(),
            asset_key: Some(format!("sprite:town:{}", start.faction_slug)),
            owner_participant_id: Some(participant_id.clone()),
            object_slug: None,
            object_type: Some("town".to_string()),
            scoring_kind: None,
            x: start.town_x,
            y: start.town_y,
            chunk_x: chunk_coord(start.town_x),
            chunk_y: chunk_coord(start.town_y),
            state: "active".to_string(),
            public_json: format!(
                "{{\"type\":\"town\",\"town_id\":\"{}\",\"faction_id\":\"{}\",\"status\":\"active\",\"garrison_strength_label\":\"starting\"}}",
                escape_json(&start.town_key),
                escape_json(&start.faction_slug)
            ),
            redacted_json: format!(
                "{{\"type\":\"town\",\"town_id\":\"{}\",\"status\":\"last_known\"}}",
                escape_json(&start.town_key)
            ),
        });
        state.insert_occupancy_footprint(
            start.town_x,
            start.town_y,
            1,
            1,
            "town",
            "town",
            &start.town_key,
            true,
            None,
        )?;

        state.subjects.push(MapSubjectRecord {
            subject_kind: "champion".to_string(),
            subject_id_text: start.champion_key.clone(),
            display_name: start.champion_name.clone(),
            asset_key: Some(format!("sprite:champion:{}", start.champion_class_slug)),
            owner_participant_id: Some(participant_id),
            object_slug: None,
            object_type: Some("champion".to_string()),
            scoring_kind: None,
            x: start.champion_x,
            y: start.champion_y,
            chunk_x: chunk_coord(start.champion_x),
            chunk_y: chunk_coord(start.champion_y),
            state: "active".to_string(),
            public_json: format!(
                "{{\"type\":\"champion\",\"champion_id\":\"{}\",\"class_key\":\"{}\",\"status\":\"active\",\"strength_label\":\"starting\"}}",
                escape_json(&start.champion_key),
                escape_json(&start.champion_class_slug)
            ),
            redacted_json: format!(
                "{{\"type\":\"champion\",\"champion_id\":\"{}\",\"status\":\"last_known\"}}",
                escape_json(&start.champion_key)
            ),
        });
        state.insert_occupancy_footprint(
            start.champion_x,
            start.champion_y,
            1,
            1,
            "champion",
            "champion",
            &start.champion_key,
            true,
            None,
        )?;
    }
    Ok(())
}

fn seed_world_objects(
    state: &mut FirstPlayableMapState,
    scenario: &FirstPlayableScenario,
    map_objects: &[MapObjectContent],
) -> Result<(), MapError> {
    for object in &scenario.mines {
        seed_object_record(state, object, map_objects, "mine")?;
    }
    for object in &scenario.external_dwellings {
        seed_object_record(state, object, map_objects, "external_dwelling")?;
    }
    for object in &scenario.central_objectives {
        seed_object_record(state, object, map_objects, "central_objective")?;
    }
    for pile in &scenario.resource_piles {
        seed_resource_pile_record(state, pile, map_objects)?;
    }
    for neutral in &scenario.neutral_armies {
        state.subjects.push(MapSubjectRecord {
            subject_kind: "neutral_army".to_string(),
            subject_id_text: neutral.key.clone(),
            display_name: strength_display_name(&neutral.strength_band),
            asset_key: Some("sprite:unit:broken-pike".to_string()),
            owner_participant_id: None,
            object_slug: None,
            object_type: Some("neutral_army".to_string()),
            scoring_kind: None,
            x: neutral.x,
            y: neutral.y,
            chunk_x: chunk_coord(neutral.x),
            chunk_y: chunk_coord(neutral.y),
            state: "guarding".to_string(),
            public_json: format!(
                "{{\"type\":\"neutral_army\",\"army_id\":\"{}\",\"strength_label\":\"{}\",\"stack_count\":{}}}",
                escape_json(&neutral.key),
                escape_json(&neutral.strength_band),
                neutral.stacks.len()
            ),
            redacted_json: format!(
                "{{\"type\":\"neutral_army\",\"army_id\":\"{}\",\"strength_label\":\"{}\"}}",
                escape_json(&neutral.key),
                escape_json(&neutral.strength_band)
            ),
        });
        state.insert_occupancy_footprint(
            neutral.x,
            neutral.y,
            1,
            1,
            "army",
            "neutral_army",
            &neutral.key,
            true,
            None,
        )?;
    }
    Ok(())
}

fn seed_object_record(
    state: &mut FirstPlayableMapState,
    object: &ObjectSeed,
    map_objects: &[MapObjectContent],
    scoring_kind: &str,
) -> Result<(), MapError> {
    let definition = map_objects
        .iter()
        .find(|definition| definition.slug == object.object_slug)
        .expect("scenario object slug should exist in manifest");
    let owner = object
        .owner_slot_index
        .map(|slot| state.participant_ids[usize::from(slot)].clone());
    let record = make_world_object_record(
        &state.session_id,
        &object.key,
        definition,
        Some(scoring_kind.to_string()),
        owner,
        object.guard_neutral_army_key.clone(),
        object.x,
        object.y,
    );
    state
        .subjects
        .push(world_object_subject(&record, definition));
    state.world_objects.push(record);
    state.insert_occupancy_footprint(
        object.x,
        object.y,
        u16::from(definition.footprint_w),
        u16::from(definition.footprint_h),
        "object",
        "world_object",
        &object.key,
        definition.blocking,
        None,
    )?;
    Ok(())
}

fn seed_resource_pile_record(
    state: &mut FirstPlayableMapState,
    pile: &ResourcePileSeed,
    map_objects: &[MapObjectContent],
) -> Result<(), MapError> {
    let definition = map_objects
        .iter()
        .find(|definition| definition.slug == pile.object_slug)
        .expect("scenario resource pile slug should exist in manifest");
    let record = make_world_object_record(
        &state.session_id,
        &pile.key,
        definition,
        Some("resource_pile".to_string()),
        None,
        None,
        pile.x,
        pile.y,
    );
    state
        .subjects
        .push(world_object_subject(&record, definition));
    state.world_objects.push(record);
    state.insert_occupancy_footprint(
        pile.x,
        pile.y,
        u16::from(definition.footprint_w),
        u16::from(definition.footprint_h),
        "pickup",
        "world_object",
        &pile.key,
        definition.blocking,
        None,
    )?;
    Ok(())
}

fn make_world_object_record(
    session_id: &str,
    object_id: &str,
    definition: &MapObjectContent,
    scoring_kind: Option<String>,
    owner_participant_id: Option<String>,
    guarded_neutral_army_id: Option<String>,
    x: u16,
    y: u16,
) -> WorldObjectRecord {
    let public_json = format!(
        "{{\"type\":\"world_object\",\"object_id\":\"{}\",\"object_type\":\"{}\",\"scoring_kind\":\"{}\",\"interaction_key\":\"{}\",\"state\":\"available\"}}",
        escape_json(object_id),
        escape_json(&definition.object_type),
        escape_json(scoring_kind.as_deref().unwrap_or("none")),
        escape_json(&definition.interaction_key)
    );
    let redacted_json = format!(
        "{{\"type\":\"world_object\",\"object_id\":\"{}\",\"object_type\":\"{}\",\"state\":\"last_known\"}}",
        escape_json(object_id),
        escape_json(&definition.object_type)
    );

    WorldObjectRecord {
        object_id: object_id.to_string(),
        session_id: session_id.to_string(),
        object_slug: definition.slug.clone(),
        object_type: definition.object_type.clone(),
        scoring_kind,
        owner_participant_id,
        guarded_neutral_army_id,
        x,
        y,
        chunk_x: chunk_coord(x),
        chunk_y: chunk_coord(y),
        state: "available".to_string(),
        public_json,
        redacted_json,
    }
}

fn world_object_subject(
    record: &WorldObjectRecord,
    definition: &MapObjectContent,
) -> MapSubjectRecord {
    MapSubjectRecord {
        subject_kind: "world_object".to_string(),
        subject_id_text: record.object_id.clone(),
        display_name: definition.name.clone(),
        asset_key: definition.sprite_key.clone(),
        owner_participant_id: record.owner_participant_id.clone(),
        object_slug: Some(record.object_slug.clone()),
        object_type: Some(record.object_type.clone()),
        scoring_kind: record.scoring_kind.clone(),
        x: record.x,
        y: record.y,
        chunk_x: record.chunk_x,
        chunk_y: record.chunk_y,
        state: record.state.clone(),
        public_json: record.public_json.clone(),
        redacted_json: record.redacted_json.clone(),
    }
}

fn seed_known_objects(state: &mut FirstPlayableMapState) {
    for participant_id in &state.participant_ids {
        for subject in &state.subjects {
            if state.is_visible_at(participant_id, subject.x, subject.y) {
                state.known_objects.push(ParticipantKnownObjectRecord {
                    participant_id: participant_id.clone(),
                    subject_kind: subject.subject_kind.clone(),
                    subject_id_text: subject.subject_id_text.clone(),
                    x: subject.x,
                    y: subject.y,
                    chunk_x: subject.chunk_x,
                    chunk_y: subject.chunk_y,
                    visibility: "visible".to_string(),
                    last_seen_turn: 1,
                    redacted_json: subject.redacted_json.clone(),
                });
            }
        }
    }
}

fn grid_index(x: u16, y: u16, width: u16) -> usize {
    usize::from(y) * usize::from(width) + usize::from(x)
}

fn strength_display_name(strength_band: &str) -> String {
    format!("Neutral Guard ({strength_band})")
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
