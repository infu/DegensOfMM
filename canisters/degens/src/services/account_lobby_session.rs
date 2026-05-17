use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    FactionDefinition, GameCommand, GameEvent, GameParticipant, GameSession, LobbyCommand,
    PlayerAccount, RulesetDefinition, SystemJob,
};
use domm_game::{
    ACTIVE_SESSION_LIMIT, ApiError, ApiEventView, ChangedSubject, CommandPhase, CommandStatus,
    FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH, FIRST_PLAYABLE_PLAYER_COUNT,
    FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION,
    LobbyCommandResponse, LobbyCommandResult, MAX_LIST_LIMIT, ParticipantView, PlayerView,
    ResourceCost, SessionView, TURN_DURATION_MS, first_playable_content_manifest,
    first_playable_scenario,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Principal, Timestamp, Ulid},
};
use sha2::{Digest, Sha256};

use crate::repos::{
    aftermath_history, commands_events_effects, content, foundation, players, sessions,
    system_jobs as system_job_repo,
};

use super::{first_playable_setup, system_jobs as system_job_service};

const ACTIVE_SESSION_STATES: &[&str] = &["lobby", "starting", "active"];
const SETUP_SYSTEM_ACTOR: &str = "setup";
const SETUP_EFFECTS: &[SetupEffectSpec] = &[
    SetupEffectSpec {
        key: "seed_ruleset_content",
        effect_type: "ruleset_content",
        target_kind: "ruleset",
    },
    SetupEffectSpec {
        key: "seed_participants",
        effect_type: "participants",
        target_kind: "participant",
    },
    SetupEffectSpec {
        key: "seed_towns",
        effect_type: "towns",
        target_kind: "town",
    },
    SetupEffectSpec {
        key: "seed_champions",
        effect_type: "champions",
        target_kind: "champion",
    },
    SetupEffectSpec {
        key: "seed_map_chunks",
        effect_type: "map_chunks",
        target_kind: "map_chunk",
    },
    SetupEffectSpec {
        key: "seed_occupancy",
        effect_type: "occupancy",
        target_kind: "map_occupancy",
    },
    SetupEffectSpec {
        key: "seed_visibility",
        effect_type: "visibility",
        target_kind: "visibility_chunk",
    },
    SetupEffectSpec {
        key: "seed_neutrals",
        effect_type: "neutral_armies",
        target_kind: "neutral_army",
    },
    SetupEffectSpec {
        key: "seed_world_objects",
        effect_type: "world_objects",
        target_kind: "world_object",
    },
    SetupEffectSpec {
        key: "seed_resource_piles",
        effect_type: "resource_piles",
        target_kind: "world_object",
    },
    SetupEffectSpec {
        key: "seed_external_dwellings",
        effect_type: "external_dwellings",
        target_kind: "world_object",
    },
    SetupEffectSpec {
        key: "seed_dwelling_pools",
        effect_type: "dwelling_pools",
        target_kind: "dwelling_pool",
    },
    SetupEffectSpec {
        key: "seed_economy",
        effect_type: "economy",
        target_kind: "resource_summary",
    },
    SetupEffectSpec {
        key: "seed_tavern_offers",
        effect_type: "tavern_offers",
        target_kind: "tavern_offer",
    },
    SetupEffectSpec {
        key: "seed_scenario_progress",
        effect_type: "scenario_progress",
        target_kind: "scenario_rule",
    },
    SetupEffectSpec {
        key: "seed_worldgen",
        effect_type: "world_generation",
        target_kind: "procedural_map",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SetupEffectSpec {
    key: &'static str,
    effect_type: &'static str,
    target_kind: &'static str,
}

enum LobbyCommandAction {
    Apply(LobbyCommand),
    Return(LobbyCommandResponse),
}

pub(crate) fn register_player(
    caller: CandidPrincipal,
    username: Option<String>,
    display_name: Option<String>,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    reject_anonymous(caller)?;
    let actor_principal = Principal::from(caller);
    let display_name = display_name
        .or_else(|| username.clone())
        .unwrap_or_else(|| "Degens Player".to_string());
    let payload_json = format!(
        r#"{{"username":{},"display_name":"{}"}}"#,
        option_json(username.as_deref()),
        escape_json(&display_name)
    );
    let mut command = match begin_lobby_command(
        actor_principal,
        None,
        "register_player",
        &client_nonce,
        payload_json,
    )? {
        LobbyCommandAction::Apply(command) => command,
        LobbyCommandAction::Return(response) => return Ok(response),
    };

    let player = match players::find_by_principal(actor_principal)? {
        Some(player) => player,
        None => players::create_player_account(
            actor_principal,
            command_username(&username, caller),
            Some(display_name),
        )?,
    };
    command.actor_player_id = Some(player.id);
    let player_view = player_view(&player);
    apply_lobby_command(
        command,
        &client_nonce,
        Some(format!(r#"{{"player_id":"{}"}}"#, player.id())),
        Vec::new(),
        vec![changed("player", &player_view.player_id, "upsert")],
        LobbyCommandResult::Player(player_view),
        0,
    )
}

pub(crate) fn get_my_player(caller: CandidPrincipal) -> Result<PlayerView, ApiError> {
    reject_anonymous(caller)?;
    let player = require_player(caller)?;
    Ok(player_view(&player))
}

pub(crate) fn create_session(
    caller: CandidPrincipal,
    name: String,
    ruleset_id: String,
    seed: u64,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    reject_anonymous(caller)?;
    let actor_principal = Principal::from(caller);
    let player = players::find_by_principal(actor_principal)?;
    let actor_player_id = player.as_ref().map(EntityValue::id);
    let payload_json = format!(
        r#"{{"name":"{}","ruleset_id":"{}","seed":{seed}}}"#,
        escape_json(&name),
        escape_json(&ruleset_id)
    );
    let mut command = match begin_lobby_command(
        actor_principal,
        actor_player_id,
        "create_session",
        &client_nonce,
        payload_json,
    )? {
        LobbyCommandAction::Apply(command) => command,
        LobbyCommandAction::Return(response) => return Ok(response),
    };
    let Some(player) = player else {
        return fail_lobby_command(
            command,
            &client_nonce,
            public_error(
                "player_not_registered",
                "register a player before creating a session",
                false,
            ),
            0,
        );
    };
    command.actor_player_id = Some(player.id);

    let ruleset = match ensure_first_playable_content() {
        Ok((ruleset, _)) if is_first_playable_ruleset_arg(&ruleset_id, &ruleset) => ruleset,
        Ok(_) => {
            return fail_lobby_command(
                command,
                &client_nonce,
                public_error("ruleset_not_found", "ruleset is not available", false),
                0,
            );
        }
        Err(error) => return fail_lobby_command(command, &client_nonce, error, 0),
    };

    if player_has_live_session(player.id())? {
        return fail_lobby_command(
            command,
            &client_nonce,
            public_error(
                "active_session_limit",
                "player already has an active or lobby session",
                false,
            ),
            0,
        );
    }
    if active_session_count()? >= ACTIVE_SESSION_LIMIT {
        return fail_lobby_command(
            command,
            &client_nonce,
            public_error(
                "canister_active_session_limit_reached",
                "canister active session limit reached",
                false,
            ),
            0,
        );
    }

    let deadline = turn_deadline();
    let session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        name,
        seed,
        FIRST_PLAYABLE_MAP_WIDTH,
        FIRST_PLAYABLE_MAP_HEIGHT,
        deadline,
    )?;
    let faction = faction_for_slot(ruleset.id(), 0)?;
    let participant = sessions::create_participant(
        session.id(),
        player.id(),
        faction.id(),
        0,
        "red".to_string(),
    )?;
    let mut session = session;
    let session_id_text = session.id().to_string();
    let event = append_session_event(
        &mut session,
        None,
        "lobby:session_created",
        "session_created",
        Some("session"),
        Some(session_id_text.clone()),
        format!(r#"{{"session_id":"{}"}}"#, session_id_text),
    )?;
    let session_view = session_view(&session)?;
    apply_lobby_command(
        command,
        &client_nonce,
        Some(format!(r#"{{"session_id":"{}"}}"#, session.id())),
        event.into_iter().map(api_event_view).collect(),
        vec![
            changed("session", &session_view.session_id, "upsert"),
            changed("participant", &participant.id().to_string(), "upsert"),
        ],
        LobbyCommandResult::Session(session_view),
        session.current_turn,
    )
}

pub(crate) fn join_session(
    caller: CandidPrincipal,
    session_id: String,
    faction_id: String,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    apply_session_lobby_command(
        caller,
        &session_id,
        "join_session",
        &client_nonce,
        format!(
            r#"{{"session_id":"{}","faction_id":"{}"}}"#,
            escape_json(&session_id),
            escape_json(&faction_id)
        ),
        |command, player, mut session, client_nonce| {
            if player_has_live_session(player.id())? {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error(
                        "active_session_limit",
                        "player already has an active or lobby session",
                        false,
                    ),
                    session.current_turn,
                );
            }
            if session.state != "lobby" {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error("session_not_joinable", "session cannot be joined", false),
                    session.current_turn,
                );
            }

            let participants = participants_for_session(session.id())?;
            if participants.len() >= usize::from(FIRST_PLAYABLE_PLAYER_COUNT) {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error("player_cap_reached", "player cap reached", false),
                    session.current_turn,
                );
            }
            let slot_index = u8::try_from(participants.len()).unwrap_or(u8::MAX);
            let faction = faction_for_slot(Id::from_key(session.ruleset_id), slot_index)?;
            if !is_faction_arg_for(&faction_id, &faction) {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error(
                        "invalid_faction_for_slot",
                        "faction does not match the next first-playable slot",
                        false,
                    ),
                    session.current_turn,
                );
            }

            let color_key = if slot_index == 0 { "red" } else { "blue" }.to_string();
            let participant = sessions::create_participant(
                session.id(),
                player.id(),
                faction.id(),
                slot_index,
                color_key,
            )?;
            let session_id_text = session.id().to_string();
            let participant_id_text = participant.id().to_string();
            let event = append_session_event(
                &mut session,
                None,
                &format!("lobby:participant_joined:{participant_id_text}"),
                "participant_joined",
                Some("participant"),
                Some(participant_id_text.clone()),
                format!(
                    r#"{{"session_id":"{}","participant_id":"{}"}}"#,
                    session_id_text, participant_id_text
                ),
            )?;
            let session_view = session_view(&session)?;
            apply_lobby_command(
                command,
                client_nonce,
                Some(format!(
                    r#"{{"session_id":"{}","participant_id":"{}"}}"#,
                    session_id_text, participant_id_text
                )),
                event.into_iter().map(api_event_view).collect(),
                vec![
                    changed("session", &session_view.session_id, "update"),
                    changed("participant", &participant_id_text, "upsert"),
                ],
                LobbyCommandResult::Session(session_view),
                session.current_turn,
            )
        },
    )
}

pub(crate) fn mark_ready(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    apply_session_lobby_command(
        caller,
        &session_id,
        "mark_ready",
        &client_nonce,
        format!(r#"{{"session_id":"{}"}}"#, escape_json(&session_id)),
        |command, player, mut session, client_nonce| {
            if session.state != "lobby" {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error("session_not_joinable", "session cannot be joined", false),
                    session.current_turn,
                );
            }
            let mut participant = require_participant_for_player(session.id(), player.id())?;
            participant.ready_turn = session.current_turn;
            let participant = sessions::update_participant(participant)?;
            let session_id_text = session.id().to_string();
            let participant_id_text = participant.id().to_string();
            let event = append_session_event(
                &mut session,
                None,
                &format!("lobby:participant_ready:{participant_id_text}"),
                "participant_ready",
                Some("participant"),
                Some(participant_id_text.clone()),
                format!(
                    r#"{{"session_id":"{}","participant_id":"{}"}}"#,
                    session_id_text, participant_id_text
                ),
            )?;
            let session_view = session_view(&session)?;
            apply_lobby_command(
                command,
                client_nonce,
                Some(format!(
                    r#"{{"session_id":"{}","participant_id":"{}"}}"#,
                    session_id_text, participant_id_text
                )),
                event.into_iter().map(api_event_view).collect(),
                vec![
                    changed("session", &session_view.session_id, "update"),
                    changed("participant", &participant_id_text, "update"),
                ],
                LobbyCommandResult::Session(session_view),
                session.current_turn,
            )
        },
    )
}

pub(crate) fn start_session(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<LobbyCommandResponse, ApiError> {
    apply_session_lobby_command(
        caller,
        &session_id,
        "start_session",
        &client_nonce,
        format!(r#"{{"session_id":"{}"}}"#, escape_json(&session_id)),
        |command, player, mut session, client_nonce| {
            if session.created_by_player_id != player.id().key() {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error(
                        "not_session_creator",
                        "caller is not the session creator",
                        false,
                    ),
                    session.current_turn,
                );
            }
            if session.state == "active" {
                let session_view = session_view(&session)?;
                return apply_lobby_command(
                    command,
                    client_nonce,
                    Some(format!(
                        r#"{{"session_id":"{}","already_active":true}}"#,
                        session.id()
                    )),
                    Vec::new(),
                    vec![changed("session", &session_view.session_id, "update")],
                    LobbyCommandResult::Session(session_view),
                    session.current_turn,
                );
            }
            if !matches!(session.state.as_str(), "lobby" | "starting") {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error("session_not_joinable", "session cannot be started", false),
                    session.current_turn,
                );
            }

            let participants = participants_for_session(session.id())?;
            if participants.len() != usize::from(FIRST_PLAYABLE_PLAYER_COUNT) {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error("player_cap_reached", "player cap reached", false),
                    session.current_turn,
                );
            }
            if participants
                .iter()
                .any(|participant| participant.ready_turn == 0)
            {
                return fail_lobby_command(
                    command,
                    client_nonce,
                    public_error(
                        "participants_not_ready",
                        "participants are not ready",
                        false,
                    ),
                    session.current_turn,
                );
            }

            let started_now = session.state != "starting";
            if started_now {
                session.state = "starting".to_string();
                session = sessions::update_session(session)?;
            }
            let setup_command = ensure_setup_command(&session)?;
            if started_now {
                system_job_service::schedule_job(system_job_repo::SystemJobDraft {
                    job_key: setup_session_job_key(session.id()),
                    job_kind: "setup_session".to_string(),
                    session_id: session.id(),
                    battle_id: None,
                    turn_number: Some(session.current_turn),
                    due_at: Timestamp::now(),
                    command_id: Some(setup_command.id()),
                    cursor_json: setup_command.result_json.clone(),
                })?;
            }

            #[cfg(target_arch = "wasm32")]
            if !started_now {
                let setup_complete = run_setup(&mut session, &setup_command, &participants)?;
                if setup_complete {
                    session.state = "active".to_string();
                    session = sessions::update_session(session)?;
                    if let Some(job) = system_job_repo::find_system_job_by_key(
                        &setup_session_job_key(session.id()),
                    )? {
                        system_job_repo::complete_system_job(job)?;
                    }
                    system_job_service::schedule_job(system_job_repo::SystemJobDraft {
                        job_key: format!("turn_deadline:{}:{}", session.id(), session.current_turn),
                        job_kind: "turn_deadline".to_string(),
                        session_id: session.id(),
                        battle_id: None,
                        turn_number: Some(session.current_turn),
                        due_at: session.turn_deadline_at,
                        command_id: Some(setup_command.id()),
                        cursor_json: None,
                    })?;
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                for _ in 0..(SETUP_EFFECTS.len() + 2) {
                    if let Some(job) = system_job_repo::find_system_job_by_key(
                        &setup_session_job_key(session.id()),
                    )? {
                        process_setup_session_job(job)?;
                    } else {
                        let setup_complete =
                            run_setup(&mut session, &setup_command, &participants)?;
                        if setup_complete {
                            session.state = "active".to_string();
                            session = sessions::update_session(session)?;
                        }
                    }
                    if let Some(updated) = sessions::load_session(session.id())? {
                        session = updated;
                    }
                    if session.state == "active" {
                        break;
                    }
                }
                if let Some(updated) = sessions::load_session(session.id())? {
                    session = updated;
                }
            }
            let setup_complete = session.state == "active";
            let session_view = session_view(&session)?;
            apply_lobby_command(
                command,
                client_nonce,
                Some(format!(
                    r#"{{"session_id":"{}","setup_complete":{}}}"#,
                    session.id(),
                    setup_complete
                )),
                Vec::new(),
                vec![changed("session", &session_view.session_id, "update")],
                LobbyCommandResult::Session(session_view),
                session.current_turn,
            )
        },
    )
}

pub(crate) fn get_session(session_id: String) -> Result<SessionView, ApiError> {
    let session = load_session_from_text(&session_id)?;
    session_view(&session)
}

pub(crate) fn get_my_participant(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<ParticipantView, ApiError> {
    reject_anonymous(caller)?;
    let player = require_player(caller)?;
    let session = load_session_from_text(&session_id)?;
    let participant = require_participant_for_player(session.id(), player.id())?;
    participant_view(&participant)
}

fn apply_session_lobby_command<F>(
    caller: CandidPrincipal,
    session_id: &str,
    command_type: &'static str,
    client_nonce: &str,
    payload_json: String,
    apply: F,
) -> Result<LobbyCommandResponse, ApiError>
where
    F: FnOnce(
        LobbyCommand,
        PlayerAccount,
        GameSession,
        &str,
    ) -> Result<LobbyCommandResponse, ApiError>,
{
    reject_anonymous(caller)?;
    let actor_principal = Principal::from(caller);
    let player = players::find_by_principal(actor_principal)?;
    let actor_player_id = player.as_ref().map(EntityValue::id);
    let command = match begin_lobby_command(
        actor_principal,
        actor_player_id,
        command_type,
        client_nonce,
        payload_json,
    )? {
        LobbyCommandAction::Apply(command) => command,
        LobbyCommandAction::Return(response) => return Ok(response),
    };
    let Some(player) = player else {
        return fail_lobby_command(
            command,
            client_nonce,
            public_error(
                "player_not_registered",
                "register a player before using lobby commands",
                false,
            ),
            0,
        );
    };
    let session = match load_session_from_text(session_id) {
        Ok(session) => session,
        Err(error) => return fail_lobby_command(command, client_nonce, error, 0),
    };
    apply(command, player, session, client_nonce)
}

fn begin_lobby_command(
    actor_principal: Principal,
    actor_player_id: Option<Id<PlayerAccount>>,
    command_type: &'static str,
    client_nonce_text: &str,
    payload_json: String,
) -> Result<LobbyCommandAction, ApiError> {
    if payload_json.len() > domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES {
        return Ok(LobbyCommandAction::Return(failed_lobby_response(
            actor_principal,
            command_type,
            client_nonce_text,
            payload_hash(
                command_type,
                &actor_principal.to_string(),
                client_nonce_text,
                &payload_json,
            ),
            public_error(
                "payload_too_large",
                "lobby command payload is too large",
                false,
            ),
            0,
        )));
    }

    let client_nonce = nonce_u64(command_type, client_nonce_text);
    let hash = payload_hash(
        command_type,
        &actor_principal.to_string(),
        client_nonce_text,
        &payload_json,
    );
    if let Some(existing) =
        commands_events_effects::find_lobby_command_by_idempotency(actor_principal, client_nonce)?
    {
        if existing.payload_hash != hash {
            return Ok(LobbyCommandAction::Return(failed_lobby_response(
                actor_principal,
                command_type,
                client_nonce_text,
                hash,
                public_error(
                    "duplicate_nonce_payload_mismatch",
                    format!("client nonce {client_nonce_text} was reused with a different payload"),
                    false,
                ),
                0,
            )));
        }
        if matches!(existing.status.as_str(), "pending" | "applying") {
            return Ok(LobbyCommandAction::Apply(existing));
        }
        return response_from_lobby_command(existing, client_nonce_text)
            .map(LobbyCommandAction::Return);
    }

    let command = commands_events_effects::create_lobby_command(
        actor_principal,
        actor_player_id,
        client_nonce,
        hash,
        command_type.to_string(),
        payload_json,
    )?;
    Ok(LobbyCommandAction::Apply(command))
}

fn apply_lobby_command(
    mut command: LobbyCommand,
    client_nonce_text: &str,
    result_json: Option<String>,
    events: Vec<ApiEventView>,
    changed_subjects: Vec<ChangedSubject>,
    result: LobbyCommandResult,
    turn: u32,
) -> Result<LobbyCommandResponse, ApiError> {
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = result_json;
    command.error_code = None;
    command.error_message = None;
    command.error_details_json = None;
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    command.failed_at = None;
    let command = commands_events_effects::update_lobby_command(command)?;
    Ok(LobbyCommandResponse {
        command_id: command.id().to_string(),
        command_type: command.command_type,
        actor_principal: command.actor_principal.into(),
        client_nonce: client_nonce_text.to_string(),
        payload_hash: command.payload_hash,
        status: CommandStatus::Applied,
        phase: CommandPhase::Complete,
        retryable: false,
        effective_turn: turn,
        durable_turn: turn,
        events,
        changed_subjects,
        result,
        error: None,
    })
}

fn fail_lobby_command(
    mut command: LobbyCommand,
    client_nonce_text: &str,
    error: ApiError,
    turn: u32,
) -> Result<LobbyCommandResponse, ApiError> {
    command.status = "failed".to_string();
    command.phase = "failed".to_string();
    command.result_json = None;
    command.error_code = Some(error.code.clone());
    command.error_message = Some(error.message.clone());
    command.error_details_json = error.details_json.clone();
    command.retryable = error.retryable;
    command.failed_at = Some(Timestamp::now());
    let command = commands_events_effects::update_lobby_command(command)?;
    Ok(lobby_response_from_parts(
        command.id().to_string(),
        command.command_type,
        command.actor_principal.into(),
        client_nonce_text.to_string(),
        command.payload_hash,
        CommandStatus::Failed,
        CommandPhase::Failed,
        command.retryable,
        turn,
        Vec::new(),
        Vec::new(),
        LobbyCommandResult::None,
        Some(error),
    ))
}

fn response_from_lobby_command(
    command: LobbyCommand,
    client_nonce_text: &str,
) -> Result<LobbyCommandResponse, ApiError> {
    let status = status_from_str(&command.status);
    let phase = phase_from_str(&command.phase);
    let turn = replay_turn(&command)?;
    let result = if status == CommandStatus::Applied {
        replay_lobby_result(&command)?
    } else {
        LobbyCommandResult::None
    };
    let changed_subjects = changed_subjects_for_result(&result);
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

    Ok(lobby_response_from_parts(
        command.id().to_string(),
        command.command_type,
        command.actor_principal.into(),
        client_nonce_text.to_string(),
        command.payload_hash,
        status,
        phase,
        command.retryable,
        turn,
        Vec::new(),
        changed_subjects,
        result,
        error,
    ))
}

#[allow(clippy::too_many_arguments)]
fn lobby_response_from_parts(
    command_id: String,
    command_type: String,
    actor_principal: CandidPrincipal,
    client_nonce: String,
    payload_hash: String,
    status: CommandStatus,
    phase: CommandPhase,
    retryable: bool,
    turn: u32,
    events: Vec<ApiEventView>,
    changed_subjects: Vec<ChangedSubject>,
    result: LobbyCommandResult,
    error: Option<ApiError>,
) -> LobbyCommandResponse {
    LobbyCommandResponse {
        command_id,
        command_type,
        actor_principal,
        client_nonce,
        payload_hash,
        status,
        phase,
        retryable,
        effective_turn: turn,
        durable_turn: turn,
        events,
        changed_subjects,
        result,
        error,
    }
}

fn failed_lobby_response(
    actor_principal: Principal,
    command_type: &str,
    client_nonce: &str,
    payload_hash: String,
    error: ApiError,
    turn: u32,
) -> LobbyCommandResponse {
    lobby_response_from_parts(
        format!("command:lobby:{command_type}:{}", short_hash(client_nonce)),
        command_type.to_string(),
        actor_principal.into(),
        client_nonce.to_string(),
        payload_hash,
        CommandStatus::Failed,
        CommandPhase::Failed,
        error.retryable,
        turn,
        Vec::new(),
        Vec::new(),
        LobbyCommandResult::None,
        Some(error),
    )
}

fn replay_lobby_result(command: &LobbyCommand) -> Result<LobbyCommandResult, ApiError> {
    match command.command_type.as_str() {
        "register_player" => {
            let Some(player_id) = json_string_field(command.result_json.as_deref(), "player_id")
            else {
                return Ok(LobbyCommandResult::None);
            };
            let player = load_player_from_text(&player_id)?;
            Ok(LobbyCommandResult::Player(player_view(&player)))
        }
        "create_session" | "join_session" | "mark_ready" | "start_session" => {
            let Some(session_id) = json_string_field(command.result_json.as_deref(), "session_id")
            else {
                return Ok(LobbyCommandResult::None);
            };
            let session = load_session_from_text(&session_id)?;
            Ok(LobbyCommandResult::Session(session_view(&session)?))
        }
        _ => Ok(LobbyCommandResult::None),
    }
}

fn replay_turn(command: &LobbyCommand) -> Result<u32, ApiError> {
    let Some(session_id) = json_string_field(command.result_json.as_deref(), "session_id") else {
        return Ok(0);
    };
    load_session_from_text(&session_id).map(|session| session.current_turn)
}

fn changed_subjects_for_result(result: &LobbyCommandResult) -> Vec<ChangedSubject> {
    match result {
        LobbyCommandResult::Player(player) => {
            vec![changed("player", &player.player_id, "upsert")]
        }
        LobbyCommandResult::Session(session) => {
            vec![changed("session", &session.session_id, "update")]
        }
        LobbyCommandResult::None => Vec::new(),
    }
}

fn ensure_first_playable_content()
-> foundation::RepoResult<(RulesetDefinition, Vec<FactionDefinition>)> {
    let manifest = first_playable_content_manifest();
    let ruleset = match content::find_ruleset_by_slug_version(
        FIRST_PLAYABLE_RULESET_SLUG,
        FIRST_PLAYABLE_RULESET_VERSION,
    )? {
        Some(ruleset) => ruleset,
        None => content::create_ruleset_definition(
            manifest.ruleset.slug.clone(),
            manifest.ruleset.version,
            manifest.ruleset.name.clone(),
            manifest.ruleset.description.clone(),
            Some(manifest.ruleset.content_manifest_hash.clone()),
        )?,
    };

    let mut factions = Vec::with_capacity(manifest.factions.len());
    for faction in manifest.factions {
        let row = match content::find_faction_by_ruleset_slug(ruleset.id(), &faction.slug)? {
            Some(row) => row,
            None => content::create_faction_definition(
                ruleset.id(),
                faction.slug,
                faction.name,
                faction.trait_key,
            )?,
        };
        factions.push(row);
    }

    Ok((ruleset, factions))
}

fn ensure_setup_command(session: &GameSession) -> Result<GameCommand, ApiError> {
    let client_nonce = nonce_u64("setup_session", &session.id().to_string());
    if let Some(command) = commands_events_effects::find_game_command_by_idempotency(
        session.id(),
        "system",
        SETUP_SYSTEM_ACTOR,
        client_nonce,
    )? {
        return Ok(command);
    }

    let scenario = first_playable_scenario();
    commands_events_effects::create_game_command(
        session.id(),
        "system".to_string(),
        SETUP_SYSTEM_ACTOR.to_string(),
        None,
        None,
        None,
        session.current_turn,
        client_nonce,
        "setup_session".to_string(),
        payload_hash(
            "setup_session",
            SETUP_SYSTEM_ACTOR,
            &session.id().to_string(),
            &scenario.scenario_hash,
        ),
        format!(
            r#"{{"scenario_hash":"{}","ruleset":"{}"}}"#,
            scenario.scenario_hash, FIRST_PLAYABLE_RULESET_ID
        ),
    )
    .map_err(Into::into)
}

pub(crate) fn process_setup_session_job(job: SystemJob) -> Result<(), ApiError> {
    let fallback = job.clone();
    if let Err(error) = process_setup_session_job_inner(job) {
        system_job_repo::fail_system_job(fallback, error.retryable, error.message.clone())?;
        return Err(error);
    }
    Ok(())
}

fn process_setup_session_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(mut session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(job, false, "setup session row not found".to_string())?;
        return Ok(());
    };

    if session.state == "active" {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }
    if session.state != "starting" {
        system_job_repo::fail_system_job(job, false, "session is not starting".to_string())?;
        return Ok(());
    }

    let participants = participants_for_session(session.id())?;
    let setup_command = ensure_setup_command(&session)?;
    let setup_complete = run_setup(&mut session, &setup_command, &participants)?;
    if setup_complete {
        session.state = "active".to_string();
        session = sessions::update_session(session)?;
        system_job_repo::complete_system_job(job)?;
        system_job_service::schedule_job(system_job_repo::SystemJobDraft {
            job_key: format!("turn_deadline:{}:{}", session.id(), session.current_turn),
            job_kind: "turn_deadline".to_string(),
            session_id: session.id(),
            battle_id: None,
            turn_number: Some(session.current_turn),
            due_at: session.turn_deadline_at,
            command_id: Some(setup_command.id()),
            cursor_json: None,
        })?;
    } else {
        system_job_repo::reschedule_system_job(
            job,
            Timestamp::now(),
            setup_command.result_json.clone(),
        )?;
    }
    Ok(())
}

fn run_setup(
    session: &mut GameSession,
    setup_command: &GameCommand,
    participants: &[GameParticipant],
) -> Result<bool, ApiError> {
    for effect in SETUP_EFFECTS
        .iter()
        .skip(next_setup_effect_index(setup_command))
    {
        if commands_events_effects::find_command_effect(setup_command.id(), effect.key)?.is_none() {
            apply_setup_effect(session, participants, effect)?;
            ensure_setup_effects(session.id(), setup_command.id(), effect)?;
            let mut command = setup_command.clone();
            command.status = "applying".to_string();
            command.phase = "effects_applied".to_string();
            command.result_json = Some(format!(
                r#"{{"setup_complete":false,"last_effect":"{}"}}"#,
                effect.key
            ));
            command.retryable = true;
            commands_events_effects::update_game_command(command)?;
            return Ok(false);
        }
    }
    ensure_setup_event(session, setup_command.id())?;
    ensure_match_summary_shells(session.id(), participants)?;

    let mut command = setup_command.clone();
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some("{\"setup_complete\":true}".to_string());
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    commands_events_effects::update_game_command(command)?;
    Ok(true)
}

fn next_setup_effect_index(setup_command: &GameCommand) -> usize {
    if setup_command.status == "applied" {
        return SETUP_EFFECTS.len();
    }
    let Some(last_effect) = json_string_field(setup_command.result_json.as_deref(), "last_effect")
    else {
        return 0;
    };
    SETUP_EFFECTS
        .iter()
        .position(|effect| effect.key == last_effect)
        .map_or(0, |index| index.saturating_add(1))
}

fn setup_session_job_key(session_id: Id<GameSession>) -> String {
    format!("setup_session:{session_id}")
}

fn apply_setup_effect(
    session: &GameSession,
    participants: &[GameParticipant],
    effect: &SetupEffectSpec,
) -> Result<(), ApiError> {
    match effect.key {
        "seed_ruleset_content" => {
            first_playable_setup::ensure_first_playable_content_rows().map(|_| ())
        }
        "seed_participants" => Ok(()),
        "seed_towns" => first_playable_setup::seed_first_playable_towns(session, participants),
        "seed_champions" => {
            first_playable_setup::seed_first_playable_champions(session, participants)
        }
        "seed_map_chunks" => {
            first_playable_setup::seed_first_playable_map_chunks(session, participants)
        }
        "seed_occupancy" => {
            first_playable_setup::seed_first_playable_occupancy(session, participants)
        }
        "seed_visibility" => {
            first_playable_setup::seed_first_playable_visibility(session, participants)
        }
        "seed_world_objects" => {
            first_playable_setup::seed_first_playable_world_objects(session, participants)
        }
        "seed_resource_piles" => first_playable_setup::seed_first_playable_resource_piles(session),
        "seed_external_dwellings" => {
            first_playable_setup::seed_first_playable_external_dwellings(session, participants)
        }
        "seed_dwelling_pools" => {
            first_playable_setup::seed_first_playable_dwelling_pools(session, participants)
        }
        "seed_neutrals" => first_playable_setup::seed_first_playable_neutrals(session),
        "seed_economy" => first_playable_setup::seed_first_playable_economy(session, participants),
        "seed_tavern_offers" => {
            first_playable_setup::seed_first_playable_tavern_offers(session, participants)
        }
        "seed_scenario_progress" => {
            first_playable_setup::seed_first_playable_scenario_progress(session, participants)
        }
        "seed_worldgen" => first_playable_setup::seed_first_playable_worldgen(session),
        _ => Ok(()),
    }
}

fn ensure_setup_effects(
    session_id: Id<GameSession>,
    setup_command_id: Id<GameCommand>,
    effect: &SetupEffectSpec,
) -> Result<(), ApiError> {
    let now = Timestamp::now();
    if commands_events_effects::find_command_effect(setup_command_id, effect.key)?.is_none() {
        commands_events_effects::create_applied_command_effect(
            session_id,
            setup_command_id,
            effect.key.to_string(),
            effect.effect_type.to_string(),
            effect.target_kind.to_string(),
            session_id.to_string(),
            "{}".to_string(),
            now,
        )?;
    }
    let pending_key = format!("setup:{}", effect.key);
    if commands_events_effects::find_pending_effect(session_id, &pending_key)?.is_none() {
        commands_events_effects::create_applied_pending_effect(
            session_id,
            Some(setup_command_id),
            None,
            None,
            pending_key,
            1,
            effect.effect_type.to_string(),
            "{}".to_string(),
            now,
        )?;
    }
    Ok(())
}

fn ensure_setup_event(
    session: &mut GameSession,
    setup_command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    append_session_event_with_command(
        session,
        Some(setup_command_id),
        "setup:complete",
        "session_started",
        Some("session"),
        Some(session.id().to_string()),
        format!(
            r#"{{"ruleset":"{}","version":{},"map_width":{},"map_height":{}}}"#,
            FIRST_PLAYABLE_RULESET_SLUG,
            FIRST_PLAYABLE_RULESET_VERSION,
            FIRST_PLAYABLE_MAP_WIDTH,
            FIRST_PLAYABLE_MAP_HEIGHT
        ),
    )
    .map(|_| ())
}

fn ensure_match_summary_shells(
    session_id: Id<GameSession>,
    participants: &[GameParticipant],
) -> Result<(), ApiError> {
    for participant in participants {
        if aftermath_history::find_match_summary_for_player_session(
            Id::from_key(participant.player_id),
            session_id,
        )?
        .is_none()
        {
            aftermath_history::create_match_summary_shell(
                Id::from_key(participant.player_id),
                session_id,
                "pending".to_string(),
                None,
                0,
                Some("{\"state\":\"active\"}".to_string()),
            )?;
        }
    }
    Ok(())
}

fn append_session_event(
    session: &mut GameSession,
    command_id: Option<Id<GameCommand>>,
    event_key: &str,
    event_type: &str,
    subject_kind: Option<&str>,
    subject_id_text: Option<String>,
    payload_json: String,
) -> Result<Option<GameEvent>, ApiError> {
    append_session_event_with_command(
        session,
        command_id,
        event_key,
        event_type,
        subject_kind,
        subject_id_text,
        payload_json,
    )
    .map(Some)
}

fn append_session_event_with_command(
    session: &mut GameSession,
    command_id: Option<Id<GameCommand>>,
    event_key: &str,
    event_type: &str,
    subject_kind: Option<&str>,
    subject_id_text: Option<String>,
    payload_json: String,
) -> Result<GameEvent, ApiError> {
    if let Some(event) = commands_events_effects::find_event_by_key(session.id(), event_key)? {
        bump_event_seq_after_existing(session, event.event_seq)?;
        return Ok(event);
    }

    let event_seq = session.next_event_seq;
    let event = commands_events_effects::create_game_event(
        session.id(),
        command_id,
        None,
        session.current_turn,
        event_seq,
        event_key.to_string(),
        "public".to_string(),
        event_type.to_string(),
        subject_kind.map(str::to_string),
        subject_id_text,
        payload_json,
    )?;
    session.next_event_seq = event_seq.saturating_add(1);
    *session = sessions::update_session(session.clone())?;
    Ok(event)
}

fn bump_event_seq_after_existing(
    session: &mut GameSession,
    existing_event_seq: u64,
) -> Result<(), ApiError> {
    let next = existing_event_seq.saturating_add(1);
    if session.next_event_seq <= existing_event_seq {
        session.next_event_seq = next;
        *session = sessions::update_session(session.clone())?;
    }
    Ok(())
}

fn require_player(caller: CandidPrincipal) -> Result<PlayerAccount, ApiError> {
    players::find_by_principal(Principal::from(caller))?.ok_or_else(|| {
        public_error(
            "player_not_registered",
            "caller does not have a registered player",
            false,
        )
    })
}

fn load_player_from_text(player_id: &str) -> Result<PlayerAccount, ApiError> {
    let id = parse_id::<PlayerAccount>(player_id, "player_id")?;
    foundation::load_by_id("players.load_player", id)?.ok_or_else(|| {
        public_error(
            "player_not_found",
            format!("player was not found: {player_id}"),
            false,
        )
    })
}

fn load_session_from_text(session_id: &str) -> Result<GameSession, ApiError> {
    let id = parse_id::<GameSession>(session_id, "session_id")?;
    sessions::load_session(id)?.ok_or_else(|| {
        public_error(
            "session_not_found",
            format!("session was not found: {session_id}"),
            false,
        )
    })
}

fn require_participant_for_player(
    session_id: Id<GameSession>,
    player_id: Id<PlayerAccount>,
) -> Result<GameParticipant, ApiError> {
    sessions::find_participant_by_session_player(session_id, player_id)?.ok_or_else(|| {
        public_error(
            "participant_not_found",
            "caller is not a participant in this session",
            false,
        )
    })
}

fn participants_for_session(session_id: Id<GameSession>) -> Result<Vec<GameParticipant>, ApiError> {
    sessions::page_participants_by_session_status(session_id, "active", MAX_LIST_LIMIT, None)
        .map(|page| page.items)
}

fn player_has_live_session(player_id: Id<PlayerAccount>) -> Result<bool, ApiError> {
    let participants =
        sessions::page_participants_by_player_status(player_id, "active", MAX_LIST_LIMIT, None)?;
    for participant in participants.items {
        let Some(session) = sessions::load_session(Id::from_key(participant.session_id))? else {
            continue;
        };
        if ACTIVE_SESSION_STATES.contains(&session.state.as_str()) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn active_session_count() -> Result<u32, ApiError> {
    let mut total = 0_u32;
    for state in ACTIVE_SESSION_STATES {
        total = total.saturating_add(
            sessions::page_sessions_by_state(state, MAX_LIST_LIMIT, None)?
                .items
                .len()
                .try_into()
                .unwrap_or(u32::MAX),
        );
    }
    Ok(total)
}

fn session_view(session: &GameSession) -> Result<SessionView, ApiError> {
    let mut participants = participants_for_session(session.id())?;
    participants.sort_by_key(|participant| participant.slot_index);
    Ok(SessionView {
        session_id: session.id().to_string(),
        state: session.state.clone(),
        participant_ids: participants
            .into_iter()
            .map(|participant| participant.id().to_string())
            .collect(),
    })
}

fn player_view(player: &PlayerAccount) -> PlayerView {
    PlayerView {
        player_id: player.id().to_string(),
        display_name: player
            .display_name
            .clone()
            .or_else(|| player.username.clone())
            .unwrap_or_else(|| "Degens Player".to_string()),
        principal: player.account_principal.into(),
    }
}

fn participant_view(participant: &GameParticipant) -> Result<ParticipantView, ApiError> {
    let faction =
        content::load_faction(Id::from_key(participant.faction_id))?.ok_or_else(|| {
            public_error(
                "faction_not_found",
                "participant faction was not found",
                false,
            )
        })?;
    Ok(ParticipantView {
        participant_id: participant.id().to_string(),
        session_id: Id::<GameSession>::from_key(participant.session_id).to_string(),
        player_id: Id::<PlayerAccount>::from_key(participant.player_id).to_string(),
        faction_slug: faction.slug,
        slot_index: participant.slot_index,
        status: participant.status.clone(),
        ready: participant.ready_turn > 0,
        resources: ResourceCost {
            gold: participant.gold.try_into().unwrap_or(u32::MAX),
            wood: participant.wood,
            stone: participant.stone,
            iron: participant.iron,
            crystal: participant.crystal,
            ember: participant.ember,
            aether: participant.aether,
        },
    })
}

fn faction_for_slot(
    ruleset_id: Id<RulesetDefinition>,
    slot_index: u8,
) -> Result<FactionDefinition, ApiError> {
    let scenario = first_playable_scenario();
    let slug = scenario
        .starts
        .get(usize::from(slot_index))
        .ok_or_else(|| public_error("player_cap_reached", "player cap reached", false))?
        .faction_slug
        .clone();
    content::find_faction_by_ruleset_slug(ruleset_id, &slug)?.ok_or_else(|| {
        public_error(
            "faction_not_found",
            format!("faction was not seeded: {slug}"),
            false,
        )
    })
}

fn is_first_playable_ruleset_arg(arg: &str, ruleset: &RulesetDefinition) -> bool {
    matches!(
        arg,
        FIRST_PLAYABLE_RULESET_ID | FIRST_PLAYABLE_RULESET_SLUG | "ruleset:first-playable"
    ) || arg == ruleset.id().to_string()
}

fn is_faction_arg_for(arg: &str, faction: &FactionDefinition) -> bool {
    arg == faction.slug
        || arg == faction.id().to_string()
        || arg == format!("faction:{}", faction.slug)
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

fn parse_id<E>(value: &str, field_name: &str) -> Result<Id<E>, ApiError>
where
    E: icydb::traits::EntityKey<Key = Ulid>,
{
    Ulid::from_str(value).map(Id::from_key).map_err(|_| {
        public_error(
            "invalid_id",
            format!("{field_name} is not a valid Ulid"),
            false,
        )
    })
}

fn reject_anonymous(caller: CandidPrincipal) -> Result<(), ApiError> {
    if caller == CandidPrincipal::anonymous() {
        Err(public_error(
            "anonymous_not_allowed",
            "anonymous callers cannot use player session endpoints",
            false,
        ))
    } else {
        Ok(())
    }
}

fn public_error(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> ApiError {
    ApiError::new(code, message, retryable)
}

fn changed(subject_kind: &str, subject_id_text: &str, operation: &str) -> ChangedSubject {
    ChangedSubject {
        subject_kind: subject_kind.to_string(),
        subject_id_text: subject_id_text.to_string(),
        operation: operation.to_string(),
    }
}

fn command_username(username: &Option<String>, caller: CandidPrincipal) -> Option<String> {
    username
        .clone()
        .or_else(|| Some(format!("player-{}", short_hash(&caller.to_text()))))
}

fn turn_deadline() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(i64::try_from(TURN_DURATION_MS).unwrap_or(i64::MAX)),
    )
}

fn status_from_str(value: &str) -> CommandStatus {
    match value {
        "pending" => CommandStatus::Pending,
        "applying" => CommandStatus::Applying,
        "applied" => CommandStatus::Applied,
        "failed" => CommandStatus::Failed,
        "cancelled" => CommandStatus::Cancelled,
        "superseded" => CommandStatus::Superseded,
        "applied_noop" => CommandStatus::AppliedNoop,
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

fn option_json(value: Option<&str>) -> String {
    match value {
        Some(value) => format!(r#""{}""#, escape_json(value)),
        None => "null".to_string(),
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

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn payload_hash(command_type: &str, actor_key: &str, client_nonce: &str, payload: &str) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "command_type", command_type);
    hash_text(&mut hasher, "actor_key", actor_key);
    hash_text(&mut hasher, "client_nonce", client_nonce);
    hash_text(&mut hasher, "payload", payload);
    to_hex(&hasher.finalize())
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
    hasher.update([0xFF]);
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0F) as usize] as char);
    }
    output
}
