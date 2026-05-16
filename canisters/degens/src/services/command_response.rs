use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{Champion, GameCommand, GameEvent, GameSession};
use domm_game::{
    ApiError, ApiEventView, BattleActionReceipt, BattleSyncOutcome, ChampionMagicReceipt,
    ChangedSubject, CommandPhase, CommandResponse, CommandResult, CommandStatus,
    StrategicCommandReceipt,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};
use sha2::{Digest, Sha256};

use crate::repos::{commands_events_effects, sessions};

use super::session_context::{SessionCallerContext, public_error};

pub(crate) enum GameCommandAction {
    Apply(GameCommand),
    Return(CommandResponse),
}

pub(crate) fn begin_participant_command(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    command_type: &str,
    client_nonce_text: &str,
    champion_id: Option<Id<Champion>>,
    payload_json: String,
) -> Result<GameCommandAction, ApiError> {
    if payload_json.len() > domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES {
        return Ok(GameCommandAction::Return(failed_response(
            caller,
            context,
            command_type,
            client_nonce_text,
            payload_hash(
                command_type,
                &context.participant.id().to_string(),
                client_nonce_text,
                &payload_json,
            ),
            public_error(
                "payload_too_large",
                "game command payload is too large",
                false,
            ),
        )));
    }

    let client_nonce = nonce_u64(command_type, client_nonce_text);
    let hash = payload_hash(
        command_type,
        &context.participant.id().to_string(),
        client_nonce_text,
        &payload_json,
    );
    if let Some(existing) = commands_events_effects::find_game_command_by_idempotency(
        context.session.id(),
        "player",
        &context.participant.id().to_string(),
        client_nonce,
    )? {
        if existing.payload_hash != hash {
            return Ok(GameCommandAction::Return(failed_response(
                caller,
                context,
                command_type,
                client_nonce_text,
                hash,
                public_error(
                    "duplicate_nonce_payload_mismatch",
                    format!("client nonce {client_nonce_text} was reused with a different payload"),
                    false,
                ),
            )));
        }
        if is_recoverable_movement_command(command_type)
            && matches!(existing.status.as_str(), "pending" | "applying")
        {
            return Ok(GameCommandAction::Apply(existing));
        }
        return response_from_command(caller, context, existing, client_nonce_text)
            .map(GameCommandAction::Return);
    }

    let command = commands_events_effects::create_game_command(
        context.session.id(),
        "player".to_string(),
        context.participant.id().to_string(),
        Some(Id::from_key(context.participant.player_id)),
        Some(context.participant.id()),
        champion_id,
        context.session.current_turn,
        client_nonce,
        command_type.to_string(),
        hash,
        payload_json,
    )?;
    Ok(GameCommandAction::Apply(command))
}

fn is_recoverable_movement_command(command_type: &str) -> bool {
    matches!(
        command_type,
        "submit_move_intent"
            | "sync_session_turn"
            | "submit_battle_action"
            | "sync_battle"
            | "select_champion_level_up"
            | "learn_champion_spell"
            | "cast_adventure_spell"
    )
}

pub(crate) fn apply_command(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    mut command: GameCommand,
    client_nonce_text: &str,
    result_json: String,
    events: Vec<ApiEventView>,
    changed_subjects: Vec<ChangedSubject>,
) -> Result<CommandResponse, ApiError> {
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(result_json.clone());
    command.error_code = None;
    command.error_message = None;
    command.error_details_json = None;
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    command.failed_at = None;
    let command = commands_events_effects::update_game_command(command)?;
    let result = CommandResult::StrategicReceipt(receipt_from_json(
        &command.command_type,
        &command.id().to_string(),
        context.session.current_turn,
        events.len() as u32,
        Some(&result_json),
    ));
    Ok(response_from_parts(
        caller,
        context,
        command,
        client_nonce_text,
        CommandStatus::Applied,
        CommandPhase::Complete,
        false,
        events,
        changed_subjects,
        result,
        None,
    ))
}

pub(crate) fn apply_command_with_result(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    mut command: GameCommand,
    client_nonce_text: &str,
    result_json: String,
    events: Vec<ApiEventView>,
    changed_subjects: Vec<ChangedSubject>,
    result: CommandResult,
) -> Result<CommandResponse, ApiError> {
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(result_json.clone());
    command.error_code = None;
    command.error_message = None;
    command.error_details_json = None;
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    command.failed_at = None;
    let command = commands_events_effects::update_game_command(command)?;
    Ok(response_from_parts(
        caller,
        context,
        command,
        client_nonce_text,
        CommandStatus::Applied,
        CommandPhase::Complete,
        false,
        events,
        changed_subjects,
        result,
        None,
    ))
}

