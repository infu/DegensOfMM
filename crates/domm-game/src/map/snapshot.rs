use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::types::{
    FirstPlayableMapState, OPENING_VIEWPORT_EAST_X, OPENING_VIEWPORT_EAST_Y,
    OPENING_VIEWPORT_HEIGHT, OPENING_VIEWPORT_WEST_X, OPENING_VIEWPORT_WEST_Y,
    OPENING_VIEWPORT_WIDTH, OpeningViewportSnapshot, Viewport,
};

impl FirstPlayableMapState {
    #[must_use]
    pub fn opening_viewport_snapshot(&self, participant_id: &str) -> OpeningViewportSnapshot {
        let viewport = opening_viewport_for_participant(&self.participant_ids, participant_id);
        let chunks = self
            .map_chunk_views(participant_id, &viewport, None, 32)
            .chunks;
        let objects = self
            .object_views(participant_id, &viewport, None, 64)
            .objects;
        let mut snapshot = OpeningViewportSnapshot {
            participant_id: participant_id.to_string(),
            viewport,
            chunks,
            objects,
            snapshot_hash: String::new(),
        };
        snapshot.snapshot_hash = snapshot.computed_snapshot_hash();
        snapshot
    }
}

impl OpeningViewportSnapshot {
    #[must_use]
    pub fn computed_snapshot_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hash_text(&mut hasher, "schema", "domm.opening_viewport.v1");
        hash_text(&mut hasher, "participant_id", &self.participant_id);
        hash_u16(&mut hasher, "viewport.x", self.viewport.x);
        hash_u16(&mut hasher, "viewport.y", self.viewport.y);
        hash_u16(&mut hasher, "viewport.width", self.viewport.width);
        hash_u16(&mut hasher, "viewport.height", self.viewport.height);
        for chunk in &self.chunks {
            hash_text(&mut hasher, "chunk.id", &chunk.chunk_id);
            hash_u16(&mut hasher, "chunk.x", chunk.chunk_x);
            hash_u16(&mut hasher, "chunk.y", chunk.chunk_y);
            hash_u16(&mut hasher, "chunk.width", chunk.width);
            hash_u16(&mut hasher, "chunk.height", chunk.height);
            hash_bytes(&mut hasher, "chunk.terrain", &chunk.terrain_blob);
            hash_bytes(&mut hasher, "chunk.movement", &chunk.movement_blob);
            hash_bytes(&mut hasher, "chunk.flags", &chunk.flags_blob);
            hash_bytes(&mut hasher, "chunk.discovered", &chunk.discovered_blob);
            hash_bytes(&mut hasher, "chunk.visible", &chunk.visible_blob);
        }
        for object in &self.objects {
            hash_text(&mut hasher, "object.kind", &object.subject_kind);
            hash_text(&mut hasher, "object.id", &object.subject_id_text);
            hash_text(&mut hasher, "object.visibility", &object.visibility);
            hash_text(&mut hasher, "object.redaction", &object.redaction_level);
            hash_u16(&mut hasher, "object.x", object.x);
            hash_u16(&mut hasher, "object.y", object.y);
            hash_optional_u32(&mut hasher, "object.last_seen", object.last_seen_turn);
            hash_optional_text(
                &mut hasher,
                "object.display_name",
                object.display_name.as_deref(),
            );
            hash_optional_text(&mut hasher, "object.asset", object.asset_key.as_deref());
            hash_optional_text(
                &mut hasher,
                "object.owner",
                object.owner_participant_id.as_deref(),
            );
            hash_text(&mut hasher, "object.details", &object.details_json);
        }
        to_hex(&hasher.finalize())
    }
}

fn opening_viewport_for_participant(participant_ids: &[String], participant_id: &str) -> Viewport {
    if participant_ids
        .get(1)
        .is_some_and(|id| id == participant_id)
    {
        Viewport::new(
            OPENING_VIEWPORT_EAST_X,
            OPENING_VIEWPORT_EAST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        )
    } else {
        Viewport::new(
            OPENING_VIEWPORT_WEST_X,
            OPENING_VIEWPORT_WEST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        )
    }
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hash_bytes(hasher, label, value.as_bytes());
}

fn hash_optional_text(hasher: &mut Sha256, label: &str, value: Option<&str>) {
    match value {
        Some(value) => hash_text(hasher, label, value),
        None => hash_text(hasher, label, "<none>"),
    }
}

fn hash_optional_u32(hasher: &mut Sha256, label: &str, value: Option<u32>) {
    match value {
        Some(value) => hash_u32(hasher, label, value),
        None => hash_text(hasher, label, "<none>"),
    }
}

fn hash_u16(hasher: &mut Sha256, label: &str, value: u16) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_u32(hasher: &mut Sha256, label: &str, value: u32) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_bytes(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update(b":");
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value);
    hasher.update(b"\n");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}
