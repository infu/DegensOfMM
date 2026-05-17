//! Repository boundary for map chunks, terrain blobs, visibility, known objects, and occupancy.

use domm_degens_schema::schema::{
    GameParticipant, GameSession, MapChunk, MapOccupancy, ParticipantKnownObject,
    ParticipantObjectVisit, VisibilityChunk, WorldObject,
};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const MAP_CHUNK_COORD_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.chunk_by_session_coord",
    entity: "MapChunk",
    indexed_fields: &["session_id", "chunk_x", "chunk_y"],
    bounded_limit: Some(1),
};

pub(crate) const VISIBILITY_CHUNK_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.visibility_by_participant_chunk",
    entity: "VisibilityChunk",
    indexed_fields: &["participant_id", "chunk_x", "chunk_y"],
    bounded_limit: Some(1),
};

pub(crate) const OCCUPANCY_CELL_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.occupancy_by_cell_layer",
    entity: "MapOccupancy",
    indexed_fields: &["session_id", "x", "y", "layer"],
    bounded_limit: Some(1),
};

pub(crate) const OCCUPANCY_OCCUPANT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.occupancy_by_occupant",
    entity: "MapOccupancy",
    indexed_fields: &[
        "session_id",
        "occupant_kind",
        "occupant_id_text",
        "occupant_cell_index",
    ],
    bounded_limit: Some(1),
};

pub(crate) const OCCUPANCY_CHUNK_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.occupancy_by_chunk",
    entity: "MapOccupancy",
    indexed_fields: &["session_id", "chunk_x", "chunk_y"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const KNOWN_OBJECT_CHUNK_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.known_objects_by_chunk",
    entity: "ParticipantKnownObject",
    indexed_fields: &["session_id", "participant_id", "chunk_x", "chunk_y"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const KNOWN_OBJECT_SUBJECT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.known_object_by_subject",
    entity: "ParticipantKnownObject",
    indexed_fields: &["participant_id", "subject_kind", "subject_id_text"],
    bounded_limit: Some(1),
};

pub(crate) const WORLD_OBJECT_COORD_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.world_object_by_session_xy",
    entity: "WorldObject",
    indexed_fields: &["session_id", "x", "y"],
    bounded_limit: Some(1),
};

pub(crate) const WORLD_OBJECT_OWNER_SCORING_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "map.world_objects_by_owner_scoring",
    entity: "WorldObject",
    indexed_fields: &[
        "session_id",
        "scoring_kind",
        "owner_participant_id",
        "state",
    ],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn create_map_chunk(
    session_id: Id<GameSession>,
    chunk_x: u16,
    chunk_y: u16,
    width: u8,
    height: u8,
    terrain_blob: Vec<u8>,
    movement_blob: Vec<u8>,
    flags_blob: Vec<u8>,
) -> RepoResult<MapChunk> {
    let input: Create<MapChunk> = Create::<MapChunk> {
        session_id: Some(session_id.key()),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        width: Some(width),
        height: Some(height),
        terrain_blob: Some(terrain_blob.into()),
        movement_blob: Some(movement_blob.into()),
        flags_blob: Some(flags_blob.into()),
    };

    foundation::create("map.create_map_chunk", input)
}