pub(crate) fn fail_command(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    mut command: GameCommand,
    client_nonce_text: &str,
    error: ApiError,
) -> Result<CommandResponse, ApiError> {
    command.status = "failed".to_string();
    command.phase = "failed".to_string();
    command.result_json = None;
    command.error_code = Some(error.code.clone());
    command.error_message = Some(error.message.clone());
    command.error_details_json = error.details_json.clone();
    command.retryable = error.retryable;
    command.failed_at = Some(Timestamp::now());
    let command = commands_events_effects::update_game_command(command)?;
    Ok(response_from_parts(
        caller,
        context,
        command,
        client_nonce_text,
        CommandStatus::Failed,
        CommandPhase::Failed,
        error.retryable,
        Vec::new(),
        Vec::new(),
        CommandResult::None,
        Some(error),
    ))
}

pub(crate) fn append_public_event(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    event_key: String,
    event_type: String,
    subject_kind: Option<String>,
    subject_id_text: Option<String>,
    payload_json: String,
) -> Result<ApiEventView, ApiError> {
    let event = if let Some(event) =
        commands_events_effects::find_event_by_key(session.id(), &event_key)?
    {
        event
    } else {
        let event_seq = session.next_event_seq;
        let event = commands_events_effects::create_game_event(
            session.id(),
            Some(command_id),
            None,
            session.current_turn,
            event_seq,
            event_key,
            "public".to_string(),
            event_type,
            subject_kind,
            subject_id_text,
            payload_json,
        )?;
        session.next_event_seq = event_seq.saturating_add(1);
        *session = sessions::update_session(session.clone())?;
        event
    };
    Ok(api_event_view(event))
}

pub(crate) fn ensure_command_effect(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    effect_key: String,
    effect_type: String,
    target_kind: String,
    target_id_text: String,
    payload_json: String,
) -> Result<(), ApiError> {
    if commands_events_effects::find_command_effect(command_id, &effect_key)?.is_none() {
        commands_events_effects::create_applied_command_effect(
            session_id,
            command_id,
            effect_key,
            effect_type,
            target_kind,
            target_id_text,
            payload_json,
            Timestamp::now(),
        )?;
    }
    Ok(())
}

pub(crate) fn changed(kind: &str, id: &str, operation: &str) -> ChangedSubject {
    ChangedSubject {
        subject_kind: kind.to_string(),
        subject_id_text: id.to_string(),
        operation: operation.to_string(),
    }
}

pub(crate) fn result_json(command_kind: &str, current_turn: u32) -> String {
    format!(
        r#"{{"command_kind":"{}","current_turn":{},"command_count":1,"event_count":1}}"#,
        escape_json(command_kind),
        current_turn
    )
}

pub(crate) fn nonce_u64(command_type: &str, client_nonce: &str) -> u64 {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}

pub(crate) fn payload_hash(
    command_type: &str,
    actor_key: &str,
    client_nonce: &str,
    payload: &str,
) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "actor_key", actor_key);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    hash_text(&mut hasher, "payload", payload);
    to_hex(&hasher.finalize())
}

pub(crate) fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn response_from_command(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    command: GameCommand,
    client_nonce_text: &str,
) -> Result<CommandResponse, ApiError> {
    let status = status_from_str(&command.status);
    let phase = phase_from_str(&command.phase);
    let error = command.error_code.as_ref().map(|code| {
        ApiError::new(
            code.clone(),
            command
                .error_message
                .clone()
                .unwrap_or_else(|| code.clone()),
            command.retryable,
        )
    });
    let result = if status == CommandStatus::Applied {
        result_from_json(&command)
    } else {
        CommandResult::None
    };
    Ok(response_from_parts(
        caller,
        context,
        command,
        client_nonce_text,
        status,
        phase,
        false,
        Vec::new(),
        Vec::new(),
        result,
        error,
    ))
}

fn result_from_json(command: &GameCommand) -> CommandResult {
    match command.command_type.as_str() {
        "submit_battle_action" => CommandResult::BattleAction(battle_action_from_json(command)),
        "sync_battle" => CommandResult::BattleSync(battle_sync_from_json(command)),
        "select_champion_level_up" | "learn_champion_spell" | "cast_adventure_spell" => {
            CommandResult::ChampionMagic(champion_magic_from_json(command))
        }
        _ => CommandResult::StrategicReceipt(receipt_from_json(
            &command.command_type,
            &command.id().to_string(),
            command.turn_number,
            0,
            command.result_json.as_deref(),
        )),
    }
}

