use candid::Principal as CandidPrincipal;
use domm_game::{
    ApiError, ApiEventView, ChampionView, EventPageInfo, GameView, GameViewRequest, MapChunkPage,
    ObjectViewPage, ParticipantSummary, RenderTimeMeta, Viewport,
};
use icydb::{traits::EntityValue, types::Timestamp};

use crate::repos::foundation;

use super::{render_projection, session_context};

pub(crate) fn get_game_view(
    caller: CandidPrincipal,
    session_id: String,
    request: GameViewRequest,
) -> Result<GameView, ApiError> {
    validate_game_view_request(&request)?;
    let context = session_context::require_session_caller(caller, &session_id)?;
    let chunks = render_projection::visible_map_chunks(
        &context,
        &request.viewport,
        request.chunk_cursor,
        request.chunk_limit,
    )?;
    let objects = render_projection::visible_objects(
        &context,
        &request.viewport,
        request.object_cursor,
        request.object_limit,
    )?;
    let champions = Vec::new();
    let towns = Vec::new();
    let events = opening_event_page(&context, request.events_after_seq, request.event_limit);
    let render_time = render_time_meta(&context.session);
    let action_affordances =
        render_projection::action_affordances(&champions, &towns, render_time.sync_required);
    let map_page_info = render_projection::map_page_info(&chunks, request.chunk_limit);
    let object_page_info = render_projection::object_page_info(&objects, request.object_limit);

    Ok(GameView {
        session: session_context::session_summary(&context.session)?,
        participant: ParticipantSummary::from(session_context::participant_view(
            &context.participant,
        )?),
        viewport: request.viewport,
        map_chunks: chunks.chunks,
        map_page_info,
        objects: objects.objects,
        object_page_info,
        champions,
        towns,
        battle: None,
        battle_summary: None,
        events: events.events,
        event_page_info: events.page_info,
        content_manifest_hash: domm_game::first_playable_content_manifest().computed_content_hash(),
        render_time,
        action_affordances,
    })
}

pub(crate) fn get_visible_map_chunks(
    caller: CandidPrincipal,
    session_id: String,
    viewport: Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<MapChunkPage, ApiError> {
    validate_viewport(&viewport)?;
    let context = session_context::require_session_caller(caller, &session_id)?;
    render_projection::visible_map_chunks(&context, &viewport, cursor, limit)
}

pub(crate) fn get_visible_objects(
    caller: CandidPrincipal,
    session_id: String,
    viewport: Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<ObjectViewPage, ApiError> {
    validate_viewport(&viewport)?;
    let context = session_context::require_session_caller(caller, &session_id)?;
    render_projection::visible_objects(&context, &viewport, cursor, limit)
}

pub(crate) fn get_my_champions(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<Vec<ChampionView>, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    render_projection::my_champions(&context)
}

pub(crate) fn get_champion_view(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
) -> Result<ChampionView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    render_projection::champion_view_by_id(&context, &champion_id)
}

struct EventPage {
    events: Vec<ApiEventView>,
    page_info: EventPageInfo,
}

fn opening_event_page(
    context: &session_context::SessionCallerContext,
    after_seq: u64,
    limit: u32,
) -> EventPage {
    let mut events = Vec::new();
    if after_seq < 1 && limit > 0 {
        events.push(ApiEventView {
            session_id: context.session.id().to_string(),
            event_seq: 1,
            event_key: "setup:complete".to_string(),
            audience_key: "public".to_string(),
            turn_number: context.session.current_turn,
            event_type: "session_started".to_string(),
            subject_kind: Some("session".to_string()),
            subject_id_text: Some(context.session.id().to_string()),
            payload: Some(format!(
                "{{\"ruleset\":\"{}\",\"version\":{}}}",
                domm_game::FIRST_PLAYABLE_RULESET_SLUG,
                domm_game::FIRST_PLAYABLE_RULESET_VERSION
            )),
            redacted: false,
        });
    }
    EventPage {
        page_info: EventPageInfo {
            next_event_seq: None,
            has_more: false,
            limit,
        },
        events,
    }
}

fn render_time_meta(session: &domm_degens_schema::schema::GameSession) -> RenderTimeMeta {
    let now_ms = timestamp_to_u64(Timestamp::now());
    let started_ms = timestamp_to_u64(session.turn_started_at);
    RenderTimeMeta {
        server_now_ms: now_ms,
        turn_started_at_ms: started_ms,
        turn_duration_ms: u64::from(session.turn_duration_ms),
        sync_required: now_ms >= timestamp_to_u64(session.turn_deadline_at),
    }
}

fn validate_game_view_request(request: &GameViewRequest) -> Result<(), ApiError> {
    validate_viewport(&request.viewport)?;
    foundation::validate_limit(
        "chunk_limit",
        request.chunk_limit,
        domm_game::MAX_CHUNK_LIMIT,
        "viewport_chunk_limit_exceeded",
    )?;
    foundation::validate_limit(
        "object_limit",
        request.object_limit,
        domm_game::MAX_OBJECT_LIMIT,
        "list_limit_exceeded",
    )?;
    foundation::validate_limit(
        "event_limit",
        request.event_limit,
        domm_game::MAX_EVENT_LIMIT,
        "event_limit_exceeded",
    )?;
    Ok(())
}

fn validate_viewport(viewport: &Viewport) -> Result<(), ApiError> {
    let tiles = u32::from(viewport.width) * u32::from(viewport.height);
    if tiles > domm_game::MAX_VIEWPORT_TILES {
        return Err(ApiError::new(
            "viewport_too_large",
            "viewport exceeds the v1 public query limit",
            false,
        )
        .with_details(format!(
            "{{\"tiles\":{},\"max\":{}}}",
            tiles,
            domm_game::MAX_VIEWPORT_TILES
        )));
    }
    Ok(())
}

fn timestamp_to_u64(timestamp: Timestamp) -> u64 {
    u64::try_from(timestamp.as_millis()).unwrap_or(0)
}