pub(crate) fn find_map_chunk(
    session_id: Id<GameSession>,
    chunk_x: u16,
    chunk_y: u16,
) -> RepoResult<Option<MapChunk>> {
    foundation::storage_result(
        MAP_CHUNK_COORD_LOOKUP.name,
        crate::db()
            .load::<MapChunk>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_map_chunks_by_session(
    session_id: Id<GameSession>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<MapChunk>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "map.chunks_by_session",
        crate::db()
            .load::<MapChunk>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("chunk_y")
            .order_asc("chunk_x")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_visibility_chunk(
    participant_id: Id<GameParticipant>,
    chunk_x: u16,
    chunk_y: u16,
) -> RepoResult<Option<VisibilityChunk>> {
    foundation::storage_result(
        VISIBILITY_CHUNK_LOOKUP.name,
        crate::db()
            .load::<VisibilityChunk>()
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_visibility_chunks_by_participant(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<VisibilityChunk>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "map.visibility_by_participant",
        crate::db()
            .load::<VisibilityChunk>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .order_asc("chunk_y")
            .order_asc("chunk_x")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn create_visibility_chunk(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    chunk_x: u16,
    chunk_y: u16,
    discovered_blob: Vec<u8>,
    visible_blob: Vec<u8>,
    visible_turn: u32,
) -> RepoResult<VisibilityChunk> {
    let input: Create<VisibilityChunk> = Create::<VisibilityChunk> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        discovered_blob: Some(discovered_blob.into()),
        visible_blob: Some(visible_blob.into()),
        visible_turn: Some(visible_turn),
    };

    foundation::create("map.create_visibility_chunk", input)
}

pub(crate) fn update_visibility_chunk(row: VisibilityChunk) -> RepoResult<VisibilityChunk> {
    foundation::update("map.update_visibility_chunk", row)
}

pub(crate) fn find_occupancy_cell(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
    layer: &str,
) -> RepoResult<Option<MapOccupancy>> {
    foundation::storage_result(
        OCCUPANCY_CELL_LOOKUP.name,
        crate::db()
            .load::<MapOccupancy>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("x").eq(x))
            .filter(FieldRef::new("y").eq(y))
            .filter(FieldRef::new("layer").eq(layer))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn find_occupancy_by_occupant(
    session_id: Id<GameSession>,
    occupant_kind: &str,
    occupant_id_text: &str,
    occupant_cell_index: u8,
) -> RepoResult<Option<MapOccupancy>> {
    foundation::storage_result(
        OCCUPANCY_OCCUPANT_LOOKUP.name,
        crate::db()
            .load::<MapOccupancy>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("occupant_kind").eq(occupant_kind))
            .filter(FieldRef::new("occupant_id_text").eq(occupant_id_text))
            .filter(FieldRef::new("occupant_cell_index").eq(occupant_cell_index))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_occupancy_cell(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    layer: String,
    occupant_kind: String,
    occupant_id_text: String,
    occupant_cell_index: u8,
    blocking: bool,
) -> RepoResult<MapOccupancy> {
    let input: Create<MapOccupancy> = Create::<MapOccupancy> {
        session_id: Some(session_id.key()),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        layer: Some(layer),
        occupant_kind: Some(occupant_kind),
        occupant_id_text: Some(occupant_id_text),
        occupant_cell_index: Some(occupant_cell_index),
        blocking: Some(blocking),
        last_command_id: Some(None),
    };

    foundation::create("map.create_occupancy_cell", input)
}

pub(crate) fn update_occupancy_cell(row: MapOccupancy) -> RepoResult<MapOccupancy> {
    foundation::update("map.update_occupancy_cell", row)
}

pub(crate) fn page_occupancy_for_chunk(
    session_id: Id<GameSession>,
    chunk_x: u16,
    chunk_y: u16,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<MapOccupancy>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        OCCUPANCY_CHUNK_LOOKUP.name,
        crate::db()
            .load::<MapOccupancy>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("layer")
            .order_asc("x")
            .order_asc("y")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_known_objects_for_chunk(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    chunk_x: u16,
    chunk_y: u16,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ParticipantKnownObject>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        KNOWN_OBJECT_CHUNK_LOOKUP.name,
        crate::db()
            .load::<ParticipantKnownObject>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("subject_kind")
            .order_asc("subject_id_text")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_known_objects_for_participant(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ParticipantKnownObject>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "map.known_objects_by_participant",
        crate::db()
            .load::<ParticipantKnownObject>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .order_asc("chunk_y")
            .order_asc("chunk_x")
            .order_asc("subject_kind")
            .order_asc("subject_id_text")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_known_object(
    participant_id: Id<GameParticipant>,
    subject_kind: &str,
    subject_id_text: &str,
) -> RepoResult<Option<ParticipantKnownObject>> {
    foundation::storage_result(
        KNOWN_OBJECT_SUBJECT_LOOKUP.name,
        crate::db()
            .load::<ParticipantKnownObject>()
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("subject_kind").eq(subject_kind))
            .filter(FieldRef::new("subject_id_text").eq(subject_id_text))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_known_object(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    subject_kind: String,
    subject_id_text: String,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    visibility: String,
    last_seen_turn: u32,
    redacted_json: Option<String>,
) -> RepoResult<ParticipantKnownObject> {
    let input: Create<ParticipantKnownObject> = Create::<ParticipantKnownObject> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        subject_kind: Some(subject_kind),
        subject_id_text: Some(subject_id_text),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        visibility: Some(visibility),
        last_seen_turn: Some(last_seen_turn),
        redacted_json: Some(redacted_json),
    };

    foundation::create("map.create_known_object", input)
}

pub(crate) fn update_known_object(
    object: ParticipantKnownObject,
) -> RepoResult<ParticipantKnownObject> {
    foundation::update("map.update_known_object", object)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_world_object(
    session_id: Id<GameSession>,
    object_def_id: Id<domm_degens_schema::schema::MapObjectDefinition>,
    owner_participant_id: Option<Id<GameParticipant>>,
    guarded_neutral_army_id: Option<Id<domm_degens_schema::schema::NeutralArmy>>,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    state: String,
    scoring_kind: String,
    last_visited_turn: u32,
    captured_turn: u32,
    income_started_turn: u32,
    instance_json: Option<String>,
) -> RepoResult<WorldObject> {
    let input: Create<WorldObject> = Create::<WorldObject> {
        session_id: Some(session_id.key()),
        object_def_id: Some(object_def_id.key()),
        owner_participant_id: Some(owner_participant_id.map(|id| id.key())),
        guarded_neutral_army_id: Some(guarded_neutral_army_id.map(|id| id.key())),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        state: Some(state),
        scoring_kind: Some(scoring_kind),
        last_visited_turn: Some(last_visited_turn),
        captured_turn: Some(captured_turn),
        income_started_turn: Some(income_started_turn),
        instance_json: Some(instance_json),
        last_command_id: Some(None),
    };

    foundation::create("map.create_world_object", input)
}

pub(crate) fn find_world_object_by_session_xy(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
) -> RepoResult<Option<WorldObject>> {
    foundation::storage_result(
        WORLD_OBJECT_COORD_LOOKUP.name,
        crate::db()
            .load::<WorldObject>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("x").eq(x))
            .filter(FieldRef::new("y").eq(y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_world_object(id: Id<WorldObject>) -> RepoResult<Option<WorldObject>> {
    foundation::load_by_id("map.load_world_object", id)
}

pub(crate) fn update_world_object(object: WorldObject) -> RepoResult<WorldObject> {
    foundation::update("map.update_world_object", object)
}

pub(crate) fn page_world_objects_by_session(
    session_id: Id<GameSession>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<WorldObject>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "map.world_objects_by_session",
        crate::db()
            .load::<WorldObject>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("chunk_y")
            .order_asc("chunk_x")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_world_objects_by_owner_scoring_state(
    session_id: Id<GameSession>,
    owner_participant_id: Id<GameParticipant>,
    scoring_kind: &str,
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<WorldObject>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        WORLD_OBJECT_OWNER_SCORING_LOOKUP.name,
        crate::db()
            .load::<WorldObject>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("scoring_kind").eq(scoring_kind))
            .filter(FieldRef::new("owner_participant_id").eq(owner_participant_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_participant_object_visit(
    object_id: Id<WorldObject>,
    participant_id: Id<GameParticipant>,
    visit_key: &str,
) -> RepoResult<Option<ParticipantObjectVisit>> {
    foundation::storage_result(
        "map.participant_object_visit_by_key",
        crate::db()
            .load::<ParticipantObjectVisit>()
            .filter(FieldRef::new("object_id").eq(object_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("visit_key").eq(visit_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_participant_object_visit(
    session_id: Id<GameSession>,
    object_id: Id<WorldObject>,
    participant_id: Id<GameParticipant>,
    visit_key: String,
    visit_kind: String,
    visited_turn: u32,
) -> RepoResult<ParticipantObjectVisit> {
    let input: Create<ParticipantObjectVisit> = Create::<ParticipantObjectVisit> {
        session_id: Some(session_id.key()),
        object_id: Some(object_id.key()),
        participant_id: Some(participant_id.key()),
        visit_key: Some(visit_key),
        visit_kind: Some(visit_kind),
        visited_turn: Some(visited_turn),
    };

    foundation::create("map.create_participant_object_visit", input)
}

#[cfg(test)]
pub(crate) fn map_chunk_plan_text(
    session_id: Id<GameSession>,
    chunk_x: u16,
    chunk_y: u16,
) -> RepoResult<String> {
    foundation::explain_text(
        MAP_CHUNK_COORD_LOOKUP.name,
        crate::db()
            .load::<MapChunk>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn visibility_plan_text(
    participant_id: Id<GameParticipant>,
    chunk_x: u16,
    chunk_y: u16,
) -> RepoResult<String> {
    foundation::explain_text(
        VISIBILITY_CHUNK_LOOKUP.name,
        crate::db()
            .load::<VisibilityChunk>()
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("chunk_x").eq(chunk_x))
            .filter(FieldRef::new("chunk_y").eq(chunk_y))
            .order_asc("id")
            .limit(1),
    )
}
