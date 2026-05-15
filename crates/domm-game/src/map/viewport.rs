use super::bitset::read_visibility_bit;
use super::types::{
    FirstPlayableMapState, MapChunkPage, MapChunkRecord, MapChunkView, MapSubjectRecord,
    ObjectView, ObjectViewPage, ParticipantKnownObjectRecord, SubjectViewResult, Viewport,
    VisibilityChunkRecord,
};
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;

enum VisibilityBlobKind {
    Discovered,
    Visible,
}

impl FirstPlayableMapState {
    #[must_use]
    pub fn chunk_at(&self, chunk_x: u16, chunk_y: u16) -> Option<&MapChunkRecord> {
        self.chunks
            .iter()
            .find(|chunk| chunk.chunk_x == chunk_x && chunk.chunk_y == chunk_y)
    }

    #[must_use]
    pub fn visibility_at(
        &self,
        participant_id: &str,
        chunk_x: u16,
        chunk_y: u16,
    ) -> Option<&VisibilityChunkRecord> {
        self.visibility_chunks.iter().find(|chunk| {
            chunk.participant_id == participant_id
                && chunk.chunk_x == chunk_x
                && chunk.chunk_y == chunk_y
        })
    }

    #[must_use]
    pub fn terrain_code_at(&self, x: u16, y: u16) -> Option<u8> {
        self.chunk_cell_blob_value(x, y, |chunk| &chunk.terrain_blob)
    }

    #[must_use]
    pub fn movement_cost_at(&self, x: u16, y: u16) -> Option<u8> {
        self.chunk_cell_blob_value(x, y, |chunk| &chunk.movement_blob)
    }

    #[must_use]
    pub fn flags_at(&self, x: u16, y: u16) -> Option<u8> {
        self.chunk_cell_blob_value(x, y, |chunk| &chunk.flags_blob)
    }

