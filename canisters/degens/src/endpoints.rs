use canic_cdk::{query, update};
use domm_game::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview, ChampionView,
    CommandResponse, CommandStatusView, ContentManifestResponse, GameView, GameViewRequest,
    LobbyCommandResponse, MapChunkPage, MatchHistoryPage, MoveCoord, MovementPreview,
    ObjectViewPage, ParticipantView, PlayerView, RecruitPreview, RecruitTarget, SessionView,
    Viewport,
};

use crate::contract::{CanisterEndpointView, required_endpoint_views};

#[query]
fn get_canister_endpoint_inventory() -> Vec<CanisterEndpointView> {
    required_endpoint_views()
}

#[update]
fn register_player(
    _username: Option<String>,
    _display_name: Option<String>,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    unavailable("register_player")
}

#[query]
fn get_my_player() -> Result<PlayerView, ApiError> {
    unavailable("get_my_player")
}

#[update]
fn create_session(
    _name: String,
    _ruleset_id: String,
    _seed: u64,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    unavailable("create_session")
}

#[update]
fn join_session(
    _session_id: String,
    _faction_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    unavailable("join_session")
}

#[update]
fn mark_ready(
    _session_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    unavailable("mark_ready")
}

#[update]
fn start_session(
    _session_id: String,
    _client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    unavailable("start_session")
}

#[query]
fn get_session(_session_id: String) -> Result<SessionView, ApiError> {
    unavailable("get_session")
}

#[query]
fn get_my_participant(_session_id: String) -> Result<ParticipantView, ApiError> {
    unavailable("get_my_participant")
}

#[query]
fn get_match_history(_cursor: u32, _limit: u32) -> Result<MatchHistoryPage, ApiError> {
    unavailable("get_match_history")
}

#[query]
fn get_game_view(_session_id: String, _request: GameViewRequest) -> Result<GameView, ApiError> {
    unavailable("get_game_view")
}

#[query]
fn get_visible_map_chunks(
    _session_id: String,
    _viewport: Viewport,
    _cursor: Option<u32>,
    _limit: u32,
) -> Result<MapChunkPage, ApiError> {
    unavailable("get_visible_map_chunks")
}

#[query]
fn get_visible_objects(
    _session_id: String,
    _viewport: Viewport,
    _cursor: Option<u32>,
    _limit: u32,
) -> Result<ObjectViewPage, ApiError> {
    unavailable("get_visible_objects")
}

#[query]
fn get_my_champions(_session_id: String) -> Result<Vec<ChampionView>, ApiError> {
    unavailable("get_my_champions")
}

#[query]
fn get_champion_view(_session_id: String, _champion_id: String) -> Result<ChampionView, ApiError> {
    unavailable("get_champion_view")
}

#[query]
fn get_town_view(_session_id: String, _town_id: String) -> Result<ApiTownView, ApiError> {
    unavailable("get_town_view")
}

#[query]
fn get_battle_state(
    _session_id: String,
    _battle_id: String,
    _now_ms: u64,
) -> Result<BattleView, ApiError> {
    unavailable("get_battle_state")
}

#[query]
fn get_content_manifest(
    _ruleset_id: String,
    _version: u32,
) -> Result<ContentManifestResponse, ApiError> {
    unavailable("get_content_manifest")
}

#[query]
fn get_events_after(
    _session_id: String,
    _audience_key: String,
    _events_after_seq: u64,
    _limit: u32,
) -> Result<ApiEventPage, ApiError> {
    unavailable("get_events_after")
}

#[query]
fn get_command_status(
    _session_id: String,
    _command_id_or_client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    unavailable("get_command_status")
}

#[query]
fn preview_move_path(
    _session_id: String,
    _champion_id: String,
    _path: Vec<MoveCoord>,
    _now_ms: u64,
) -> Result<MovementPreview, ApiError> {
    unavailable("preview_move_path")
}

#[query]
fn preview_build_town_structure(
    _session_id: String,
    _town_id: String,
    _building_def_id: String,
) -> Result<BuildPreview, ApiError> {
    unavailable("preview_build_town_structure")
}

#[query]
fn preview_recruit_units(
    _session_id: String,
    _town_id: String,
    _unit_id: String,
    _quantity: u32,
    _target: RecruitTarget,
) -> Result<RecruitPreview, ApiError> {
    unavailable("preview_recruit_units")
}

#[update]
fn submit_move_intent(
    _session_id: String,
    _champion_id: String,
    _path: Vec<MoveCoord>,
    _client_nonce: String,
    _now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    unavailable("submit_move_intent")
}

#[update]
fn sync_session_turn(
    _session_id: String,
    _now_ms: u64,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    unavailable("sync_session_turn")
}

#[update]
fn submit_build_town_structure(
    _session_id: String,
    _town_id: String,
    _building_def_id: String,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    unavailable("submit_build_town_structure")
}

#[update]
fn submit_recruit_units(
    _session_id: String,
    _town_id: String,
    _unit_id: String,
    _quantity: u32,
    _target: RecruitTarget,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    unavailable("submit_recruit_units")
}

#[update]
fn sync_battle(
    _session_id: String,
    _battle_id: String,
    _now_ms: u64,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    unavailable("sync_battle")
}

#[update]
fn submit_battle_action(
    _session_id: String,
    _input: BattleActionInput,
    _client_nonce: String,
    _now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    unavailable("submit_battle_action")
}

fn unavailable<T>(endpoint: &str) -> Result<T, ApiError> {
    Err(ApiError::new(
        "icydb_repository_not_implemented",
        format!(
            "{endpoint} is declared in the canister contract; IcyDB repository wiring starts in checkpoint 19C"
        ),
        true,
    ))
}
