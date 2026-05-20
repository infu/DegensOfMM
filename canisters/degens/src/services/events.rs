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

use super::{
    account_lobby_session, battle_runtime,
    session_context::{self, public_error},
    session_turn_runtime,
};

const LOBBY_COMMAND_TYPES: &[&str] = &[
    "register_player",
    "create_session",
    "join_session",
    "mark_ready",
    "start_session",
];
const GAME_COMMAND_TYPES: &[&str] = &[
    "submit_move_intent",
    "end_turn",
    "sync_session_turn",
    "submit_build_town_structure",
    "submit_recruit_units",
    "select_champion_level_up",
    "learn_champion_spell",
    "cast_adventure_spell",
    "hire_tavern_champion",
    "submit_market_trade",
    "submit_dwelling_recruit",
    "accept_quest",
    "claim_quest_reward",
    "sync_objectives",
    "sync_world_events",
    "sync_advanced_victory",
    "sync_world_generation",
    "sync_battle",
    "end_battle_turn",
    "submit_battle_action",
];

pub(crate) fn get_events_after(
    caller: CandidPrincipal,
    session_id: String,
    audience_key: String,
    events_after_seq: u64,
    limit: u32,
) -> Result<ApiEventPage, ApiError> {
    let limit = foundation::validate_list_limit(limit)?;
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    authorize_audience(&context, &audience_key)?;

    let fetch_limit = limit.saturating_add(1).min(MAX_LIST_LIMIT);
    let mut durable_events = commands_events_effects::events_after(
        context.session.id(),
        "public",
        events_after_seq,
        fetch_limit,
    )?;
    if audience_key != "public" {
        durable_events.extend(commands_events_effects::events_after(
            context.session.id(),
            &audience_key,
            events_after_seq,
            fetch_limit,
        )?);
    }

    durable_events.sort_by_key(|event| (event.event_seq, event.id()));
    durable_events.dedup_by_key(|event| event.id());
    let mut views = durable_events
        .into_iter()
        .map(api_event_view)
        .collect::<Vec<_>>();
    let canonical_session_id = context.session.id().to_string();
    views.extend(session_turn_runtime::active_events_after(
        &canonical_session_id,
        "public",
        events_after_seq,
    ));
    if audience_key != "public" {
        views.extend(session_turn_runtime::active_events_after(
            &canonical_session_id,
            &audience_key,
            events_after_seq,
        ));
    }
    views.extend(battle_runtime::active_events_after(
        &canonical_session_id,
        "public",
        events_after_seq,
    ));
    if audience_key != "public" {
        views.extend(battle_runtime::active_events_after(
            &canonical_session_id,
            &audience_key,
            events_after_seq,
        ));
    }

    views.sort_by(|left, right| {
        (
            left.event_seq,
            left.event_key.as_str(),
            left.audience_key.as_str(),
        )
            .cmp(&(
                right.event_seq,
                right.event_key.as_str(),
                right.audience_key.as_str(),
            ))
    });
    views.dedup_by(|left, right| {
        left.event_key == right.event_key && left.audience_key == right.audience_key
    });
    let has_more = views.len() > limit as usize;
    views.truncate(limit as usize);

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
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let actor_principal = Principal::from(caller);

    if let Some(status) =
        command_status_by_id(&context, actor_principal, &command_id_or_client_nonce)?
    {
        return Ok(status);
    }
    let lobby_candidates = lobby_command_type_candidates(&command_id_or_client_nonce);
    let game_candidates = game_command_type_candidates(&command_id_or_client_nonce);
    if lobby_candidates.is_empty() {
        if let Some(status) = runtime_game_command_status_by_nonce(
            &context,
            &command_id_or_client_nonce,
            game_candidates,
        ) {
            return Ok(status);
        }
        if let Some(command) =
            find_game_command_by_nonce(&context, &command_id_or_client_nonce, game_candidates)?
        {
            return Ok(game_status_view(command));
        }
    } else if game_candidates.is_empty() {
        if let Some(command) = find_lobby_command_by_nonce(
            actor_principal,
            &context,
            &command_id_or_client_nonce,
            lobby_candidates,
        )? {
            return Ok(lobby_status_view(command));
        }
    } else {
        if let Some(command) = find_lobby_command_by_nonce(
            actor_principal,
            &context,
            &command_id_or_client_nonce,
            lobby_candidates,
        )? {
            return Ok(lobby_status_view(command));
        }
        if let Some(status) = runtime_game_command_status_by_nonce(
            &context,
            &command_id_or_client_nonce,
            game_candidates,
        ) {
            return Ok(status);
        }
        if let Some(command) =
            find_game_command_by_nonce(&context, &command_id_or_client_nonce, game_candidates)?
        {
            return Ok(game_status_view(command));
        }
    }

    Err(public_error(
        "command_status_not_found",
        "command status was not found for this caller and session",
        false,
    ))
}

