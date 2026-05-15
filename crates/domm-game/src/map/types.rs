use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;

pub const MAP_FLAG_PASSABLE: u8 = 0b0000_0001;
pub const MAP_FLAG_ROAD: u8 = 0b0000_0010;
pub const MAP_FLAG_BLOCKING_TERRAIN: u8 = 0b0000_0100;
pub const OPENING_VIEWPORT_WIDTH: u16 = 24;
pub const OPENING_VIEWPORT_HEIGHT: u16 = 24;
pub const OPENING_VIEWPORT_WEST_X: u16 = 0;
pub const OPENING_VIEWPORT_WEST_Y: u16 = 16;
pub const OPENING_VIEWPORT_EAST_X: u16 = 24;
pub const OPENING_VIEWPORT_EAST_Y: u16 = 16;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct Viewport {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Viewport {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        let max_x = self.x.saturating_add(self.width);
        let max_y = self.y.saturating_add(self.height);
        x >= self.x && x < max_x && y >= self.y && y < max_y
    }

    #[must_use]
    pub fn intersects_chunk(&self, chunk: &MapChunkRecord) -> bool {
        let chunk_min_x = chunk.chunk_x * u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        let chunk_min_y = chunk.chunk_y * u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        let chunk_max_x = chunk_min_x + chunk.width;
        let chunk_max_y = chunk_min_y + chunk.height;
        let view_max_x = self.x.saturating_add(self.width);
        let view_max_y = self.y.saturating_add(self.height);

        self.x < chunk_max_x
            && view_max_x > chunk_min_x
            && self.y < chunk_max_y
            && view_max_y > chunk_min_y
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapChunkRecord {
    pub chunk_id: String,
    pub session_id: String,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub width: u16,
    pub height: u16,
    pub terrain_blob: Vec<u8>,
    pub movement_blob: Vec<u8>,
    pub flags_blob: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct VisibilityChunkRecord {
    pub visibility_chunk_id: String,
    pub session_id: String,
    pub participant_id: String,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub width: u16,
    pub height: u16,
    pub discovered_blob: Vec<u8>,
    pub visible_blob: Vec<u8>,
    pub visible_turn: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapOccupancyRecord {
    pub occupancy_id: String,
    pub session_id: String,
    pub x: u16,
    pub y: u16,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub layer: String,
    pub occupant_kind: String,
    pub occupant_id_text: String,
    pub occupant_cell_index: u16,
    pub blocking: bool,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WorldObjectRecord {
    pub object_id: String,
    pub session_id: String,
    pub object_slug: String,
    pub object_type: String,
    pub scoring_kind: Option<String>,
    pub owner_participant_id: Option<String>,
    pub guarded_neutral_army_id: Option<String>,
    pub x: u16,
    pub y: u16,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub state: String,
    pub public_json: String,
    pub redacted_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ParticipantKnownObjectRecord {
    pub participant_id: String,
    pub subject_kind: String,
    pub subject_id_text: String,
    pub x: u16,
    pub y: u16,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub visibility: String,
    pub last_seen_turn: u32,
    pub redacted_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapSubjectRecord {
    pub subject_kind: String,
    pub subject_id_text: String,
    pub display_name: String,
    pub asset_key: Option<String>,
    pub owner_participant_id: Option<String>,
    pub object_slug: Option<String>,
    pub object_type: Option<String>,
    pub scoring_kind: Option<String>,
    pub x: u16,
    pub y: u16,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub state: String,
    pub public_json: String,
    pub redacted_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapChunkView {
    pub chunk_id: String,
    pub chunk_x: u16,
    pub chunk_y: u16,
    pub width: u16,
    pub height: u16,
    pub terrain_blob: Vec<u8>,
    pub movement_blob: Vec<u8>,
    pub flags_blob: Vec<u8>,
    pub discovered_blob: Vec<u8>,
    pub visible_blob: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapChunkPage {
    pub chunks: Vec<MapChunkView>,
    pub next_cursor: Option<u32>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectView {
    pub subject_kind: String,
    pub subject_id_text: String,
    pub visibility: String,
    pub redaction_level: String,
    pub x: u16,
    pub y: u16,
    pub last_seen_turn: Option<u32>,
    pub display_name: Option<String>,
    pub asset_key: Option<String>,
    pub owner_participant_id: Option<String>,
    pub details_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectViewPage {
    pub objects: Vec<ObjectView>,
    pub next_cursor: Option<u32>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum SubjectViewResult {
    Visible(ObjectView),
    LastKnown(ObjectView),
    NotVisible {
        subject_kind: String,
        subject_id_text: String,
        visibility: String,
    },
    NotFound {
        subject_kind: String,
        subject_id_text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct OpeningViewportSnapshot {
    pub participant_id: String,
    pub viewport: Viewport,
    pub chunks: Vec<MapChunkView>,
    pub objects: Vec<ObjectView>,
    pub snapshot_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FirstPlayableMapState {
    pub session_id: String,
    pub participant_ids: Vec<String>,
    pub chunks: Vec<MapChunkRecord>,
    pub visibility_chunks: Vec<VisibilityChunkRecord>,
    pub occupancy_rows: Vec<MapOccupancyRecord>,
    pub world_objects: Vec<WorldObjectRecord>,
    pub known_objects: Vec<ParticipantKnownObjectRecord>,
    pub subjects: Vec<MapSubjectRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MapError {
    #[error("tile ({x},{y}) is outside map bounds")]
    OutOfBounds { x: u16, y: u16 },
    #[error("tile ({x},{y}) layer {layer} is already occupied")]
    OccupancyTileCollision { x: u16, y: u16, layer: String },
    #[error("occupant {occupant_kind}:{occupant_id_text} already has cell {occupant_cell_index}")]
    OccupancyCellCollision {
        occupant_kind: String,
        occupant_id_text: String,
        occupant_cell_index: u16,
    },
}
