use candid::Principal;

use crate::battle::BattleView;
use crate::command::EventView;
use crate::content::first_playable_content_manifest;
use crate::fixtures::TURN_DURATION_MS;
use crate::limits::{
    DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT, MAX_MAP_HEIGHT, MAX_MAP_WIDTH,
    MAX_RECENT_EVENTS_IN_GAME_VIEW, MAX_VIEWPORT_CHUNKS_PER_REQUEST,
};
use crate::map::{
    OPENING_VIEWPORT_EAST_X, OPENING_VIEWPORT_EAST_Y, OPENING_VIEWPORT_HEIGHT,
    OPENING_VIEWPORT_WEST_X, OPENING_VIEWPORT_WEST_Y, OPENING_VIEWPORT_WIDTH, ObjectView, Viewport,
};
use crate::strategic::StrategicFixtureBackend;

use super::types::{
    ActionAffordance, ApiError, ApiEventView, ApiTownView, BattleSummary, EventPageInfo, GameView,
    GameViewRequest, PageInfo, ParticipantSummary, RenderTimeMeta, SessionSummary,
};

pub const DEFAULT_CHUNK_LIMIT: u32 = MAX_VIEWPORT_CHUNKS_PER_REQUEST;
pub const DEFAULT_OBJECT_LIMIT: u32 = DEFAULT_LIST_LIMIT;
pub const DEFAULT_EVENT_LIMIT: u32 = MAX_RECENT_EVENTS_IN_GAME_VIEW;
pub const MAX_VIEWPORT_TILES: u32 = (MAX_MAP_WIDTH as u32) * (MAX_MAP_HEIGHT as u32);
pub const MAX_CHUNK_LIMIT: u32 = MAX_VIEWPORT_CHUNKS_PER_REQUEST;
pub const MAX_OBJECT_LIMIT: u32 = MAX_LIST_LIMIT;
pub const MAX_EVENT_LIMIT: u32 = MAX_RECENT_EVENTS_IN_GAME_VIEW;

impl GameViewRequest {
    #[must_use]
    pub fn opening_for_slot(slot_index: u8) -> Self {
        Self {
            viewport: opening_viewport_for_slot(slot_index),
            chunk_cursor: None,
            chunk_limit: DEFAULT_CHUNK_LIMIT,
            object_cursor: None,
            object_limit: DEFAULT_OBJECT_LIMIT,
            events_after_seq: 0,
            event_limit: DEFAULT_EVENT_LIMIT,
            include_battle: true,
        }
    }
}

#[must_use]
pub fn opening_viewport_for_slot(slot_index: u8) -> Viewport {
    match slot_index {
        1 => Viewport::new(
            OPENING_VIEWPORT_EAST_X,
            OPENING_VIEWPORT_EAST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        ),
        _ => Viewport::new(
            OPENING_VIEWPORT_WEST_X,
            OPENING_VIEWPORT_WEST_Y,
            OPENING_VIEWPORT_WIDTH,
            OPENING_VIEWPORT_HEIGHT,
        ),
    }
}