pub(crate) fn get_command_status_by_nonce(
    caller: CandidPrincipal,
    session_id: String,
    command_type: String,
    client_nonce: String,
) -> Result<CommandStatusView, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let actor_principal = Principal::from(caller);
    if LOBBY_COMMAND_TYPES.contains(&command_type.as_str()) {
        let command = find_lobby_command_by_nonce(
            actor_principal,
            &context,
            &client_nonce,
            &[command_type.as_str()],
        )?;
        return command.map(lobby_status_view).ok_or_else(|| {
            public_error(
                "command_status_not_found",
                "lobby command status was not found for this caller and nonce",
                false,
            )
        });
    }

    if !GAME_COMMAND_TYPES.contains(&command_type.as_str()) {
        return Err(public_error(
            "unknown_command_type",
            "command_type is not part of the v1.1 command status contract",
            false,
        ));
    }

    if let Some(status) =
        runtime_game_command_status_by_nonce(&context, &client_nonce, &[command_type.as_str()])
    {
        return Ok(status);
    }

    let command = find_game_command_by_nonce(&context, &client_nonce, &[command_type.as_str()])?;
    command.map(game_status_view).ok_or_else(|| {
        public_error(
            "command_status_not_found",
            "game command status was not found for this participant and nonce",
            false,
        )
    })
}

fn runtime_game_command_status_by_nonce(
    context: &session_context::SessionCallerContext,
    client_nonce: &str,
    command_types: &[&str],
) -> Option<CommandStatusView> {
    let canonical_session_id = context.session.id().to_string();
    let actor_participant_id = context.participant.id().to_string();
    for command_type in command_types {
        let nonce = nonce_u64(command_type, client_nonce);
        if matches!(
            *command_type,
            "submit_move_intent" | "end_turn" | "sync_session_turn"
        ) && let Some(receipt) = session_turn_runtime::command_receipt_by_nonce(
            &canonical_session_id,
            &actor_participant_id,
            nonce,
        ) {
            return Some(receipt.status_view());
        }
        if *command_type == "submit_battle_action"
            && let Some(receipt) = battle_runtime::command_receipt_by_nonce(
                &canonical_session_id,
                &actor_participant_id,
                nonce,
            )
        {
            return Some(receipt.status_view());
        }
    }
    None
}

fn find_game_command_by_nonce(
    context: &session_context::SessionCallerContext,
    client_nonce: &str,
    command_types: &[&str],
) -> Result<Option<GameCommand>, ApiError> {
    for command_type in command_types {
        let nonce = nonce_u64(command_type, client_nonce);
        let Some(command) = commands_events_effects::find_game_command_by_idempotency(
            context.session.id(),
            "player",
            &context.participant.id().to_string(),
            nonce,
        )?
        else {
            continue;
        };
        return Ok(Some(command));
    }
    Ok(None)
}

fn game_command_type_candidates(client_nonce: &str) -> &'static [&'static str] {
    if client_nonce.contains("move") {
        &["submit_move_intent"]
    } else if client_nonce.contains("end-turn") {
        &["end_turn"]
    } else if client_nonce.contains("sync-turn") || client_nonce.contains("income") {
        &["sync_session_turn"]
    } else if client_nonce.contains("build") {
        &["submit_build_town_structure"]
    } else if client_nonce.contains("dwelling") {
        &["submit_dwelling_recruit"]
    } else if client_nonce.contains("recruit") {
        &["submit_recruit_units"]
    } else if client_nonce.contains("skill") {
        &["select_champion_level_up"]
    } else if client_nonce.contains("learn") {
        &["learn_champion_spell"]
    } else if client_nonce.contains("cast") {
        &["cast_adventure_spell"]
    } else if client_nonce.contains("hire") {
        &["hire_tavern_champion"]
    } else if client_nonce.contains("market") {
        &["submit_market_trade"]
    } else if client_nonce.contains("quest") && client_nonce.contains("claim") {
        &["claim_quest_reward"]
    } else if client_nonce.contains("quest") || client_nonce.contains("accept") {
        &["accept_quest"]
    } else if client_nonce.contains("objective") {
        &["sync_objectives"]
    } else if client_nonce.contains("world-event") || client_nonce.contains("event") {
        &["sync_world_events"]
    } else if client_nonce.contains("worldgen") || client_nonce.contains("generation") {
        &["sync_world_generation"]
    } else if client_nonce.contains("victory") {
        &["sync_advanced_victory"]
    } else if client_nonce.contains("battle-action") {
        &["submit_battle_action"]
    } else if client_nonce.contains("end-battle") {
        &["end_battle_turn"]
    } else if client_nonce.contains("sync-battle") {
        &["sync_battle"]
    } else if lobby_command_type_candidates(client_nonce).is_empty() {
        GAME_COMMAND_TYPES
    } else {
        &[]
    }
}

