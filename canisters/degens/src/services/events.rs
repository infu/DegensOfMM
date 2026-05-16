use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{GameCommand, GameEvent, LobbyCommand};
use domm_game::{
    ApiError, ApiEventPage, ApiEventView, CommandPhase, CommandStatus, CommandStatusView,
    EventPageInfo, MAX_LIST_LIMIT,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Principal, Ulid},
};
use sha2::{Digest, Sha256};

use crate::repos::{commands_events_effects, foundation};

use super::session_context::{self, public_error};

const LOBBY_COMMAND_TYPES: &[&str] = &[
    "register_player",
    "create_session",
    "join_session",
    "mark_ready",
    "start_session",
];

pub(crate) fn get_events_after(
    caller: CandidPrincipal,
    session_id: String,
    audience_key: String,
    events_after_seq: u64,
    limit: u32,
) -> Result<ApiEventPage, ApiError> {
    let limit = foundation::validate_list_limit(limit)?;
    let context = session_context::require_session_caller(caller, &session_id)?;
    authorize_audience(&context, &audience_key)?;

    let fetch_limit = limit.saturating_add(1).min(MAX_LIST_LIMIT);
    let mut events = commands_events_effects::events_after(
        context.session.id(),
        "public",
        events_after_seq,
        fetch_limit,
    )?;
    if audience_key != "public" {
        events.extend(commands_events_effects::events_after(
            context.session.id(),
            &audience_key,
            events_after_seq,
            fetch_limit,
        )?);
    }

    events.sort_by_key(|event| (event.event_seq, event.id()));
    events.dedup_by_key(|event| event.id());
    let has_more = events.len() > limit as usize;
    events.truncate(limit as usize);
    let views = events.into_iter().map(api_event_view).collect::<Vec<_>>();

    Ok(ApiEventPage {
        page_info: EventPageInfo {
            next_event_seq: has_more.then(|| {
                views
                    .last()
                    .map_or(events_after_seq, |event| event.event_seq)
            }),
            has_more,
            limit,
        },
        events: views,
    })
}

pub(crate) fn get_command_status(
    caller: CandidPrincipal,
    session_id: String,
    command_id_or_client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let actor_principal = Principal::from(caller);

    if let Some(status) =
        command_status_by_id(&context, actor_principal, &command_id_or_client_nonce)?
    {
        return Ok(status);
    }
    if let Some(command) =
        find_lobby_command_by_nonce(actor_principal, &context, &command_id_or_client_nonce)?
    {
        return Ok(lobby_status_view(command));
    }

    Err(public_error(
        "command_status_not_found",
        "command status was not found for this caller and session",
        false,
    ))
}

fn authorize_audience(
    context: &session_context::SessionCallerContext,
    audience_key: &str,
) -> Result<(), ApiError> {
    let participant_audience = format!("participant:{}", context.participant.id());
    if audience_key == "public" || audience_key == participant_audience {
        return Ok(());
    }
    Err(public_error(
        "audience_not_allowed",
        "caller cannot read this event audience",
        false,
    ))
}

fn command_status_by_id(
    context: &session_context::SessionCallerContext,
    actor_principal: Principal,
    value: &str,
) -> Result<Option<CommandStatusView>, ApiError> {
    let Ok(id) = Ulid::from_str(value) else {
        return Ok(None);
    };

    if let Some(command) =
        commands_events_effects::load_game_command(Id::<GameCommand>::from_key(id))?
    {
        if command.session_id != context.session.id().key() {
            return Ok(None);
        }
        return Ok(Some(game_status_view(command)));
    }

    let Some(command) =
        commands_events_effects::load_lobby_command(Id::<LobbyCommand>::from_key(id))?
    else {
        return Ok(None);
    };
    if !lobby_command_visible(actor_principal, context, &command) {
        return Ok(None);
    }
    Ok(Some(lobby_status_view(command)))
}

fn find_lobby_command_by_nonce(
    actor_principal: Principal,
    context: &session_context::SessionCallerContext,
    client_nonce: &str,
) -> Result<Option<LobbyCommand>, ApiError> {
    for command_type in LOBBY_COMMAND_TYPES {
        let nonce = nonce_u64(command_type, client_nonce);
        let Some(command) =
            commands_events_effects::find_lobby_command_by_idempotency(actor_principal, nonce)?
        else {
            continue;
        };
        if lobby_command_visible(actor_principal, context, &command) {
            return Ok(Some(command));
        }
    }
    Ok(None)
}

fn lobby_command_visible(
    actor_principal: Principal,
    context: &session_context::SessionCallerContext,
    command: &LobbyCommand,
) -> bool {
    if command.actor_principal != actor_principal {
        return false;
    }
    json_string_field(command.result_json.as_deref(), "session_id")
        .map(|session_id| session_id == context.session.id().to_string())
        .unwrap_or(true)
}

fn api_event_view(event: GameEvent) -> ApiEventView {
    ApiEventView {
        session_id: Id::<domm_degens_schema::schema::GameSession>::from_key(event.session_id)
            .to_string(),
        event_seq: event.event_seq,
        event_key: event.event_key,
        audience_key: event.audience_key,
        turn_number: event.turn_number,
        event_type: event.event_type,
        subject_kind: event.subject_kind,
        subject_id_text: event.subject_id_text,
        payload: Some(event.payload_json),
        redacted: false,
    }
}

fn game_status_view(command: GameCommand) -> CommandStatusView {
    CommandStatusView {
        command_id: command.id().to_string(),
        status: status_from_str(&command.status),
        phase: phase_from_str(&command.phase),
        retryable: command.retryable,
        error_code: command.error_code,
        error_message: command.error_message,
        result_json: command.result_json,
    }
}

fn lobby_status_view(command: LobbyCommand) -> CommandStatusView {
    CommandStatusView {
        command_id: command.id().to_string(),
        status: status_from_str(&command.status),
        phase: phase_from_str(&command.phase),
        retryable: command.retryable,
        error_code: command.error_code,
        error_message: command.error_message,
        result_json: command.result_json,
    }
}

fn status_from_str(value: &str) -> CommandStatus {
    match value {
        "pending" => CommandStatus::Pending,
        "validated" => CommandStatus::Applying,
        "applying" => CommandStatus::Applying,
        "applied" => CommandStatus::Applied,
        "failed" => CommandStatus::Failed,
        _ => CommandStatus::Failed,
    }
}

fn phase_from_str(value: &str) -> CommandPhase {
    match value {
        "created" => CommandPhase::Created,
        "validated" => CommandPhase::Validated,
        "applying" => CommandPhase::Applying,
        "effects_applied" => CommandPhase::EffectsApplied,
        "events_applied" => CommandPhase::EventsApplied,
        "recovered" => CommandPhase::Recovered,
        "complete" => CommandPhase::Complete,
        "failed" => CommandPhase::Failed,
        _ => CommandPhase::Failed,
    }
}

fn json_string_field(json: Option<&str>, field: &str) -> Option<String> {
    let json = json?;
    let needle = format!(r#""{field}":""#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn nonce_u64(command_type: &str, client_nonce: &str) -> u64 {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.len().to_be_bytes());
    hasher.update(value.as_bytes());
    hasher.update([0xff]);
}