pub fn build_game_view(
    strategic: &mut StrategicFixtureBackend,
    caller: Principal,
    session_id: &str,
    request: &GameViewRequest,
    server_now_ms: u64,
    battle: Option<BattleView>,
    api_events: &[ApiEventView],
) -> Result<GameView, ApiError> {
    validate_request(request)?;
    let session = strategic
        .get_session_public(session_id)
        .map_err(|error| map_api_error("get_session_failed", error))?;
    let participant = strategic
        .get_my_participant_public(caller, session_id)
        .map_err(|error| map_api_error("get_participant_failed", error))?;
    let participant_id = participant.participant_id.clone();
    let chunks = strategic
        .visible_map_chunks_public(
            caller,
            session_id,
            &request.viewport,
            request.chunk_cursor,
            request.chunk_limit,
        )
        .map_err(|error| map_api_error("visible_chunks_failed", error))?;
    let objects = strategic
        .visible_objects_public(
            caller,
            session_id,
            &request.viewport,
            request.object_cursor,
            request.object_limit,
        )
        .map_err(|error| map_api_error("visible_objects_failed", error))?;
    let champions = strategic
        .my_champions_public(caller, session_id)
        .map_err(|error| map_api_error("champions_failed", error))?;
    let event_limit = request.event_limit;
    let lifecycle_events = strategic
        .get_events_public(
            caller,
            session_id,
            request.events_after_seq,
            event_limit as usize,
        )
        .map_err(|error| map_api_error("events_failed", error))?;
    let events = merged_events(
        lifecycle_events
            .events
            .iter()
            .map(api_event_from_command_event)
            .collect(),
        api_events,
        session_id,
        &participant_id,
        request.events_after_seq,
        event_limit,
    );
    let event_page_info = EventPageInfo {
        next_event_seq: events.has_more.then(|| {
            events
                .events
                .last()
                .map_or(request.events_after_seq, |event| event.event_seq)
        }),
        has_more: events.has_more,
        limit: event_limit,
    };
    let content_manifest = first_playable_content_manifest();
    let turn_started_at_ms = strategic.turn_started_at();
    let render_time = RenderTimeMeta {
        server_now_ms,
        turn_started_at_ms,
        turn_duration_ms: TURN_DURATION_MS,
        sync_required: server_now_ms >= turn_started_at_ms.saturating_add(TURN_DURATION_MS),
    };
    let battle_summary = battle.as_ref().map(BattleSummary::from);
    let towns = town_views_for_visible_objects(strategic, &participant_id, &objects.objects)?;
    let action_affordances = action_affordances(&champions, &towns, battle.as_ref());

    Ok(GameView {
        session: SessionSummary::from_session(session, strategic.current_turn()),
        participant: ParticipantSummary::from(participant),
        viewport: request.viewport.clone(),
        map_chunks: chunks.chunks,
        map_page_info: PageInfo {
            next_cursor: chunks.next_cursor,
            has_more: chunks.has_more,
            limit: request.chunk_limit,
        },
        objects: objects.objects,
        object_page_info: PageInfo {
            next_cursor: objects.next_cursor,
            has_more: objects.has_more,
            limit: request.object_limit,
        },
        champions,
        towns,
        battle,
        battle_summary,
        events: events.events,
        event_page_info,
        content_manifest_hash: content_manifest.computed_content_hash(),
        render_time,
        action_affordances,
        omitted_fields: Vec::new(),
    })
}

pub fn api_event_from_command_event(event: &EventView) -> ApiEventView {
    ApiEventView {
        session_id: event.session_id.clone(),
        event_seq: event.event_seq,
        event_key: event.event_key.clone(),
        audience_key: event.audience_key.clone(),
        turn_number: event.turn_number,
        event_type: event.event_type.clone(),
        subject_kind: event.subject_kind.clone(),
        subject_id_text: event.subject_id_text.clone(),
        payload: event.payload_json.clone(),
        redacted: event.redacted,
    }
}

#[must_use]
pub fn participant_audience_key(participant_id: &str) -> String {
    format!("participant:{participant_id}")
}

#[must_use]
pub fn deliver_event_to_audience(event: &ApiEventView, audience_key: &str) -> ApiEventView {
    let allowed = event.audience_key == "public" || event.audience_key == audience_key;
    let mut delivered = event.clone();
    delivered.payload = allowed.then(|| event.payload.clone()).flatten();
    delivered.redacted = !allowed;
    delivered
}

pub fn map_api_error(code: &str, error: impl ToString) -> ApiError {
    ApiError::new(code, error.to_string(), false)
}

struct EventMerge {
    events: Vec<ApiEventView>,
    has_more: bool,
}

fn merged_events(
    lifecycle_events: Vec<ApiEventView>,
    api_events: &[ApiEventView],
    session_id: &str,
    participant_id: &str,
    after_seq: u64,
    limit: u32,
) -> EventMerge {
    let audience_key = participant_audience_key(participant_id);
    let api_start_index = api_events.partition_point(|event| event.event_seq <= after_seq);
    let mut events = lifecycle_events
        .into_iter()
        .chain(
            api_events[api_start_index..]
                .iter()
                .filter(|event| event.session_id == session_id)
                .filter(|event| event.event_seq > after_seq)
                .take(limit.saturating_add(1) as usize)
                .map(|event| deliver_event_to_audience(event, &audience_key)),
        )
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.event_seq);
    let limit = limit.max(1) as usize;
    let has_more = events.len() > limit;
    events.truncate(limit);

    EventMerge { events, has_more }
}

