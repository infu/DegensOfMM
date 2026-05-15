use candid::Principal;
use domm_game::{
    ApiEventPage, FIRST_PLAYABLE_CHUNK_SIZE, GameViewRequest, MAP_FLAG_BLOCKING_TERRAIN,
    MAP_FLAG_ROAD, MapChunkView, ObjectView, read_visibility_bit,
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
        let mut game_view = self.backend.default_game_view(caller, session_id)?;
        let viewport = game_view.viewport.clone();
        let chunks = self.collect_chunks(caller, session_id, &mut game_view)?;
        let objects = self.collect_objects(caller, session_id, &mut game_view)?;
        let events = ApiEventPage {
            events: game_view.events.clone(),
            page_info: game_view.event_page_info.clone(),
        };
        let sync_required = game_view.render_time.sync_required;

        Ok(ClientOpeningViewport {
            game_view,
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
        game_view: &mut domm_game::GameView,
    ) -> Result<Vec<MapChunkView>, ProbeError> {
        let mut chunks = game_view.map_chunks.clone();
        let mut page_info = game_view.map_page_info.clone();
        while page_info.has_more {
            let page = self.fetch_page(
                caller,
                session_id,
                game_view,
                Some(page_info.next_cursor),
                None,
            )?;
            chunks.extend(page.map_chunks.clone());
            page_info = page.map_page_info;
        }
        Ok(chunks)
    }

    fn collect_objects(
        &mut self,
        caller: Principal,
        session_id: &str,
        game_view: &mut domm_game::GameView,
    ) -> Result<Vec<ObjectView>, ProbeError> {
        let mut objects = game_view.objects.clone();
        let mut page_info = game_view.object_page_info.clone();
        while page_info.has_more {
            let page = self.fetch_page(
                caller,
                session_id,
                game_view,
                None,
                Some(page_info.next_cursor),
            )?;
            objects.extend(page.objects.clone());
            page_info = page.object_page_info;
        }
        Ok(objects)
    }

    fn fetch_page(
        &mut self,
        caller: Principal,
        session_id: &str,
        game_view: &domm_game::GameView,
        chunk_cursor: Option<Option<u32>>,
        object_cursor: Option<Option<u32>>,
    ) -> Result<domm_game::GameView, ProbeError> {
        let request = GameViewRequest {
            viewport: game_view.viewport.clone(),
            chunk_cursor: chunk_cursor.flatten(),
            chunk_limit: CHUNK_PAGE_LIMIT,
            object_cursor: object_cursor.flatten(),
            object_limit: OBJECT_PAGE_LIMIT,
            events_after_seq: game_view.events.last().map_or(0, |event| event.event_seq),
            event_limit: EVENT_PAGE_LIMIT as u32,
            include_battle: false,
        };
        self.backend.game_view(caller, session_id, request)
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
