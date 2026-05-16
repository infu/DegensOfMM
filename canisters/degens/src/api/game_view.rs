use canic_cdk::query;

use crate::dto::public::{
    ApiError, ChampionView, GameView, GameViewRequest, MapChunkPage, ObjectViewPage, Viewport,
};

#[query]
fn get_game_view(_session_id: String, _request: GameViewRequest) -> Result<GameView, ApiError> {
    crate::services::game_view::unavailable("get_game_view")
}

#[query]
fn get_visible_map_chunks(
    _session_id: String,
    _viewport: Viewport,
    _cursor: Option<u32>,
    _limit: u32,
) -> Result<MapChunkPage, ApiError> {
    crate::services::game_view::unavailable("get_visible_map_chunks")
}

#[query]
fn get_visible_objects(
    _session_id: String,
    _viewport: Viewport,
    _cursor: Option<u32>,
    _limit: u32,
) -> Result<ObjectViewPage, ApiError> {
    crate::services::game_view::unavailable("get_visible_objects")
}

#[query]
fn get_my_champions(_session_id: String) -> Result<Vec<ChampionView>, ApiError> {
    crate::services::game_view::unavailable("get_my_champions")
}

#[query]
fn get_champion_view(_session_id: String, _champion_id: String) -> Result<ChampionView, ApiError> {
    crate::services::game_view::unavailable("get_champion_view")
}