fn champion_magic_from_json(command: &GameCommand) -> ChampionMagicReceipt {
    let json = command.result_json.as_deref();
    ChampionMagicReceipt {
        command_id: command.id().to_string(),
        champion_id: json_string_field(json, "champion_id").unwrap_or_default(),
        action: json_string_field(json, "action").unwrap_or_else(|| command.command_type.clone()),
        skill_key: json_string_field(json, "skill_key"),
        spell_slug: json_string_field(json, "spell_slug"),
        mana_after: json_u32_field(json, "mana_after")
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0),
        movement_remaining_after: json_u32_field(json, "movement_remaining_after")
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0),
        status_keys: Vec::new(),
    }
}

fn battle_action_from_json(command: &GameCommand) -> BattleActionReceipt {
    let json = command.result_json.as_deref();
    BattleActionReceipt {
        command_id: command.id().to_string(),
        status: "applied".to_string(),
        current_round: json_u32_field(json, "current_round")
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0),
        active_stack_id: json_string_field(json, "active_stack_id"),
        event_seq: json_u64_field(json, "event_seq"),
    }
}

fn battle_sync_from_json(command: &GameCommand) -> BattleSyncOutcome {
    let json = command.result_json.as_deref();
    BattleSyncOutcome {
        battle_id: json_string_field(json, "battle_id").unwrap_or_default(),
        timeout_actions_applied: json_u32_field(json, "timeout_actions_applied").unwrap_or(0),
        recovered_commands: json_u32_field(json, "recovered_commands").unwrap_or(0),
        battle_sync_incomplete: json_bool_field(json, "battle_sync_incomplete").unwrap_or(false),
        active_stack_id: json_string_field(json, "active_stack_id"),
    }
}

fn response_from_parts(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    command: GameCommand,
    client_nonce_text: &str,
    status: CommandStatus,
    phase: CommandPhase,
    retryable: bool,
    events: Vec<ApiEventView>,
    changed_subjects: Vec<ChangedSubject>,
    result: CommandResult,
    error: Option<ApiError>,
) -> CommandResponse {
    CommandResponse {
        command_id: command.id().to_string(),
        command_type: command.command_type,
        actor_principal: caller,
        actor_participant_id: Some(context.participant.id().to_string()),
        client_nonce: client_nonce_text.to_string(),
        payload_hash: command.payload_hash,
        status,
        phase,
        retryable,
        effective_turn: context.session.current_turn,
        durable_turn: context.session.current_turn,
        events,
        changed_subjects,
        result,
        error,
    }
}

fn failed_response(
    caller: CandidPrincipal,
    context: &SessionCallerContext,
    command_type: &str,
    client_nonce_text: &str,
    hash: String,
    error: ApiError,
) -> CommandResponse {
    CommandResponse {
        command_id: format!(
            "command:game:{command_type}:{}",
            short_hash(client_nonce_text)
        ),
        command_type: command_type.to_string(),
        actor_principal: caller,
        actor_participant_id: Some(context.participant.id().to_string()),
        client_nonce: client_nonce_text.to_string(),
        payload_hash: hash,
        status: CommandStatus::Failed,
        phase: CommandPhase::Failed,
        retryable: error.retryable,
        effective_turn: context.session.current_turn,
        durable_turn: context.session.current_turn,
        events: Vec::new(),
        changed_subjects: Vec::new(),
        result: CommandResult::None,
        error: Some(error),
    }
}

fn receipt_from_json(
    command_type: &str,
    command_id: &str,
    fallback_turn: u32,
    fallback_event_count: u32,
    json: Option<&str>,
) -> StrategicCommandReceipt {
    StrategicCommandReceipt {
        command_kind: json_string_field(json, "command_kind")
            .unwrap_or_else(|| command_type.to_string()),
        command_id: command_id.to_string(),
        current_turn: json_u32_field(json, "current_turn").unwrap_or(fallback_turn),
        command_count: json_u32_field(json, "command_count").unwrap_or(1),
        event_count: json_u32_field(json, "event_count").unwrap_or(fallback_event_count),
    }
}

fn api_event_view(event: GameEvent) -> ApiEventView {
    ApiEventView {
        session_id: Id::<GameSession>::from_key(event.session_id).to_string(),
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

fn status_from_str(value: &str) -> CommandStatus {
    match value {
        "pending" => CommandStatus::Pending,
        "validated" | "applying" => CommandStatus::Applying,
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
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_u32_field(json: Option<&str>, field: &str) -> Option<u32> {
    let json = json?;
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    rest.get(..end)?.parse().ok()
}

fn json_u64_field(json: Option<&str>, field: &str) -> Option<u64> {
    let json = json?;
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    rest.get(..end)?.parse().ok()
}

fn json_bool_field(json: Option<&str>, field: &str) -> Option<bool> {
    let json = json?;
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn short_hash(text: &str) -> String {
    payload_hash("short", "api", text, "")
        .chars()
        .take(16)
        .collect()
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.len().to_be_bytes());
    hasher.update(value.as_bytes());
    hasher.update([0xff]);
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