    #[must_use]
    pub fn is_visible_at(&self, participant_id: &str, x: u16, y: u16) -> bool {
        self.visibility_bit(participant_id, x, y, VisibilityBlobKind::Visible)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn is_discovered_at(&self, participant_id: &str, x: u16, y: u16) -> bool {
        self.visibility_bit(participant_id, x, y, VisibilityBlobKind::Discovered)
            .unwrap_or(false)
    }

    #[must_use]
    pub fn map_chunk_views(
        &self,
        participant_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> MapChunkPage {
        let mut matching: Vec<&MapChunkRecord> = self
            .chunks
            .iter()
            .filter(|chunk| viewport.intersects_chunk(chunk))
            .collect();
        matching.sort_by_key(|chunk| (chunk.chunk_y, chunk.chunk_x));

        let start = cursor.unwrap_or(0) as usize;
        let limit = limit.max(1) as usize;
        let end = (start + limit).min(matching.len());
        let chunks = matching
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .filter_map(|chunk| self.chunk_view(participant_id, chunk))
            .collect::<Vec<_>>();
        let has_more = end < matching.len();

        MapChunkPage {
            chunks,
            next_cursor: has_more.then_some(end as u32),
            has_more,
        }
    }

    #[must_use]
    pub fn object_views(
        &self,
        participant_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> ObjectViewPage {
        let mut visible =
            self.subjects
                .iter()
                .filter(|subject| viewport.contains(subject.x, subject.y))
                .filter_map(|subject| {
                    match self.subject_view(
                        participant_id,
                        &subject.subject_kind,
                        &subject.subject_id_text,
                    ) {
                        SubjectViewResult::Visible(view) | SubjectViewResult::LastKnown(view) => {
                            Some(view)
                        }
                        SubjectViewResult::NotVisible { .. }
                        | SubjectViewResult::NotFound { .. } => None,
                    }
                })
                .collect::<Vec<_>>();
        visible.sort_by_key(|view| {
            (
                view.y,
                view.x,
                view.subject_kind.clone(),
                view.subject_id_text.clone(),
            )
        });

        let start = cursor.unwrap_or(0) as usize;
        let limit = limit.max(1) as usize;
        let end = (start + limit).min(visible.len());
        let objects = visible.get(start..end).unwrap_or_default().to_vec();
        let has_more = end < visible.len();

        ObjectViewPage {
            objects,
            next_cursor: has_more.then_some(end as u32),
            has_more,
        }
    }

    #[must_use]
    pub fn subject_view(
        &self,
        participant_id: &str,
        subject_kind: &str,
        subject_id_text: &str,
    ) -> SubjectViewResult {
        let Some(subject) = self.subjects.iter().find(|subject| {
            subject.subject_kind == subject_kind && subject.subject_id_text == subject_id_text
        }) else {
            return SubjectViewResult::NotFound {
                subject_kind: subject_kind.to_string(),
                subject_id_text: subject_id_text.to_string(),
            };
        };

        if self.is_visible_at(participant_id, subject.x, subject.y) {
            return SubjectViewResult::Visible(subject.to_visible_view(1));
        }

        if let Some(known) = self.known_objects.iter().find(|known| {
            known.participant_id == participant_id
                && known.subject_kind == subject_kind
                && known.subject_id_text == subject_id_text
        }) {
            return SubjectViewResult::LastKnown(subject.to_last_known_view(known));
        }

        SubjectViewResult::NotVisible {
            subject_kind: subject_kind.to_string(),
            subject_id_text: subject_id_text.to_string(),
            visibility: "hidden".to_string(),
        }
    }

    fn chunk_view(&self, participant_id: &str, chunk: &MapChunkRecord) -> Option<MapChunkView> {
        let visibility = self.visibility_at(participant_id, chunk.chunk_x, chunk.chunk_y)?;
        Some(MapChunkView {
            chunk_id: chunk.chunk_id.clone(),
            chunk_x: chunk.chunk_x,
            chunk_y: chunk.chunk_y,
            width: chunk.width,
            height: chunk.height,
            terrain_blob: chunk.terrain_blob.clone(),
            movement_blob: chunk.movement_blob.clone(),
            flags_blob: chunk.flags_blob.clone(),
            discovered_blob: visibility.discovered_blob.clone(),
            visible_blob: visibility.visible_blob.clone(),
        })
    }

    fn chunk_cell_blob_value(
        &self,
        x: u16,
        y: u16,
        blob: impl Fn(&MapChunkRecord) -> &[u8],
    ) -> Option<u8> {
        let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        let chunk = self.chunk_at(x / chunk_size, y / chunk_size)?;
        let local_x = x % chunk_size;
        let local_y = y % chunk_size;
        let index = local_index(local_x, local_y, chunk.width)?;
        blob(chunk).get(index).copied()
    }

    fn visibility_bit(
        &self,
        participant_id: &str,
        x: u16,
        y: u16,
        kind: VisibilityBlobKind,
    ) -> Option<bool> {
        let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        let chunk_x = x / chunk_size;
        let chunk_y = y / chunk_size;
        let local_x = x % chunk_size;
        let local_y = y % chunk_size;
        let visibility = self.visibility_at(participant_id, chunk_x, chunk_y)?;
        let index = local_index(local_x, local_y, visibility.width)?;
        let blob = match kind {
            VisibilityBlobKind::Discovered => &visibility.discovered_blob,
            VisibilityBlobKind::Visible => &visibility.visible_blob,
        };
        Some(read_visibility_bit(blob, index))
    }
}

impl MapSubjectRecord {
    fn to_visible_view(&self, turn: u32) -> ObjectView {
        ObjectView {
            subject_kind: self.subject_kind.clone(),
            subject_id_text: self.subject_id_text.clone(),
            visibility: "visible".to_string(),
            redaction_level: "none".to_string(),
            x: self.x,
            y: self.y,
            last_seen_turn: Some(turn),
            display_name: Some(self.display_name.clone()),
            asset_key: self.asset_key.clone(),
            owner_participant_id: self.owner_participant_id.clone(),
            details_json: self.public_json.clone(),
        }
    }

    fn to_last_known_view(&self, known: &ParticipantKnownObjectRecord) -> ObjectView {
        ObjectView {
            subject_kind: self.subject_kind.clone(),
            subject_id_text: self.subject_id_text.clone(),
            visibility: "last_known".to_string(),
            redaction_level: "last_known".to_string(),
            x: known.x,
            y: known.y,
            last_seen_turn: Some(known.last_seen_turn),
            display_name: None,
            asset_key: self.asset_key.clone(),
            owner_participant_id: None,
            details_json: known.redacted_json.clone(),
        }
    }
}

pub(crate) fn local_index(local_x: u16, local_y: u16, width: u16) -> Option<usize> {
    (local_x < width).then_some(usize::from(local_y) * usize::from(width) + usize::from(local_x))
}