fn town_views_for_visible_objects(
    strategic: &StrategicFixtureBackend,
    participant_id: &str,
    objects: &[ObjectView],
) -> Result<Vec<ApiTownView>, ApiError> {
    let state = strategic
        .export_aftermath_state()
        .map_err(|error| map_api_error("town_view_failed", error))?;
    let visible_town_ids = objects
        .iter()
        .filter(|object| object.subject_kind == "town")
        .map(|object| object.subject_id_text.as_str())
        .collect::<Vec<_>>();
    let towns = state
        .town
        .towns
        .iter()
        .filter(|town| {
            town.owner_participant_id == participant_id
                || visible_town_ids
                    .iter()
                    .any(|town_id| *town_id == town.town_id.as_str())
        })
        .map(|town| ApiTownView {
            town: town.clone(),
            buildings: state
                .town
                .buildings
                .iter()
                .filter(|building| building.town_id == town.town_id)
                .cloned()
                .collect(),
            recruit_pools: state
                .town
                .recruit_pools
                .iter()
                .filter(|pool| pool.town_id == town.town_id)
                .cloned()
                .collect(),
            garrison_stacks: state
                .town
                .garrison_stacks
                .iter()
                .filter(|stack| stack.owner_id == town.town_id)
                .cloned()
                .collect(),
        })
        .collect();
    Ok(towns)
}

fn action_affordances(
    champions: &[crate::champion::ChampionView],
    towns: &[ApiTownView],
    battle: Option<&BattleView>,
) -> Vec<ActionAffordance> {
    let mut actions = Vec::new();
    actions.extend(champions.iter().map(|champion| ActionAffordance {
        action: "submit_move_intent".to_string(),
        enabled: champion.status == "active",
        target_id: Some(champion.champion_id.clone()),
        disabled_reason: (champion.status != "active").then(|| champion.status.clone()),
    }));
    actions.extend(towns.iter().flat_map(|town| {
        [
            ActionAffordance {
                action: "submit_build_town_structure".to_string(),
                enabled: town.town.status == "active",
                target_id: Some(town.town.town_id.clone()),
                disabled_reason: (town.town.status != "active").then(|| town.town.status.clone()),
            },
            ActionAffordance {
                action: "submit_recruit_units".to_string(),
                enabled: town.town.status == "active",
                target_id: Some(town.town.town_id.clone()),
                disabled_reason: (town.town.status != "active").then(|| town.town.status.clone()),
            },
        ]
    }));
    if let Some(battle) = battle {
        actions.extend(
            battle
                .legal_actions_for_caller
                .iter()
                .map(|legal| ActionAffordance {
                    action: format!("battle:{}", legal.action),
                    enabled: legal.enabled,
                    target_id: legal.targets.first().cloned(),
                    disabled_reason: legal.disabled_reason.clone(),
                }),
        );
    }
    actions
}

fn validate_request(request: &GameViewRequest) -> Result<(), ApiError> {
    let tiles = u32::from(request.viewport.width) * u32::from(request.viewport.height);
    if tiles > MAX_VIEWPORT_TILES {
        return Err(ApiError::new(
            "viewport_too_large",
            "viewport exceeds the v1 public query limit",
            false,
        )
        .with_details(format!(r#"{{"tiles":{tiles}}}"#)));
    }
    validate_limit(
        "chunk_limit",
        request.chunk_limit,
        MAX_CHUNK_LIMIT,
        "viewport_chunk_limit_exceeded",
    )?;
    validate_limit(
        "object_limit",
        request.object_limit,
        MAX_OBJECT_LIMIT,
        "list_limit_exceeded",
    )?;
    validate_limit(
        "event_limit",
        request.event_limit,
        MAX_EVENT_LIMIT,
        "event_limit_exceeded",
    )?;
    Ok(())
}

fn validate_limit(name: &str, limit: u32, max: u32, code: &str) -> Result<(), ApiError> {
    if limit == 0 {
        return Err(ApiError::new(
            "limit_must_be_positive",
            format!("{name} must be at least 1"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    if limit > max {
        return Err(ApiError::new(
            code,
            format!("{name} exceeds the v1 public query limit"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    Ok(())
}
