use candid::Principal;
use domm_game::{
    FIRST_PLAYABLE_CHUNK_SIZE, MAP_FLAG_BLOCKING_TERRAIN, MAP_FLAG_ROAD, MapChunkView,
    OPENING_VIEWPORT_EAST_X, OPENING_VIEWPORT_EAST_Y, OPENING_VIEWPORT_HEIGHT,
    OPENING_VIEWPORT_WEST_X, OPENING_VIEWPORT_WEST_Y, OPENING_VIEWPORT_WIDTH, ObjectView, Viewport,
    read_visibility_bit,
};

use crate::backend::ThinClientBackend;
use crate::types::{ClientOpeningViewport, ProbeError, RenderedViewport};

const CHUNK_PAGE_LIMIT: u32 = 2;
const OBJECT_PAGE_LIMIT: u32 = 4;
const EVENT_PAGE_LIMIT: usize = 16;

pub struct ThinClientProbe<B> {
    backend: B,
}

impl<B> ThinClientProbe<B> {
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: ThinClientBackend> ThinClientProbe<B> {
    pub fn load_opening_viewport(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ClientOpeningViewport, ProbeError> {
        let match_view = self.backend.active_match(caller, session_id)?;
        let participant = self.backend.my_participant(caller, session_id)?;
        let viewport = opening_viewport_for_slot(participant.slot_index)?;
        let chunks = self.collect_chunks(caller, session_id, &viewport)?;
        let objects = self.collect_objects(caller, session_id, &viewport)?;
        let events = self
            .backend
            .events_after(caller, session_id, 0, EVENT_PAGE_LIMIT)?;
        let sync_required = match_view.sync_required;

        Ok(ClientOpeningViewport {
            match_view,
            participant,
            viewport,
            chunks,
            objects,
            events,
            sync_required,
        })
    }

    fn collect_chunks(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
    ) -> Result<Vec<MapChunkView>, ProbeError> {
        let mut chunks = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.backend.viewport_chunks(
                caller,
                session_id,
                viewport,
                cursor,
                CHUNK_PAGE_LIMIT,
            )?;
            chunks.extend(page.chunks);
            if !page.has_more {
                return Ok(chunks);
            }
            cursor = page.next_cursor;
        }
    }

    fn collect_objects(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
    ) -> Result<Vec<ObjectView>, ProbeError> {
        let mut objects = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.backend.viewport_objects(
                caller,
                session_id,
                viewport,
                cursor,
                OBJECT_PAGE_LIMIT,
            )?;
            objects.extend(page.objects);
            if !page.has_more {
                return Ok(objects);
            }
            cursor = page.next_cursor;
        }
    }
}

pub fn render_opening_viewport(
    state: &ClientOpeningViewport,
) -> Result<RenderedViewport, ProbeError> {
    let mut rows =
        vec![vec![b' '; usize::from(state.viewport.width)]; usize::from(state.viewport.height)];

    for row_y in 0..state.viewport.height {
        for row_x in 0..state.viewport.width {
            let x = state.viewport.x + row_x;
            let y = state.viewport.y + row_y;
            let chunk = chunk_for_tile(&state.chunks, x, y)
                .ok_or(ProbeError::MissingChunkForTile { x, y })?;
            let local_x = x - chunk.chunk_x * u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
            let local_y = y - chunk.chunk_y * u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
            let cell_index = usize::from(local_y) * usize::from(chunk.width) + usize::from(local_x);
            let terrain = *chunk
                .terrain_blob
                .get(cell_index)
                .ok_or(ProbeError::MissingChunkCell { cell_index })?;
            let flags = *chunk
                .flags_blob
                .get(cell_index)
                .ok_or(ProbeError::MissingChunkCell { cell_index })?;
            rows[usize::from(row_y)][usize::from(row_x)] =
                tile_char(chunk, cell_index, terrain, flags);
        }
    }

    let mut visible_champions = Vec::new();
    let mut visible_towns = Vec::new();
    let mut visible_resources = Vec::new();
    let mut visible_neutrals = Vec::new();

    for object in &state.objects {
        if !state.viewport.contains(object.x, object.y) {
            continue;
        }
        let row_x = usize::from(object.x - state.viewport.x);
        let row_y = usize::from(object.y - state.viewport.y);
        rows[row_y][row_x] = object_char(object);
        match object.subject_kind.as_str() {
            "champion" => visible_champions.push(object_label(object)),
            "town" => visible_towns.push(object_label(object)),
            "neutral_army" => visible_neutrals.push(object.subject_id_text.clone()),
            "world_object" if object.subject_id_text.starts_with("pile:") => {
                visible_resources.push(object.subject_id_text.clone());
            }
            _ => {}
        }
    }

    let rows = rows
        .into_iter()
        .map(|row| String::from_utf8(row).map_err(|_| ProbeError::InvalidRenderedRow))
        .collect::<Result<Vec<_>, _>>()?;
    let event_summaries = state
        .events
        .events
        .iter()
        .map(|event| format!("{}#{}", event.event_type, event.event_seq))
        .collect();

    Ok(RenderedViewport {
        width: state.viewport.width,
        height: state.viewport.height,
        rows,
        visible_champions,
        visible_towns,
        visible_resources,
        visible_neutrals,
        event_summaries,
        sync_required: state.sync_required,
    })
}

fn opening_viewport_for_slot(slot_index: u8) -> Result<Viewport, ProbeError> {
    match slot_index {
        0 => Ok(Viewport::new(
            OPENING_VIEWPORT_WEST_X,
            OPENING_VIEWPORT_WEST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        )),
        1 => Ok(Viewport::new(
            OPENING_VIEWPORT_EAST_X,
            OPENING_VIEWPORT_EAST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        )),
        _ => Err(ProbeError::MissingOpeningViewport { slot_index }),
    }
}

fn chunk_for_tile(chunks: &[MapChunkView], x: u16, y: u16) -> Option<&MapChunkView> {
    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let chunk_x = x / chunk_size;
    let chunk_y = y / chunk_size;
    chunks
        .iter()
        .find(|chunk| chunk.chunk_x == chunk_x && chunk.chunk_y == chunk_y)
}

fn tile_char(chunk: &MapChunkView, cell_index: usize, terrain: u8, flags: u8) -> u8 {
    if read_visibility_bit(&chunk.visible_blob, cell_index) {
        return visible_terrain_char(terrain, flags);
    }
    if read_visibility_bit(&chunk.discovered_blob, cell_index) {
        return b'?';
    }
    b' '
}

fn visible_terrain_char(terrain: u8, flags: u8) -> u8 {
    if flags & MAP_FLAG_ROAD != 0 {
        return b'=';
    }
    if flags & MAP_FLAG_BLOCKING_TERRAIN != 0 {
        return b'^';
    }
    match terrain {
        1 => b'.',
        3 => b'F',
        4 => b'S',
        5 => b'R',
        6 => b'^',
        _ => b'.',
    }
}

fn object_char(object: &ObjectView) -> u8 {
    match object.subject_kind.as_str() {
        "champion" => b'C',
        "town" => b'T',
        "neutral_army" => b'N',
        "world_object" if object.subject_id_text.starts_with("mine:") => b'M',
        "world_object" if object.subject_id_text.starts_with("pile:") => b'$',
        "world_object" if object.subject_id_text.starts_with("objective:") => b'O',
        _ => b'*',
    }
}

fn object_label(object: &ObjectView) -> String {
    object
        .display_name
        .clone()
        .unwrap_or_else(|| object.subject_id_text.clone())
}