fn lobby_command_type_candidates(client_nonce: &str) -> &'static [&'static str] {
    if client_nonce.contains("register") {
        &["register_player"]
    } else if client_nonce.contains("create") {
        &["create_session"]
    } else if client_nonce.contains("join") {
        &["join_session"]
    } else if client_nonce.contains("ready") {
        &["mark_ready"]
    } else if client_nonce.contains("start") {
        &["start_session"]
    } else if game_command_type_candidates_without_lobby_fallback(client_nonce).is_empty() {
        LOBBY_COMMAND_TYPES
    } else {
        &[]
    }
}

fn game_command_type_candidates_without_lobby_fallback(
    client_nonce: &str,
) -> &'static [&'static str] {
    if client_nonce.contains("move") {
        &["submit_move_intent"]
    } else if client_nonce.contains("end-turn") {
        &["end_turn"]
    } else if client_nonce.contains("sync-turn") || client_nonce.contains("income") {
        &["sync_session_turn"]
    } else if client_nonce.contains("build") {
        &["submit_build_town_structure"]
    } else if client_nonce.contains("dwelling") {
        &["submit_dwelling_recruit"]
    } else if client_nonce.contains("recruit") {
        &["submit_recruit_units"]
    } else if client_nonce.contains("skill") {
        &["select_champion_level_up"]
    } else if client_nonce.contains("learn") {
        &["learn_champion_spell"]
    } else if client_nonce.contains("cast") {
        &["cast_adventure_spell"]
    } else if client_nonce.contains("hire") {
        &["hire_tavern_champion"]
    } else if client_nonce.contains("market") {
        &["submit_market_trade"]
    } else if client_nonce.contains("quest") && client_nonce.contains("claim") {
        &["claim_quest_reward"]
    } else if client_nonce.contains("quest") || client_nonce.contains("accept") {
        &["accept_quest"]
    } else if client_nonce.contains("objective") {
        &["sync_objectives"]
    } else if client_nonce.contains("world-event") || client_nonce.contains("event") {
        &["sync_world_events"]
    } else if client_nonce.contains("victory") {
        &["sync_advanced_victory"]
    } else if client_nonce.contains("battle-action") {
        &["submit_battle_action"]
    } else if client_nonce.contains("end-battle") {
        &["end_battle_turn"]
    } else if client_nonce.contains("sync-battle") {
        &["sync_battle"]
    } else {
        &[]
    }
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
    let canonical_session_id = context.session.id().to_string();
    if let Some(receipt) = session_turn_runtime::command_receipt_by_id(&canonical_session_id, value)
    {
        return Ok(Some(receipt.status_view()));
    }
    if let Some(receipt) = battle_runtime::command_receipt_by_id(&canonical_session_id, value) {
        return Ok(Some(receipt.status_view()));
    }

    let Ok(id) = Ulid::from_str(value) else {
        return Ok(None);
    };

    if let Some(command) = account_lobby_session::runtime_lobby_command_by_id(value) {
        if !lobby_command_visible(actor_principal, context, &command) {
            return Ok(None);
        }
        return Ok(Some(lobby_status_view(command)));
    }

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
    command_types: &[&str],
) -> Result<Option<LobbyCommand>, ApiError> {
    for command_type in command_types {
        let nonce = nonce_u64(command_type, client_nonce);
        if let Some(command) =
            account_lobby_session::runtime_lobby_command_by_idempotency(actor_principal, nonce)
        {
            if lobby_command_visible(actor_principal, context, &command) {
                return Ok(Some(command));
            }
        }
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
