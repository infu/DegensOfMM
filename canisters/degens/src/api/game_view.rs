use canic_cdk::query;

use crate::dto::public::{
    ApiError, ChampionView, GameView, GameViewRequest, MapChunkPage, ObjectView, ObjectViewPage,
    Viewport,
};

#[query]
fn get_game_view(session_id: String, request: GameViewRequest) -> Result<GameView, ApiError> {
    crate::services::game_view::get_game_view(canic_cdk::api::msg_caller(), session_id, request)
}

#[query]
fn get_visible_map_chunks(
    session_id: String,
    viewport: Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<MapChunkPage, ApiError> {
    crate::services::game_view::get_visible_map_chunks(
        canic_cdk::api::msg_caller(),
        session_id,
        viewport,
        cursor,
        limit,
    )
}

#[query]
fn get_visible_objects(
    session_id: String,
    viewport: Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<ObjectViewPage, ApiError> {
    crate::services::game_view::get_visible_objects(
        canic_cdk::api::msg_caller(),
        session_id,
        viewport,
        cursor,
        limit,
    )
}

#[query]
fn get_object_view(
    session_id: String,
    subject_kind: String,
    subject_id_text: String,
) -> Result<ObjectView, ApiError> {
    crate::services::game_view::get_object_view(
        canic_cdk::api::msg_caller(),
        session_id,
        subject_kind,
        subject_id_text,
    )
}

#[query]
fn get_my_champions(session_id: String) -> Result<Vec<ChampionView>, ApiError> {
    crate::services::game_view::get_my_champions(canic_cdk::api::msg_caller(), session_id)
}

#[query]
fn get_champion_view(session_id: String, champion_id: String) -> Result<ChampionView, ApiError> {
    crate::services::game_view::get_champion_view(
        canic_cdk::api::msg_caller(),
        session_id,
        champion_id,
    )
}
