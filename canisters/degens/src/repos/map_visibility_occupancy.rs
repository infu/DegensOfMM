//! Repository boundary for map chunks, terrain blobs, visibility, known objects, and occupancy.

use domm_degens_schema::schema::{
    GameParticipant, GameSession, MapChunk, MapOccupancy, ParticipantKnownObject, VisibilityChunk,
};
use icydb::{db::query::FieldRef, types::Id};

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
