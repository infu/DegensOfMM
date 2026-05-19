use std::collections::{BTreeMap, BTreeSet};

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    Battle, BattleStack, Champion, GameCommand, GameParticipant, GameSession, MovementIntent,
    NeutralArmy, SystemJob, Town, UnitDefinition, WorldObject,
};
use domm_game::{
    ApiError, CommandPhase, CommandResponse, CommandResult, CommandStatus, MoveCoord,
    MovementPathStop, MovementPreview, StrategicCommandReceipt,
};
use icydb::{
    traits::EntityValue,
    types::{Blob, Id, Timestamp, Ulid},
};

use crate::repos::{
    battles, champions_artifacts, commands_events_effects, content, economy,
    map_visibility_occupancy, movement, neutrals, sessions, system_jobs as system_job_repo, towns,
    turn_ready,
};

use super::{
    battle as battle_service, battle_runtime, battle_start,
    command_response::{self, GameCommandAction, GameCommandStart},
    economy_expansion, scenario_progress,
    session_context::{self, public_error},
    session_turn_runtime, system_jobs as system_job_service,
};

const CANISTER_MOVEMENT_MICROSTEPS_PER_SYNC: u16 = 1;
const PARTIAL_TURN_RETRY_DELAY_MS: i64 = 60_000;

pub(crate) fn preview_move_path(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    _now_ms: u64,
) -> Result<MovementPreview, ApiError> {
    validate_path_limit(&path)?;
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let champion = resolve_owned_champion(&context, &champion_id)?;
    validate_path_bounds(&context.session, &path)?;
    validate_path_adjacency(champion.x, champion.y, &path)?;
    let total_cost = validate_path_cost(&context.session, &champion, &path)?;
    let chunks_touched = chunks_touched(&context.session, &path);
    let stop = preview_path_stop(&context, &champion, &path)?;

    Ok(MovementPreview {
        champion_id: champion.id().to_string(),
        participant_id: context.participant.id().to_string(),
        turn_number: context.session.current_turn,
        total_cost,
        available_movement: effective_movement(&champion, context.session.current_turn),
        chunks_touched,
        path,
        stop,
    })
}

pub(crate) fn submit_move_intent(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    path: Vec<MoveCoord>,
    client_nonce: String,
    _now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    validate_path_limit(&path)?;
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let champion = resolve_owned_champion(&context, &champion_id)?;
    validate_path_bounds(&context.session, &path)?;
    validate_path_adjacency(champion.x, champion.y, &path)?;
    validate_path_cost(&context.session, &champion, &path)?;
    validate_no_friendly_champion_blocker(&context, &champion, &path)?;
    let path_text = path_text(&path);
    let payload_json = format!(
        r#"{{"champion_id":"{}","path":"{}"}}"#,
        command_response::escape_json(&champion_id),
        command_response::escape_json(&path_text)
    );
    let command_payload_hash = command_response::payload_hash(
        "submit_move_intent",
        &context.participant.id().to_string(),
        &client_nonce,
        &payload_json,
    );
    if payload_json.len() > domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES {
        return Ok(runtime_movement_failed_response(
            caller,
            &context,
            Ulid::generate().to_string(),
            &client_nonce,
            command_payload_hash,
            public_error(
                "payload_too_large",
                "game command payload is too large",
                false,
            ),
        ));
    }
    let client_nonce_u64 = command_response::nonce_u64("submit_move_intent", &client_nonce);
    let canonical_session_id = context.session.id().to_string();
    let actor_participant_id = context.participant.id().to_string();
    if let Some(existing) = session_turn_runtime::command_receipt_by_nonce(
        &canonical_session_id,
        &actor_participant_id,
        client_nonce_u64,
    ) {
        if existing.payload_hash != command_payload_hash {
            return Ok(runtime_movement_failed_response(
                caller,
                &context,
                Ulid::generate().to_string(),
                &client_nonce,
                command_payload_hash,
                public_error(
                    "duplicate_nonce_payload_mismatch",
                    format!("client nonce {client_nonce} was reused with a different payload"),
                    false,
                ),
            ));
        }
        return Ok(existing.response);
    }
    command_response::ensure_map_turn_accepts_new_command(&context, "submit_move_intent")?;
    ensure_session_turn_runtime(&context)?;
    let command_id = Id::<GameCommand>::from_key(Ulid::generate());
    let command_id_text = command_id.to_string();
    let path_hash = command_response::payload_hash(
        "movement_path",
        &champion.id().to_string(),
        &client_nonce,
        &path_text,
    );
    let intent = match movement::find_movement_intent(
        context.session.id(),
        champion.id(),
        context.session.current_turn,
    )? {
        Some(mut intent) => {
            intent.command_id = command_id.key();
            intent.actor_participant_id = context.participant.id().key();
            intent.status = "pending".to_string();
            intent.path_json = path_text.clone();
            intent.path_hash = path_hash;
            intent.resolved_at = None;
            movement::update_movement_intent(intent)?
        }
        None => movement::create_movement_intent(
            context.session.id(),
            context.session.current_turn,
            context.participant.id(),
            champion.id(),
            command_id,
            "pending".to_string(),
            path_text,
            path_hash,
        )?,
    };

    let mut session = context.session.clone();
    let event_key = format!(
        "movement_intent:{}:{}",
        champion.id(),
        context.session.current_turn
    );
    let event_payload = format!(
        r#"{{"intent_id":"{}","path_len":{}}}"#,
        intent.id(),
        path.len()
    );
    session_turn_runtime::with_runtime_mut(
        &canonical_session_id,
        context.session.current_turn,
        |runtime| {
            runtime.upsert_intent(session_turn_runtime::RuntimeMovementIntent::from_pending(
                intent.clone(),
                champion.clone(),
                context.participant.clone(),
            ));
            let event = runtime
                .active_events
                .iter()
                .find(|runtime_event| {
                    runtime_event.event.event_key == event_key
                        && runtime_event.event.audience_key == "public"
                })
                .map(|runtime_event| runtime_event.event.clone())
                .map_or_else(
                    || {
                        let event_seq =
                            session_turn_runtime::reserve_session_event_seq(runtime, &mut session)?;
                        let event = domm_game::ApiEventView {
                            session_id: canonical_session_id.clone(),
                            event_seq,
                            event_key: event_key.clone(),
                            audience_key: "public".to_string(),
                            turn_number: context.session.current_turn,
                            event_type: "movement_intent_submitted".to_string(),
                            subject_kind: Some("champion".to_string()),
                            subject_id_text: Some(champion.id().to_string()),
                            payload: Some(event_payload),
                            redacted: false,
                        };
                        runtime.push_event(session_turn_runtime::SessionTurnEvent {
                            command_id: Some(command_id_text.clone()),
                            event: event.clone(),
                            flushed: false,
                        });
                        Ok(event)
                    },
                    Ok::<domm_game::ApiEventView, ApiError>,
                )?;
            let changed_subjects = vec![command_response::changed(
                "movement_intent",
                &intent.id().to_string(),
                "upsert",
            )];
            let response = command_response::runtime_command_response(
                caller,
                &context,
                command_id_text.clone(),
                "submit_move_intent".to_string(),
                &client_nonce,
                command_payload_hash.clone(),
                CommandStatus::Applied,
                CommandPhase::Complete,
                false,
                vec![event],
                changed_subjects,
                CommandResult::StrategicReceipt(StrategicCommandReceipt {
                    command_kind: "submit_move_intent".to_string(),
                    command_id: command_id_text.clone(),
                    current_turn: context.session.current_turn,
                    command_count: 1,
                    event_count: 1,
                }),
                None,
            );
            runtime.insert_command_receipt(session_turn_runtime::SessionTurnCommandReceipt {
                command_id: command_id_text,
                command_type: "submit_move_intent".to_string(),
                actor_participant_id,
                client_nonce_text: client_nonce,
                client_nonce: client_nonce_u64,
                payload_hash: command_payload_hash,
                response: response.clone(),
            });
            Ok(response)
        },
    )
    .ok_or_else(|| {
        public_error(
            "turn_runtime_missing",
            "active turn runtime was not available",
            true,
        )
    })?
}

fn ensure_session_turn_runtime(
    context: &session_context::SessionCallerContext,
) -> Result<(), ApiError> {
    let session_id = context.session.id().to_string();
    let turn_number = context.session.current_turn;
    let participant = session_turn_participant(context);
    if session_turn_runtime::with_runtime_mut(&session_id, turn_number, |runtime| {
        runtime.upsert_participant(participant.clone());
    })
    .is_some()
    {
        return Ok(());
    }

    let mut runtime = session_turn_runtime::SessionTurnRuntime::new(
        session_id,
        turn_number,
        timestamp_to_u64(context.session.turn_started_at),
        timestamp_to_u64(context.session.turn_deadline_at),
        u64::from(context.session.turn_duration_ms),
    );
    hydrate_runtime_pending_movement_intents(&context.session, &mut runtime)?;
    runtime.upsert_participant(participant);
    session_turn_runtime::insert_runtime(runtime);
    Ok(())
}

fn hydrate_runtime_pending_movement_intents(
    session: &GameSession,
    runtime: &mut session_turn_runtime::SessionTurnRuntime,
) -> Result<(), ApiError> {
    for intent in movement::page_movement_intents_by_status(
        session.id(),
        session.current_turn,
        "pending",
        domm_game::MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN,
        None,
    )?
    .items
    {
        let Some(participant) = sessions::load_participant(Id::<GameParticipant>::from_key(
            intent.actor_participant_id,
        ))?
        else {
            continue;
        };
        if participant.session_id != session.id().key() || participant.status != "active" {
            continue;
        }
        let Some(champion) =
            champions_artifacts::load_champion(Id::<Champion>::from_key(intent.champion_id))?
        else {
            continue;
        };
        if champion.session_id != session.id().key()
            || champion.participant_id != participant.id().key()
        {
            continue;
        }
        runtime.upsert_intent(session_turn_runtime::RuntimeMovementIntent::from_pending(
            intent,
            champion,
            participant,
        ));
    }
    Ok(())
}

fn session_turn_participant(
    context: &session_context::SessionCallerContext,
) -> session_turn_runtime::SessionTurnParticipant {
    session_turn_runtime::SessionTurnParticipant {
        participant_id: context.participant.id().to_string(),
        player_id: context.participant.player_id.to_string(),
        slot_index: context.participant.slot_index,
        status: context.participant.status.clone(),
    }
}

fn runtime_movement_failed_response(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command_id: String,
    client_nonce: &str,
    payload_hash: String,
    error: ApiError,
) -> CommandResponse {
    let retryable = error.retryable;
    command_response::runtime_command_response(
        caller,
        context,
        command_id,
        "submit_move_intent".to_string(),
        client_nonce,
        payload_hash,
        CommandStatus::Failed,
        CommandPhase::Failed,
        retryable,
        Vec::new(),
        Vec::new(),
        CommandResult::None,
        Some(error),
    )
}

pub(crate) fn end_turn(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let payload_json = format!(
        r#"{{"session_id":"{}"}}"#,
        command_response::escape_json(&session_id)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "end_turn",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let ready = turn_ready::mark_turn_ready(
        context.session.id(),
        context.participant.id(),
        context.session.current_turn,
        Some(command.id()),
        Timestamp::now(),
    )?;
    let runtime_session_id = context.session.id().to_string();
    session_turn_runtime::with_runtime_mut(
        &runtime_session_id,
        context.session.current_turn,
        |runtime| runtime.mark_ready(context.participant.id().to_string()),
    );

    let participants = sessions::page_participants_by_session_status(
        context.session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let ready_rows = turn_ready::page_turn_ready_by_session_turn(
        context.session.id(),
        context.session.current_turn,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let all_ready =
        !participants.items.is_empty() && ready_rows.items.len() >= participants.items.len();
    let mut changed_subjects = vec![command_response::changed(
        "participant_turn_ready",
        &ready.id().to_string(),
        "upsert",
    )];

    if all_ready {
        let job = system_job_service::schedule_job(system_job_repo::SystemJobDraft {
            job_key: format!(
                "turn_resolution:{}:{}",
                context.session.id(),
                context.session.current_turn
            ),
            job_kind: "turn_resolution".to_string(),
            session_id: context.session.id(),
            battle_id: None,
            turn_number: Some(context.session.current_turn),
            due_at: Timestamp::now(),
            command_id: Some(command.id()),
            cursor_json: None,
        })?;
        changed_subjects.push(command_response::changed(
            "system_job",
            &job.id().to_string(),
            "upsert",
        ));
    }

    let session_id_text = context.session.id().to_string();
    let current_turn = context.session.current_turn;
    let participant_id_text = context.participant.id().to_string();
    let ready_count = ready_rows.items.len();
    let participant_count = participants.items.len();
    let event_key = format!("end_turn:{session_id_text}:{current_turn}:{participant_id_text}");
    let event_payload = format!(
        r#"{{"turn_number":{current_turn},"ready_count":{ready_count},"participant_count":{participant_count},"all_ready":{all_ready}}}"#
    );
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        event_key,
        "participant_turn_ready".to_string(),
        Some("participant".to_string()),
        Some(participant_id_text),
        event_payload,
    )?;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        format!(
            r#"{{"command_kind":"end_turn","current_turn":{},"ready_count":{},"participant_count":{},"all_ready":{},"command_count":1,"event_count":1}}"#,
            current_turn, ready_count, participant_count, all_ready
        ),
        vec![event],
        changed_subjects,
    )
}

pub(crate) fn sync_session_turn(
    caller: CandidPrincipal,
    session_id: String,
    now_ms: u64,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let payload_json = format!(
        r#"{{"session_id":"{}"}}"#,
        command_response::escape_json(&session_id)
    );
    if now_ms < timestamp_to_u64(context.session.turn_deadline_at)
        && runtime_has_no_ready_participants(&context.session)
    {
        return Ok(sync_turn_not_due_response(
            caller,
            &context,
            &client_nonce,
            &payload_json,
        ));
    }
    if now_ms < timestamp_to_u64(context.session.turn_deadline_at)
        && !all_participants_ready_for_turn(&context.session)?
    {
        return Ok(sync_turn_not_due_response(
            caller,
            &context,
            &client_nonce,
            &payload_json,
        ));
    }
    let command = match command_response::begin_participant_command_tracked(
        caller,
        &context,
        "sync_session_turn",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandStart::Apply(command) => command,
        GameCommandStart::Return(response) => return Ok(response),
    };

    let mut changed_subjects = Vec::new();
    let mut events = Vec::new();
    let movement_complete = resolve_pending_movement(
        &mut context.session,
        command.id(),
        &mut events,
        &mut changed_subjects,
    )?;
    if !movement_complete || should_yield_after_movement_events(&events) {
        reschedule_current_turn_jobs_for_manual_sync(&context.session)?;
        return command_response::apply_command(
            caller,
            &context,
            command,
            &client_nonce,
            command_response::result_json("sync_session_turn", context.session.current_turn),
            events,
            changed_subjects,
        );
    }
    if let Some(updated_participant) = sessions::load_participant(context.participant.id())? {
        context.participant = updated_participant;
    }

    let income_turn = context.session.current_turn;
    let mut participant = context.participant.clone();
    let income_events = materialize_income(
        &mut context.session,
        command.id(),
        &mut participant,
        income_turn,
    )?;
    if !income_events.is_empty() {
        events.extend(income_events);
        changed_subjects.push(command_response::changed(
            "participant",
            &participant.id().to_string(),
            "resources",
        ));
    }

    participant.last_action_turn = context.session.current_turn;
    participant = sessions::update_participant(participant)?;
    context.participant = participant;

    complete_current_turn_jobs(context.session.id(), income_turn)?;
    context.session.current_turn = context.session.current_turn.saturating_add(1);
    context.session.turn_started_at = Timestamp::now();
    context.session.turn_deadline_at = turn_deadline();
    context.session.last_command_id = Some(command.id);
    economy_expansion::materialize_weekly_economy(&context.session, command.id())?;
    context.session = sessions::update_session(context.session)?;
    changed_subjects.push(command_response::changed(
        "session",
        &context.session.id().to_string(),
        "update",
    ));

    let session_id_text = context.session.id().to_string();
    let current_turn = context.session.current_turn;
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("sync_turn:{session_id_text}:{current_turn}"),
        "session_turn_synced".to_string(),
        Some("session".to_string()),
        Some(session_id_text),
        format!(r#"{{"current_turn":{current_turn}}}"#),
    )?;
    events.push(event);

    let job = system_job_service::schedule_job(system_job_repo::SystemJobDraft {
        job_key: format!(
            "turn_deadline:{}:{}",
            context.session.id(),
            context.session.current_turn
        ),
        job_kind: "turn_deadline".to_string(),
        session_id: context.session.id(),
        battle_id: None,
        turn_number: Some(context.session.current_turn),
        due_at: context.session.turn_deadline_at,
        command_id: Some(command.id()),
        cursor_json: None,
    })?;
    changed_subjects.push(command_response::changed(
        "system_job",
        &job.id().to_string(),
        "upsert",
    ));
    scenario_progress::schedule_turn_maintenance_jobs(&context.session, Some(command.id()))?;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        command_response::result_json("sync_session_turn", context.session.current_turn),
        events,
        changed_subjects,
    )
}

fn sync_turn_not_due_response(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    client_nonce: &str,
    payload_json: &str,
) -> CommandResponse {
    let error = public_error("turn_not_due", "turn deadline has not elapsed", false);
    command_response::runtime_command_response(
        caller,
        context,
        Ulid::generate().to_string(),
        "sync_session_turn".to_string(),
        client_nonce,
        command_response::payload_hash(
            "sync_session_turn",
            &context.participant.id().to_string(),
            client_nonce,
            payload_json,
        ),
        CommandStatus::Failed,
        CommandPhase::Failed,
        error.retryable,
        Vec::new(),
        Vec::new(),
        CommandResult::None,
        Some(error),
    )
}

fn should_yield_after_movement_events(events: &[domm_game::ApiEventView]) -> bool {
    !events.is_empty()
}

pub(crate) fn process_turn_resolution_job(job: SystemJob) -> Result<(), ApiError> {
    let fallback = job.clone();
    if let Err(error) = process_turn_resolution_job_inner(job) {
        system_job_repo::fail_system_job(fallback, error.retryable, error.message.clone())?;
        return Err(error);
    }
    Ok(())
}

fn process_turn_resolution_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(mut session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(job, false, "turn session row not found".to_string())?;
        return Ok(());
    };

    if session.state != "active" {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }
    if job
        .turn_number
        .is_some_and(|turn| turn != session.current_turn)
    {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }
    if job.job_kind == "turn_deadline" && Timestamp::now() < session.turn_deadline_at {
        system_job_repo::reschedule_system_job(job, session.turn_deadline_at, None)?;
        return Ok(());
    }
    let command = ensure_system_turn_command(&session, &job)?;
    let mut changed_subjects = Vec::new();
    let mut events = Vec::new();
    let movement_complete = resolve_pending_movement(
        &mut session,
        command.id(),
        &mut events,
        &mut changed_subjects,
    )?;
    if !movement_complete || should_yield_after_movement_events(&events) {
        let mut command = command;
        command.status = "applying".to_string();
        command.phase = "movement_partial".to_string();
        command.retryable = true;
        command.result_json = Some(format!(
            r#"{{"command_kind":"turn_resolution","current_turn":{},"partial":true}}"#,
            session.current_turn
        ));
        commands_events_effects::update_game_command(command)?;
        system_job_repo::reschedule_system_job(job, partial_retry_at(), None)?;
        return Ok(());
    }

    let income_turn = session.current_turn;
    let participants = sessions::page_participants_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    for mut participant in participants.items {
        let income_events =
            materialize_income(&mut session, command.id(), &mut participant, income_turn)?;
        events.extend(income_events);
        participant.last_action_turn = income_turn;
        sessions::update_participant(participant)?;
    }

    session.current_turn = session.current_turn.saturating_add(1);
    session.turn_started_at = Timestamp::now();
    session.turn_deadline_at = turn_deadline();
    session.last_command_id = Some(command.id);
    economy_expansion::materialize_weekly_economy(&session, command.id())?;
    session = sessions::update_session(session)?;

    let session_id_text = session.id().to_string();
    let current_turn = session.current_turn;
    let turn_event = command_response::append_public_event(
        &mut session,
        command.id(),
        format!("turn_resolution:{session_id_text}:{current_turn}"),
        "session_turn_advanced".to_string(),
        Some("session".to_string()),
        Some(session_id_text),
        format!(r#"{{"current_turn":{current_turn}}}"#),
    )?;
    events.push(turn_event);

    let mut command = command;
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(format!(
        r#"{{"command_kind":"turn_resolution","current_turn":{},"command_count":1,"event_count":{}}}"#,
        session.current_turn,
        events.len()
    ));
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    command.failed_at = None;
    commands_events_effects::update_game_command(command.clone())?;

    system_job_repo::complete_system_job(job)?;
    system_job_service::schedule_job(system_job_repo::SystemJobDraft {
        job_key: format!("turn_deadline:{}:{}", session.id(), session.current_turn),
        job_kind: "turn_deadline".to_string(),
        session_id: session.id(),
        battle_id: None,
        turn_number: Some(session.current_turn),
        due_at: session.turn_deadline_at,
        command_id: Some(command.id()),
        cursor_json: None,
    })?;
    scenario_progress::schedule_turn_maintenance_jobs(&session, Some(command.id()))?;
    Ok(())
}

fn ensure_system_turn_command(
    session: &GameSession,
    job: &SystemJob,
) -> Result<GameCommand, ApiError> {
    let command_type = "turn_resolution";
    let client_nonce = command_response::nonce_u64(command_type, &job.job_key);
    if let Some(command) = commands_events_effects::find_game_command_by_idempotency(
        session.id(),
        "system",
        &job.job_key,
        client_nonce,
    )? {
        return Ok(command);
    }

    let payload_json = format!(
        r#"{{"job_key":"{}","job_kind":"{}","turn_number":{}}}"#,
        command_response::escape_json(&job.job_key),
        command_response::escape_json(&job.job_kind),
        session.current_turn
    );
    commands_events_effects::create_game_command(
        session.id(),
        "system".to_string(),
        job.job_key.clone(),
        None,
        None,
        None,
        session.current_turn,
        client_nonce,
        command_type.to_string(),
        command_response::payload_hash(command_type, &job.job_key, &job.job_key, &payload_json),
        payload_json,
    )
}

#[derive(Clone)]
struct PendingMovement {
    intent: MovementIntent,
    champion: Champion,
    participant: GameParticipant,
    path: Vec<MoveCoord>,
    start: MoveCoord,
    resolved: bool,
}

#[derive(Clone)]
struct MoveCandidate {
    pending_index: usize,
    from: MoveCoord,
    to: MoveCoord,
    movement_cost: u16,
    remaining_before: u16,
    path_distance: u16,
    tie_break: u64,
}

struct ObjectInteractionOutcome {
    event: Option<domm_game::ApiEventView>,
    stop_path: bool,
    participant_resources_changed: bool,
    object_changed: bool,
}

fn resolve_pending_movement(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<bool, ApiError> {
    let mut pending = load_pending_movements(session)?;
    if let Some(complete) = resolve_two_movement_crossing_fast(
        session,
        command_id,
        &mut pending,
        events,
        changed_subjects,
    )? {
        return Ok(complete);
    }
    if let Some(complete) = resolve_single_long_movement_fast(
        session,
        command_id,
        &mut pending,
        events,
        changed_subjects,
    )? {
        return Ok(complete);
    }
    let mut step_index = 0_u16;
    let mut microsteps_processed = 0_u16;
    loop {
        let candidates = movement_candidates(session, &pending, step_index)?;
        if candidates.is_empty() {
            break;
        }
        if microsteps_processed >= CANISTER_MOVEMENT_MICROSTEPS_PER_SYNC {
            park_partial_movements(
                session,
                command_id,
                &mut pending,
                step_index,
                events,
                changed_subjects,
            )?;
            return Ok(false);
        }

        let mut active = candidates
            .into_iter()
            .map(|candidate| (candidate.pending_index, candidate))
            .collect::<BTreeMap<_, _>>();
        resolve_tile_conflicts(
            session,
            command_id,
            &mut pending,
            &mut active,
            step_index,
            changed_subjects,
        )?;
        let crossing_resolution_complete = resolve_crossing_conflicts(
            session,
            command_id,
            &mut pending,
            &mut active,
            step_index,
            events,
            changed_subjects,
        )?;
        if !crossing_resolution_complete {
            park_partial_movements(
                session,
                command_id,
                &mut pending,
                step_index,
                events,
                changed_subjects,
            )?;
            return Ok(false);
        }
        let blocker_resolution_complete = resolve_blockers_and_guarded_objects(
            session,
            command_id,
            &mut pending,
            &mut active,
            step_index,
            events,
            changed_subjects,
        )?;
        if !blocker_resolution_complete {
            park_partial_movements(
                session,
                command_id,
                &mut pending,
                step_index,
                events,
                changed_subjects,
            )?;
            return Ok(false);
        }
        commit_active_moves(
            session,
            command_id,
            &mut pending,
            active,
            step_index,
            events,
            changed_subjects,
        )?;
        step_index = step_index.saturating_add(1);
        microsteps_processed = microsteps_processed.saturating_add(1);
    }

    for pending_move in &mut pending {
        if !pending_move.resolved && pending_move.path.len() <= usize::from(step_index) {
            mark_pending_resolved(session, command_id, pending_move, changed_subjects)?;
        }
    }
    Ok(true)
}

fn all_participants_ready_for_turn(session: &GameSession) -> Result<bool, ApiError> {
    let participants = sessions::page_participants_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    if participants.items.is_empty() {
        return Ok(false);
    }
    let ready_rows = turn_ready::page_turn_ready_by_session_turn(
        session.id(),
        session.current_turn,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    Ok(ready_rows.items.len() >= participants.items.len())
}

fn runtime_has_no_ready_participants(session: &GameSession) -> bool {
    let session_id = session.id().to_string();
    session_turn_runtime::with_runtime(&session_id, session.current_turn, |runtime| {
        runtime.ready_participants.is_empty()
    })
    .unwrap_or(false)
}

fn resolve_single_long_movement_fast(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<Option<bool>, ApiError> {
    if pending.len() != 1 {
        return Ok(None);
    }

    let final_coord = pending[0]
        .path
        .last()
        .copied()
        .expect("long movement path must have a final coord");
    if pending[0].path.len() <= 4
        && !fast_path_final_contact_exists(session, &pending[0], final_coord)?
    {
        return Ok(None);
    }
    let partial_event = command_response::append_public_event(
        session,
        command_id,
        format!(
            "movement_sync_incomplete:{}:{}:fast",
            session.id(),
            session.current_turn
        ),
        "movement_sync_incomplete".to_string(),
        Some("session".to_string()),
        Some(session.id().to_string()),
        format!(
            r#"{{"consumed_steps":{},"parked_intents":1}}"#,
            pending[0].path.len()
        ),
    )?;
    events.push(partial_event);

    if let Some(blocker) = map_visibility_occupancy::find_occupancy_cell(
        session.id(),
        final_coord.x,
        final_coord.y,
        "champion",
    )? {
        let moving_id = pending[0].champion.id().to_string();
        if blocker.blocking && blocker.occupant_id_text != moving_id {
            let (stop_step, stop_coord, movement_cost, remaining_after) =
                prepare_fast_path_stop(session, command_id, &mut pending[0])?;
            let mut blocker_champion = load_champion_by_text(session, &blocker.occupant_id_text)?
                .ok_or_else(|| {
                public_error("champion_not_found", "champion blocker was not found", true)
            })?;
            let enemy = blocker_champion.participant_id != pending[0].participant.id().key();
            if enemy {
                let Some(event) = mark_stationary_champion_encounter_pending(
                    session,
                    command_id,
                    pending,
                    0,
                    &mut blocker_champion,
                    final_coord,
                )?
                else {
                    return Ok(Some(false));
                };
                events.push(event);
                changed_subjects.push(command_response::changed(
                    "champion",
                    &blocker_champion.id().to_string(),
                    "status",
                ));
                changed_subjects.push(command_response::changed(
                    "champion",
                    &pending[0].champion.id().to_string(),
                    "status",
                ));
            }
            let start_coord = pending[0].start;
            stop_candidate_fast(
                session,
                command_id,
                &mut pending[0],
                stop_step,
                start_coord,
                stop_coord,
                movement_cost,
                remaining_after,
                if enemy {
                    "started_champion_battle"
                } else {
                    "stopped_champion_blocker"
                },
                Some(MovementPathStop {
                    reason: if enemy {
                        "enemy_champion_blocker".to_string()
                    } else {
                        "friendly_champion_blocker".to_string()
                    },
                    subject_kind: "champion".to_string(),
                    subject_id_text: blocker.occupant_id_text,
                    x: final_coord.x,
                    y: final_coord.y,
                }),
                changed_subjects,
            )?;
            return Ok(Some(!enemy));
        }
    }

    if let Some(object) = map_visibility_occupancy::find_world_object_by_session_xy(
        session.id(),
        final_coord.x,
        final_coord.y,
    )? {
        if let Some(neutral_id) = object.guarded_neutral_army_id {
            let neutral_id = Id::<NeutralArmy>::from_key(neutral_id);
            let (stop_step, stop_coord, movement_cost, remaining_after) =
                prepare_fast_path_stop(session, command_id, &mut pending[0])?;
            if let Some(event) = mark_neutral_encounter_pending(
                session,
                command_id,
                pending,
                0,
                neutral_id,
                object.id().to_string(),
                final_coord,
            )? {
                events.push(event);
                let start_coord = pending[0].start;
                stop_candidate_fast(
                    session,
                    command_id,
                    &mut pending[0],
                    stop_step,
                    start_coord,
                    stop_coord,
                    movement_cost,
                    remaining_after,
                    "started_neutral_battle",
                    Some(MovementPathStop {
                        reason: "guarded_object".to_string(),
                        subject_kind: "neutral_army".to_string(),
                        subject_id_text: neutral_id.to_string(),
                        x: final_coord.x,
                        y: final_coord.y,
                    }),
                    changed_subjects,
                )?;
                return Ok(Some(true));
            }
            return Ok(Some(false));
        }
    }

    let movement_cost = movement_cost_for_path(session, &pending[0].path)?;
    let remaining_after = effective_movement(&pending[0].champion, session.current_turn)
        .saturating_sub(movement_cost);
    pending[0].champion.x = final_coord.x;
    pending[0].champion.y = final_coord.y;
    pending[0].champion.chunk_x = chunk_coord(session, final_coord.x);
    pending[0].champion.chunk_y = chunk_coord(session, final_coord.y);
    pending[0].champion.movement_remaining = remaining_after;
    pending[0].champion.movement_turn = session.current_turn;
    pending[0].champion.last_command_id = Some(command_id.key());
    pending[0].champion = champions_artifacts::update_champion(pending[0].champion.clone())?;
    update_champion_occupancy(
        session.id(),
        command_id,
        pending[0].start,
        &pending[0].champion,
    )?;

    let interaction = apply_world_object_at(
        session,
        command_id,
        &mut pending[0].participant,
        &pending[0].champion,
        final_coord,
    )?;
    if let Some(event) = interaction.event {
        events.push(event);
    }
    if interaction.participant_resources_changed {
        pending[0].participant = sessions::update_participant(pending[0].participant.clone())?;
        changed_subjects.push(command_response::changed(
            "participant",
            &pending[0].participant.id().to_string(),
            "resources",
        ));
    }
    let final_step = pending[0]
        .path
        .len()
        .saturating_sub(1)
        .min(usize::from(u16::MAX)) as u16;
    record_movement_snapshot(
        session,
        command_id,
        &pending[0],
        final_step,
        pending[0].start,
        final_coord,
        movement_cost,
        remaining_after,
        if interaction.stop_path {
            "stopped_object_interaction"
        } else {
            "moved"
        },
        None,
    )?;
    pending[0].intent = movement::mark_intent_resolved(pending[0].intent.clone())?;
    mirror_runtime_pending_movement(session, &pending[0]);
    pending[0].resolved = true;
    changed_subjects.push(command_response::changed(
        "movement_intent",
        &pending[0].intent.id().to_string(),
        "resolve",
    ));
    changed_subjects.push(command_response::changed(
        "champion",
        &pending[0].champion.id().to_string(),
        "update",
    ));
    Ok(Some(true))
}

fn fast_path_final_contact_exists(
    session: &GameSession,
    pending_move: &PendingMovement,
    final_coord: MoveCoord,
) -> Result<bool, ApiError> {
    if map_visibility_occupancy::find_world_object_by_session_xy(
        session.id(),
        final_coord.x,
        final_coord.y,
    )?
    .and_then(|object| object.guarded_neutral_army_id)
    .is_some()
    {
        return Ok(true);
    }
    let Some(blocker) = map_visibility_occupancy::find_occupancy_cell(
        session.id(),
        final_coord.x,
        final_coord.y,
        "champion",
    )?
    else {
        return Ok(false);
    };
    Ok(blocker.blocking && blocker.occupant_id_text != pending_move.champion.id().to_string())
}

fn resolve_two_movement_crossing_fast(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<Option<bool>, ApiError> {
    if pending.len() != 2 {
        return Ok(None);
    }
    if pending[0].participant.id() == pending[1].participant.id() {
        return Ok(None);
    }

    let max_step = pending[0].path.len().min(pending[1].path.len());
    for step in 0..max_step {
        let left_from = if step == 0 {
            MoveCoord::new(pending[0].champion.x, pending[0].champion.y)
        } else {
            pending[0].path[step - 1]
        };
        let right_from = if step == 0 {
            MoveCoord::new(pending[1].champion.x, pending[1].champion.y)
        } else {
            pending[1].path[step - 1]
        };
        let left_to = pending[0].path[step];
        let right_to = pending[1].path[step];
        if left_to != right_from || right_to != left_from {
            continue;
        }

        let event = command_response::append_public_event(
            session,
            command_id,
            format!(
                "movement_sync_incomplete:{}:{}:crossing-fast",
                session.id(),
                session.current_turn
            ),
            "movement_sync_incomplete".to_string(),
            Some("session".to_string()),
            Some(session.id().to_string()),
            format!(
                r#"{{"consumed_steps":{},"parked_intents":2}}"#,
                step.saturating_add(1)
            ),
        )?;
        events.push(event);

        let step_index = step.min(usize::from(u16::MAX)) as u16;
        let left_start = pending[0].start;
        let right_start = pending[1].start;
        let left_cost = movement_cost_for_path(session, &pending[0].path[..step])?;
        let right_cost = movement_cost_for_path(session, &pending[1].path[..step])?;
        let left_remaining =
            apply_fast_path_position(session, command_id, &mut pending[0], left_from, left_cost);
        let right_remaining =
            apply_fast_path_position(session, command_id, &mut pending[1], right_from, right_cost);

        let Some(event) =
            mark_champion_encounter_pending(session, command_id, pending, 0, 1, left_to)?
        else {
            return Ok(Some(false));
        };
        events.push(event);
        changed_subjects.push(command_response::changed(
            "champion",
            &pending[0].champion.id().to_string(),
            "status",
        ));
        changed_subjects.push(command_response::changed(
            "champion",
            &pending[1].champion.id().to_string(),
            "status",
        ));

        let left_champion_id = pending[0].champion.id().to_string();
        let right_champion_id = pending[1].champion.id().to_string();
        stop_candidate_fast(
            session,
            command_id,
            &mut pending[0],
            step_index,
            left_start,
            left_from,
            left_cost,
            left_remaining,
            "started_crossing_battle",
            Some(MovementPathStop {
                reason: "crossing_conflict".to_string(),
                subject_kind: "champion".to_string(),
                subject_id_text: right_champion_id.clone(),
                x: left_to.x,
                y: left_to.y,
            }),
            changed_subjects,
        )?;
        stop_candidate_fast(
            session,
            command_id,
            &mut pending[1],
            step_index,
            right_start,
            right_from,
            right_cost,
            right_remaining,
            "started_crossing_battle",
            Some(MovementPathStop {
                reason: "crossing_conflict".to_string(),
                subject_kind: "champion".to_string(),
                subject_id_text: left_champion_id,
                x: right_to.x,
                y: right_to.y,
            }),
            changed_subjects,
        )?;
        return Ok(Some(true));
    }

    Ok(None)
}

fn prepare_fast_path_stop(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &mut PendingMovement,
) -> Result<(u16, MoveCoord, u16, u16), ApiError> {
    let stop_len = pending_move.path.len().saturating_sub(1);
    let stop_coord = if stop_len == 0 {
        pending_move.start
    } else {
        pending_move
            .path
            .get(stop_len - 1)
            .copied()
            .unwrap_or(pending_move.start)
    };
    let movement_cost = movement_cost_for_path(session, &pending_move.path[..stop_len])?;
    let remaining_after =
        apply_fast_path_position(session, command_id, pending_move, stop_coord, movement_cost);
    let stop_step = pending_move
        .path
        .len()
        .saturating_sub(1)
        .min(usize::from(u16::MAX)) as u16;
    Ok((stop_step, stop_coord, movement_cost, remaining_after))
}

fn apply_fast_path_position(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &mut PendingMovement,
    coord: MoveCoord,
    movement_cost: u16,
) -> u16 {
    let remaining_after = effective_movement(&pending_move.champion, session.current_turn)
        .saturating_sub(movement_cost);
    pending_move.champion.x = coord.x;
    pending_move.champion.y = coord.y;
    pending_move.champion.chunk_x = chunk_coord(session, coord.x);
    pending_move.champion.chunk_y = chunk_coord(session, coord.y);
    pending_move.champion.movement_remaining = remaining_after;
    pending_move.champion.movement_turn = session.current_turn;
    pending_move.champion.last_command_id = Some(command_id.key());
    remaining_after
}

fn movement_cost_for_path(session: &GameSession, path: &[MoveCoord]) -> Result<u16, ApiError> {
    path.iter()
        .map(|coord| movement_cost_at(session, *coord).map(u16::from))
        .try_fold(0_u16, |sum, cost| cost.map(|cost| sum.saturating_add(cost)))
}

#[allow(clippy::too_many_arguments)]
fn stop_candidate_fast(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &mut PendingMovement,
    step_index: u16,
    from: MoveCoord,
    to: MoveCoord,
    movement_cost: u16,
    remaining_after: u16,
    outcome: &str,
    stop: Option<MovementPathStop>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    record_movement_snapshot(
        session,
        command_id,
        pending_move,
        step_index,
        from,
        to,
        movement_cost,
        remaining_after,
        outcome,
        stop,
    )?;
    pending_move.champion = champions_artifacts::update_champion(pending_move.champion.clone())?;
    update_champion_occupancy(
        session.id(),
        command_id,
        pending_move.start,
        &pending_move.champion,
    )?;
    pending_move.intent = movement::mark_intent_resolved(pending_move.intent.clone())?;
    mirror_runtime_pending_movement(session, pending_move);
    pending_move.resolved = true;
    changed_subjects.push(command_response::changed(
        "movement_intent",
        &pending_move.intent.id().to_string(),
        "resolve",
    ));
    Ok(())
}

fn reschedule_current_turn_jobs_for_manual_sync(session: &GameSession) -> Result<(), ApiError> {
    let due_at = manual_sync_retry_at();
    update_current_turn_jobs(session.id(), session.current_turn, |job| {
        system_job_service::reschedule_job(job, due_at, None).map(|_| ())
    })
}

fn complete_current_turn_jobs(
    session_id: Id<GameSession>,
    turn_number: u32,
) -> Result<(), ApiError> {
    update_current_turn_jobs(session_id, turn_number, |job| {
        system_job_repo::complete_system_job(job).map(|_| ())
    })
}

fn update_current_turn_jobs<F>(
    session_id: Id<GameSession>,
    turn_number: u32,
    mut update: F,
) -> Result<(), ApiError>
where
    F: FnMut(SystemJob) -> Result<(), ApiError>,
{
    let mut cursor = None;
    loop {
        let page = system_job_repo::page_system_jobs_by_session(
            session_id,
            domm_game::MAX_LIST_LIMIT,
            cursor,
        )?;
        for job in page.items {
            if !matches!(
                job.status.as_str(),
                system_job_repo::STATUS_RUNNING | system_job_repo::STATUS_SCHEDULED
            ) {
                continue;
            }
            if !matches!(job.job_kind.as_str(), "turn_resolution" | "turn_deadline") {
                continue;
            }
            if job.turn_number != Some(turn_number) {
                continue;
            }
            update(job)?;
        }
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }
    Ok(())
}

fn preview_path_stop(
    _context: &session_context::SessionCallerContext,
    _champion: &Champion,
    path: &[MoveCoord],
) -> Result<Option<MovementPathStop>, ApiError> {
    for coord in path {
        let scenario = domm_game::first_playable_scenario();
        if let Some(object) = scenario
            .mines
            .iter()
            .chain(scenario.external_dwellings.iter())
            .chain(scenario.central_objectives.iter())
            .find(|object| object.x == coord.x && object.y == coord.y)
        {
            if let Some(neutral_key) = &object.guard_neutral_army_key {
                return Ok(Some(MovementPathStop {
                    reason: "guarded_object".to_string(),
                    subject_kind: "neutral_army".to_string(),
                    subject_id_text: neutral_key.clone(),
                    x: coord.x,
                    y: coord.y,
                }));
            }
            return Ok(Some(MovementPathStop {
                reason: scenario_object_stop_reason(&object.object_slug).to_string(),
                subject_kind: "world_object".to_string(),
                subject_id_text: object.key.clone(),
                x: coord.x,
                y: coord.y,
            }));
        }
        if let Some(pile) = scenario
            .resource_piles
            .iter()
            .find(|pile| pile.x == coord.x && pile.y == coord.y)
        {
            return Ok(Some(MovementPathStop {
                reason: "resource_pile".to_string(),
                subject_kind: "world_object".to_string(),
                subject_id_text: pile.key.clone(),
                x: coord.x,
                y: coord.y,
            }));
        }
    }
    Ok(None)
}

fn scenario_object_stop_reason(object_slug: &str) -> &'static str {
    match object_slug {
        "gold-mine" | "crystal-mine" => "mine",
        "misery-beacon" => "central_objective",
        "mudhook-den" => "external_dwelling",
        _ => "world_object",
    }
}

fn validate_no_friendly_champion_blocker(
    context: &session_context::SessionCallerContext,
    champion: &Champion,
    path: &[MoveCoord],
) -> Result<(), ApiError> {
    for coord in path {
        let Some(blocker) = map_visibility_occupancy::find_occupancy_cell(
            context.session.id(),
            coord.x,
            coord.y,
            "champion",
        )?
        else {
            continue;
        };
        if !blocker.blocking || blocker.occupant_id_text == champion.id().to_string() {
            continue;
        }
        let Some(blocker_champion) =
            load_champion_by_text(&context.session, &blocker.occupant_id_text)?
        else {
            continue;
        };
        if blocker_champion.participant_id == context.participant.id().key() {
            return Err(public_error(
                "friendly_champion_occupied",
                "movement path enters a tile occupied by an owned champion",
                false,
            ));
        }
    }
    Ok(())
}

fn load_pending_movements(session: &GameSession) -> Result<Vec<PendingMovement>, ApiError> {
    if let Some(pending) = runtime_pending_movements_for_session(session)? {
        return Ok(pending);
    }
    let items = pending_movement_intents_for_session(session)?;
    let mut pending = Vec::new();
    for (intent, participant) in items {
        let Some(champion) =
            champions_artifacts::load_champion(Id::<Champion>::from_key(intent.champion_id))?
        else {
            continue;
        };
        pending.push(PendingMovement {
            path: parse_path_text(&intent.path_json)?,
            start: MoveCoord::new(champion.x, champion.y),
            intent,
            champion,
            participant,
            resolved: false,
        });
    }
    pending.sort_by(|left, right| {
        left.champion
            .id()
            .to_string()
            .cmp(&right.champion.id().to_string())
    });
    Ok(pending)
}

fn pending_movement_intents_for_session(
    session: &GameSession,
) -> Result<Vec<(MovementIntent, GameParticipant)>, ApiError> {
    let active_participants = sessions::page_participants_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let active_participants_by_id = active_participants
        .items
        .into_iter()
        .map(|participant| (participant.id().key(), participant))
        .collect::<BTreeMap<_, _>>();
    if let Some(mut intents) =
        runtime_pending_movement_intents_for_session(session, &active_participants_by_id)
    {
        intents.sort_by(|left, right| left.0.champion_id.cmp(&right.0.champion_id));
        return Ok(intents);
    }
    let mut intents = movement::page_movement_intents_by_status(
        session.id(),
        session.current_turn,
        "pending",
        domm_game::MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN,
        None,
    )?
    .items
    .into_iter()
    .filter_map(|intent| {
        active_participants_by_id
            .get(&intent.actor_participant_id)
            .cloned()
            .map(|participant| (intent, participant))
    })
    .collect::<Vec<_>>();
    intents.sort_by(|left, right| left.0.champion_id.cmp(&right.0.champion_id));
    Ok(intents)
}

fn runtime_pending_movements_for_session(
    session: &GameSession,
) -> Result<Option<Vec<PendingMovement>>, ApiError> {
    let session_id = session.id().to_string();
    match session_turn_runtime::with_runtime(&session_id, session.current_turn, |runtime| {
        if runtime.intents.is_empty() {
            return Ok(None);
        }
        let mut pending = Vec::new();
        for runtime_intent in runtime
            .intents
            .iter()
            .filter(|intent| intent.status == "pending")
        {
            let Some(intent) = runtime_intent.durable_intent.clone() else {
                return Ok(None);
            };
            if intent.session_id != session.id().key()
                || intent.turn_number != session.current_turn
                || intent.status != "pending"
            {
                return Ok(None);
            }
            let (Some(champion), Some(participant)) = (
                runtime_intent.champion.clone(),
                runtime_intent.participant.clone(),
            ) else {
                return Ok(None);
            };
            if participant.session_id != session.id().key()
                || participant.status != "active"
                || participant.id().key() != intent.actor_participant_id
                || champion.session_id != session.id().key()
                || champion.participant_id != participant.id().key()
                || champion.id().key() != intent.champion_id
            {
                return Ok(None);
            }
            pending.push(PendingMovement {
                path: parse_path_text(&intent.path_json)?,
                start: MoveCoord::new(champion.x, champion.y),
                intent,
                champion,
                participant,
                resolved: false,
            });
        }
        pending.sort_by(|left, right| {
            left.champion
                .id()
                .to_string()
                .cmp(&right.champion.id().to_string())
        });
        Ok(Some(pending))
    }) {
        Some(result) => result,
        None => Ok(None),
    }
}

fn runtime_pending_movement_intents_for_session(
    session: &GameSession,
    active_participants_by_id: &BTreeMap<Ulid, GameParticipant>,
) -> Option<Vec<(MovementIntent, GameParticipant)>> {
    let session_id = session.id().to_string();
    session_turn_runtime::with_runtime(&session_id, session.current_turn, |runtime| {
        if runtime.intents.is_empty() {
            return None;
        }
        Some(
            runtime
                .intents
                .iter()
                .filter(|intent| intent.status == "pending")
                .filter_map(|intent| intent.durable_intent.clone())
                .filter(|intent| {
                    intent.session_id == session.id().key()
                        && intent.turn_number == session.current_turn
                        && intent.status == "pending"
                })
                .filter_map(|intent| {
                    active_participants_by_id
                        .get(&intent.actor_participant_id)
                        .cloned()
                        .map(|participant| (intent, participant))
                })
                .collect::<Vec<_>>(),
        )
    })
    .flatten()
}

fn mirror_runtime_pending_movement(session: &GameSession, pending_move: &PendingMovement) {
    let session_id = session.id().to_string();
    session_turn_runtime::with_runtime_mut(&session_id, session.current_turn, |runtime| {
        runtime.upsert_intent(session_turn_runtime::RuntimeMovementIntent::from_pending(
            pending_move.intent.clone(),
            pending_move.champion.clone(),
            pending_move.participant.clone(),
        ));
    });
}

fn movement_candidates(
    session: &GameSession,
    pending: &[PendingMovement],
    step_index: u16,
) -> Result<Vec<MoveCandidate>, ApiError> {
    let mut candidates = Vec::new();
    for (pending_index, pending_move) in pending.iter().enumerate() {
        if pending_move.resolved || pending_move.champion.status != "active" {
            continue;
        }
        let Some(to) = pending_move.path.get(usize::from(step_index)).copied() else {
            continue;
        };
        let from = MoveCoord::new(pending_move.champion.x, pending_move.champion.y);
        let movement_cost = u16::from(movement_cost_at(session, to)?);
        candidates.push(MoveCandidate {
            pending_index,
            from,
            to,
            movement_cost,
            remaining_before: effective_movement(&pending_move.champion, session.current_turn),
            path_distance: step_index.saturating_add(1),
            tie_break: movement_tie_break(
                session.seed,
                session.current_turn,
                &pending_move.champion.id().to_string(),
                to,
            ),
        });
    }
    candidates.sort_by(|left, right| {
        pending[left.pending_index]
            .champion
            .id()
            .to_string()
            .cmp(&pending[right.pending_index].champion.id().to_string())
    });
    Ok(candidates)
}

fn resolve_tile_conflicts(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    active: &mut BTreeMap<usize, MoveCandidate>,
    step_index: u16,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    let mut by_tile: BTreeMap<MoveCoord, Vec<MoveCandidate>> = BTreeMap::new();
    for candidate in active.values() {
        by_tile
            .entry(candidate.to)
            .or_default()
            .push(candidate.clone());
    }
    for group in by_tile.values().filter(|group| group.len() > 1) {
        let winner = tile_conflict_winner(group);
        for candidate in group
            .iter()
            .filter(|candidate| candidate.pending_index != winner.pending_index)
        {
            active.remove(&candidate.pending_index);
            stop_candidate(
                session,
                command_id,
                pending,
                candidate,
                step_index,
                "stopped_tile_conflict",
                Some(MovementPathStop {
                    reason: "tile_conflict".to_string(),
                    subject_kind: "tile".to_string(),
                    subject_id_text: format!("{},{}", candidate.to.x, candidate.to.y),
                    x: candidate.to.x,
                    y: candidate.to.y,
                }),
                changed_subjects,
            )?;
        }
    }
    Ok(())
}

fn resolve_crossing_conflicts(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    active: &mut BTreeMap<usize, MoveCandidate>,
    step_index: u16,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<bool, ApiError> {
    let keys = active.keys().copied().collect::<Vec<_>>();
    let mut stopped = BTreeSet::new();
    for left_pos in 0..keys.len() {
        for right_pos in left_pos + 1..keys.len() {
            let left_key = keys[left_pos];
            let right_key = keys[right_pos];
            if stopped.contains(&left_key) || stopped.contains(&right_key) {
                continue;
            }
            let Some(left) = active.get(&left_key).cloned() else {
                continue;
            };
            let Some(right) = active.get(&right_key).cloned() else {
                continue;
            };
            if left.to == right.from && right.to == left.from {
                let enemy = pending[left.pending_index].participant.id()
                    != pending[right.pending_index].participant.id();
                if enemy {
                    let Some(event) = mark_champion_encounter_pending(
                        session,
                        command_id,
                        pending,
                        left.pending_index,
                        right.pending_index,
                        left.to,
                    )?
                    else {
                        return Ok(false);
                    };
                    events.push(event);
                    changed_subjects.push(command_response::changed(
                        "champion",
                        &pending[left.pending_index].champion.id().to_string(),
                        "status",
                    ));
                    changed_subjects.push(command_response::changed(
                        "champion",
                        &pending[right.pending_index].champion.id().to_string(),
                        "status",
                    ));
                }
                active.remove(&left_key);
                active.remove(&right_key);
                stopped.insert(left_key);
                stopped.insert(right_key);
                stop_candidate(
                    session,
                    command_id,
                    pending,
                    &left,
                    step_index,
                    if enemy {
                        "started_crossing_battle"
                    } else {
                        "stopped_crossing_conflict"
                    },
                    Some(MovementPathStop {
                        reason: "crossing_conflict".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: pending[right.pending_index].champion.id().to_string(),
                        x: left.to.x,
                        y: left.to.y,
                    }),
                    changed_subjects,
                )?;
                stop_candidate(
                    session,
                    command_id,
                    pending,
                    &right,
                    step_index,
                    if enemy {
                        "started_crossing_battle"
                    } else {
                        "stopped_crossing_conflict"
                    },
                    Some(MovementPathStop {
                        reason: "crossing_conflict".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: pending[left.pending_index].champion.id().to_string(),
                        x: right.to.x,
                        y: right.to.y,
                    }),
                    changed_subjects,
                )?;
            }
        }
    }
    Ok(true)
}

fn resolve_blockers_and_guarded_objects(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    active: &mut BTreeMap<usize, MoveCandidate>,
    step_index: u16,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<bool, ApiError> {
    let keys = active.keys().copied().collect::<Vec<_>>();
    for key in keys {
        let Some(candidate) = active.get(&key).cloned() else {
            continue;
        };
        if candidate.movement_cost > candidate.remaining_before {
            active.remove(&key);
            stop_candidate(
                session,
                command_id,
                pending,
                &candidate,
                step_index,
                "stopped_budget_exhausted",
                None,
                changed_subjects,
            )?;
            continue;
        }

        if let Some(object) = map_visibility_occupancy::find_world_object_by_session_xy(
            session.id(),
            candidate.to.x,
            candidate.to.y,
        )? {
            if let Some(neutral_id) = object.guarded_neutral_army_id {
                active.remove(&key);
                let neutral_id = Id::<NeutralArmy>::from_key(neutral_id);
                let Some(event) = mark_neutral_encounter_pending(
                    session,
                    command_id,
                    pending,
                    candidate.pending_index,
                    neutral_id,
                    object.id().to_string(),
                    candidate.to,
                )?
                else {
                    return Ok(false);
                };
                events.push(event);
                changed_subjects.push(command_response::changed(
                    "champion",
                    &pending[candidate.pending_index].champion.id().to_string(),
                    "status",
                ));
                stop_candidate(
                    session,
                    command_id,
                    pending,
                    &candidate,
                    step_index,
                    "started_neutral_battle",
                    Some(MovementPathStop {
                        reason: "guarded_object".to_string(),
                        subject_kind: "neutral_army".to_string(),
                        subject_id_text: neutral_id.to_string(),
                        x: candidate.to.x,
                        y: candidate.to.y,
                    }),
                    changed_subjects,
                )?;
                continue;
            }
        }

        if let Some(blocker) = map_visibility_occupancy::find_occupancy_cell(
            session.id(),
            candidate.to.x,
            candidate.to.y,
            "champion",
        )? {
            let moving_id = pending[candidate.pending_index].champion.id().to_string();
            if blocker.blocking && blocker.occupant_id_text != moving_id {
                active.remove(&key);
                let blocker_index = pending.iter().position(|pending_move| {
                    pending_move.champion.id().to_string() == blocker.occupant_id_text
                });
                let stationary_blocker = if blocker_index.is_none() {
                    load_champion_by_text(session, &blocker.occupant_id_text)?
                } else {
                    None
                };
                let enemy = blocker_index
                    .map(|index| {
                        pending[index].participant.id()
                            != pending[candidate.pending_index].participant.id()
                    })
                    .or_else(|| {
                        stationary_blocker.as_ref().map(|champion| {
                            champion.participant_id
                                != pending[candidate.pending_index].participant.id().key()
                        })
                    })
                    .unwrap_or(false);
                if let Some(blocker_index) = blocker_index {
                    active.remove(&blocker_index);
                    if enemy {
                        let Some(event) = mark_champion_encounter_pending(
                            session,
                            command_id,
                            pending,
                            candidate.pending_index,
                            blocker_index,
                            candidate.to,
                        )?
                        else {
                            return Ok(false);
                        };
                        events.push(event);
                    }
                } else if enemy {
                    let mut blocker_champion =
                        stationary_blocker.expect("enemy stationary blocker should be loaded");
                    let Some(event) = mark_stationary_champion_encounter_pending(
                        session,
                        command_id,
                        pending,
                        candidate.pending_index,
                        &mut blocker_champion,
                        candidate.to,
                    )?
                    else {
                        return Ok(false);
                    };
                    events.push(event);
                    changed_subjects.push(command_response::changed(
                        "champion",
                        &blocker_champion.id().to_string(),
                        "status",
                    ));
                }
                if enemy {
                    changed_subjects.push(command_response::changed(
                        "champion",
                        &pending[candidate.pending_index].champion.id().to_string(),
                        "status",
                    ));
                }
                stop_candidate(
                    session,
                    command_id,
                    pending,
                    &candidate,
                    step_index,
                    if enemy {
                        "started_champion_battle"
                    } else {
                        "stopped_champion_blocker"
                    },
                    Some(MovementPathStop {
                        reason: if enemy {
                            "enemy_champion_blocker".to_string()
                        } else {
                            "friendly_champion_blocker".to_string()
                        },
                        subject_kind: "champion".to_string(),
                        subject_id_text: blocker.occupant_id_text,
                        x: candidate.to.x,
                        y: candidate.to.y,
                    }),
                    changed_subjects,
                )?;
                continue;
            }
        }

        if let Some(town) = map_visibility_occupancy::find_occupancy_cell(
            session.id(),
            candidate.to.x,
            candidate.to.y,
            "town",
        )? {
            if town.blocking {
                active.remove(&key);
                let town_row = load_town_by_text(session, &town.occupant_id_text)?;
                let enemy_town = town_row.owner_participant_id.is_some_and(|owner| {
                    owner != pending[candidate.pending_index].participant.id().key()
                });
                let outcome = if enemy_town {
                    let battle = battle_start::start_town_battle(
                        session,
                        command_id,
                        &pending[candidate.pending_index].champion,
                        pending[candidate.pending_index].participant.id(),
                        &town_row,
                        candidate.to,
                    )?;
                    pending[candidate.pending_index].champion.status = "in_battle".to_string();
                    pending[candidate.pending_index].champion.in_battle_id =
                        Some(battle.id().key());
                    pending[candidate.pending_index].champion.last_command_id =
                        Some(command_id.key());
                    pending[candidate.pending_index].champion =
                        champions_artifacts::update_champion(
                            pending[candidate.pending_index].champion.clone(),
                        )?;
                    let event = command_response::append_public_event(
                        session,
                        command_id,
                        format!(
                            "town_contact:{}:{}:{}",
                            pending[candidate.pending_index].champion.id(),
                            town_row.id(),
                            session.current_turn
                        ),
                        "town_encounter_pending".to_string(),
                        Some("town".to_string()),
                        Some(town_row.id().to_string()),
                        format!(
                            r#"{{"battle_id":"{}","champion_id":"{}","town_id":"{}","x":{},"y":{}}}"#,
                            battle.id(),
                            pending[candidate.pending_index].champion.id(),
                            town_row.id(),
                            candidate.to.x,
                            candidate.to.y
                        ),
                    )?;
                    events.push(event);
                    changed_subjects.push(command_response::changed(
                        "champion",
                        &pending[candidate.pending_index].champion.id().to_string(),
                        "status",
                    ));
                    "started_town_battle"
                } else {
                    "stopped_town_interaction"
                };
                stop_candidate(
                    session,
                    command_id,
                    pending,
                    &candidate,
                    step_index,
                    outcome,
                    Some(MovementPathStop {
                        reason: if enemy_town {
                            "enemy_town".to_string()
                        } else {
                            "town_interaction".to_string()
                        },
                        subject_kind: "town".to_string(),
                        subject_id_text: town.occupant_id_text,
                        x: candidate.to.x,
                        y: candidate.to.y,
                    }),
                    changed_subjects,
                )?;
                if enemy_town {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn commit_active_moves(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    active: BTreeMap<usize, MoveCandidate>,
    step_index: u16,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    for candidate in active.values() {
        commit_candidate_move(
            session,
            command_id,
            pending,
            candidate,
            step_index,
            "moved",
            None,
            changed_subjects,
        )?;
        let interaction = apply_world_object_at(
            session,
            command_id,
            &mut pending[candidate.pending_index].participant,
            &pending[candidate.pending_index].champion,
            candidate.to,
        )?;
        if let Some(event) = interaction.event {
            events.push(event);
        }
        if interaction.participant_resources_changed {
            pending[candidate.pending_index]
                .participant
                .last_action_turn = session.current_turn;
            pending[candidate.pending_index].participant =
                sessions::update_participant(pending[candidate.pending_index].participant.clone())?;
            changed_subjects.push(command_response::changed(
                "participant",
                &pending[candidate.pending_index]
                    .participant
                    .id()
                    .to_string(),
                "resources",
            ));
        }
        if interaction.object_changed {
            changed_subjects.push(command_response::changed(
                "world_object",
                &format!("{},{}", candidate.to.x, candidate.to.y),
                "update",
            ));
        }
        if interaction.stop_path
            || pending[candidate.pending_index].path.len() <= usize::from(step_index) + 1
        {
            record_movement_snapshot(
                session,
                command_id,
                &pending[candidate.pending_index],
                step_index,
                candidate.from,
                candidate.to,
                candidate.movement_cost,
                pending[candidate.pending_index].champion.movement_remaining,
                if interaction.stop_path {
                    "stopped_object_interaction"
                } else {
                    "moved"
                },
                None,
            )?;
            mark_pending_resolved(
                session,
                command_id,
                &mut pending[candidate.pending_index],
                changed_subjects,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_candidate_move(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    candidate: &MoveCandidate,
    step_index: u16,
    outcome: &str,
    stop: Option<MovementPathStop>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    let pending_move = &mut pending[candidate.pending_index];
    let old = MoveCoord::new(pending_move.champion.x, pending_move.champion.y);
    let remaining_after = candidate
        .remaining_before
        .saturating_sub(candidate.movement_cost);
    pending_move.champion.x = candidate.to.x;
    pending_move.champion.y = candidate.to.y;
    pending_move.champion.chunk_x = chunk_coord(session, candidate.to.x);
    pending_move.champion.chunk_y = chunk_coord(session, candidate.to.y);
    pending_move.champion.movement_remaining = remaining_after;
    pending_move.champion.movement_turn = session.current_turn;
    pending_move.champion.last_command_id = Some(command_id.key());
    update_known_champion_projection(session, &pending_move.participant, &pending_move.champion)?;
    if outcome != "moved" {
        record_movement_snapshot(
            session,
            command_id,
            pending_move,
            step_index,
            old,
            candidate.to,
            candidate.movement_cost,
            remaining_after,
            outcome,
            stop,
        )?;
    }
    changed_subjects.push(command_response::changed(
        "champion",
        &pending_move.champion.id().to_string(),
        "update",
    ));
    Ok(())
}

fn update_known_champion_projection(
    _session: &GameSession,
    _participant: &GameParticipant,
    _champion: &Champion,
) -> Result<(), ApiError> {
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stop_candidate(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    candidate: &MoveCandidate,
    step_index: u16,
    outcome: &str,
    stop: Option<MovementPathStop>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    let pending_move = &mut pending[candidate.pending_index];
    record_movement_snapshot(
        session,
        command_id,
        pending_move,
        step_index,
        candidate.from,
        candidate.from,
        0,
        candidate.remaining_before,
        outcome,
        stop,
    )?;
    mark_pending_resolved(session, command_id, pending_move, changed_subjects)
}

fn mark_pending_resolved(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &mut PendingMovement,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    if pending_move.resolved {
        return Ok(());
    }
    pending_move.champion = champions_artifacts::update_champion(pending_move.champion.clone())?;
    update_champion_occupancy(
        Id::<GameSession>::from_key(pending_move.intent.session_id),
        command_id,
        pending_move.start,
        &pending_move.champion,
    )?;
    let visibility_rows = refresh_champion_visibility(session, command_id, pending_move)?;
    if visibility_rows > 0 {
        changed_subjects.push(command_response::changed(
            "visibility",
            &pending_move.participant.id().to_string(),
            "update",
        ));
    }
    pending_move.intent = movement::mark_intent_resolved(pending_move.intent.clone())?;
    pending_move.resolved = true;
    pending_move.participant.last_action_turn = pending_move.intent.turn_number;
    pending_move.participant = sessions::update_participant(pending_move.participant.clone())?;
    mirror_runtime_pending_movement(session, pending_move);
    changed_subjects.push(command_response::changed(
        "movement_intent",
        &pending_move.intent.id().to_string(),
        "resolve",
    ));
    Ok(())
}

fn park_partial_movements(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    consumed_steps: u16,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    let consumed = usize::from(consumed_steps);
    let mut parked = 0_u32;
    for pending_move in pending
        .iter_mut()
        .filter(|pending_move| !pending_move.resolved)
    {
        if consumed == 0 || pending_move.path.len() <= consumed {
            continue;
        }
        let partial_to = MoveCoord::new(pending_move.champion.x, pending_move.champion.y);
        let movement_cost = pending_move
            .path
            .iter()
            .take(consumed)
            .map(|coord| movement_cost_at(session, *coord).map(u16::from))
            .try_fold(0_u16, |sum, cost| cost.map(|cost| sum.saturating_add(cost)))?;
        record_movement_snapshot(
            session,
            command_id,
            pending_move,
            consumed_steps.saturating_sub(1),
            pending_move.start,
            partial_to,
            movement_cost,
            pending_move.champion.movement_remaining,
            "partial_sync",
            None,
        )?;

        pending_move.champion =
            champions_artifacts::update_champion(pending_move.champion.clone())?;
        update_champion_occupancy(
            session.id(),
            command_id,
            pending_move.start,
            &pending_move.champion,
        )?;
        let visibility_rows = refresh_champion_visibility(session, command_id, pending_move)?;
        if visibility_rows > 0 {
            changed_subjects.push(command_response::changed(
                "visibility",
                &pending_move.participant.id().to_string(),
                "update",
            ));
        }

        let remaining_path = pending_move.path[consumed..].to_vec();
        let remaining_text = path_text(&remaining_path);
        pending_move.intent.path_json = remaining_text.clone();
        pending_move.intent.path_hash = command_response::payload_hash(
            "movement_remaining_path",
            &pending_move.champion.id().to_string(),
            &pending_move.intent.id().to_string(),
            &remaining_text,
        );
        pending_move.intent = movement::update_movement_intent(pending_move.intent.clone())?;
        pending_move.path = remaining_path;
        pending_move.start = partial_to;
        mirror_runtime_pending_movement(session, pending_move);
        parked = parked.saturating_add(1);
        changed_subjects.push(command_response::changed(
            "movement_intent",
            &pending_move.intent.id().to_string(),
            "partial",
        ));
        changed_subjects.push(command_response::changed(
            "champion",
            &pending_move.champion.id().to_string(),
            "partial_move",
        ));
    }

    command_response::ensure_command_effect(
        session.id(),
        command_id,
        format!(
            "movement_cursor:{}:{}",
            session.current_turn, consumed_steps
        ),
        "movement_cursor".to_string(),
        "session".to_string(),
        session.id().to_string(),
        format!(r#"{{"consumed_steps":{consumed_steps},"parked_intents":{parked}}}"#),
    )?;
    let event = command_response::append_public_event(
        session,
        command_id,
        format!(
            "movement_sync_incomplete:{}:{}:{}",
            session.id(),
            session.current_turn,
            consumed_steps
        ),
        "movement_sync_incomplete".to_string(),
        Some("session".to_string()),
        Some(session.id().to_string()),
        format!(r#"{{"consumed_steps":{consumed_steps},"parked_intents":{parked}}}"#),
    )?;
    events.push(event);
    Ok(())
}

fn refresh_champion_visibility(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &PendingMovement,
) -> Result<u32, ApiError> {
    let mut by_chunk: BTreeMap<(u16, u16), Vec<(u16, u16)>> = BTreeMap::new();
    let radius = i32::from(pending_move.champion.vision_radius);
    let center_x = i32::from(pending_move.champion.x);
    let center_y = i32::from(pending_move.champion.y);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() + dy.abs() > radius {
                continue;
            }
            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0
                || y < 0
                || x >= i32::from(session.map_width)
                || y >= i32::from(session.map_height)
            {
                continue;
            }
            let x = u16::try_from(x).unwrap_or(0);
            let y = u16::try_from(y).unwrap_or(0);
            by_chunk
                .entry((chunk_coord(session, x), chunk_coord(session, y)))
                .or_default()
                .push((x, y));
        }
    }

    let mut updated_rows = 0_u32;
    for ((chunk_x, chunk_y), tiles) in by_chunk {
        let Some(mut visibility) = map_visibility_occupancy::find_visibility_chunk(
            pending_move.participant.id(),
            chunk_x,
            chunk_y,
        )?
        else {
            continue;
        };
        let width = chunk_width(session, chunk_x);
        let chunk_size = u16::from(session.chunk_size);
        let mut discovered = visibility.discovered_blob.to_vec();
        let mut visible = visibility.visible_blob.to_vec();
        for (x, y) in tiles {
            let local_x = x % chunk_size;
            let local_y = y % chunk_size;
            let index = usize::from(local_y) * usize::from(width) + usize::from(local_x);
            domm_game::set_visibility_bit(&mut discovered, index);
            domm_game::set_visibility_bit(&mut visible, index);
        }
        visibility.discovered_blob = Blob::from(discovered);
        visibility.visible_blob = Blob::from(visible);
        visibility.visible_turn = session.current_turn;
        map_visibility_occupancy::update_visibility_chunk(visibility)?;
        updated_rows = updated_rows.saturating_add(1);
    }

    if updated_rows > 0 {
        command_response::ensure_command_effect(
            session.id(),
            command_id,
            format!(
                "visibility:{}:{}",
                pending_move.champion.id(),
                session.current_turn
            ),
            "visibility_refresh".to_string(),
            "participant".to_string(),
            pending_move.participant.id().to_string(),
            format!(r#"{{"visibility_rows":{updated_rows}}}"#),
        )?;
    }
    Ok(updated_rows)
}

#[allow(clippy::too_many_arguments)]
fn create_known_object_if_missing(
    session: &GameSession,
    participant_id: Id<GameParticipant>,
    subject_kind: &str,
    subject_id_text: &str,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    redacted_json: String,
) -> Result<bool, ApiError> {
    if map_visibility_occupancy::find_known_object(participant_id, subject_kind, subject_id_text)?
        .is_some()
    {
        return Ok(false);
    }
    map_visibility_occupancy::create_known_object(
        session.id(),
        participant_id,
        subject_kind.to_string(),
        subject_id_text.to_string(),
        x,
        y,
        chunk_x,
        chunk_y,
        "visible".to_string(),
        session.current_turn,
        Some(redacted_json),
    )?;
    Ok(true)
}

fn known_redacted_json(subject_kind: &str, subject_id_text: &str) -> String {
    format!(
        r#"{{"type":"{}","scenario_key":"{}","state":"last_known"}}"#,
        command_response::escape_json(subject_kind),
        command_response::escape_json(subject_id_text)
    )
}

fn update_champion_occupancy(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    old: MoveCoord,
    champion: &Champion,
) -> Result<(), ApiError> {
    if old.x == champion.x && old.y == champion.y {
        return Ok(());
    }
    let occupant_id = champion.id().to_string();
    let old_occupancy =
        map_visibility_occupancy::find_occupancy_cell(session_id, old.x, old.y, "champion")?;
    let mut occupancy = old_occupancy
        .clone()
        .filter(|occupancy| occupancy.occupant_id_text == occupant_id);
    if occupancy.is_none() {
        occupancy = map_visibility_occupancy::find_occupancy_by_occupant(
            session_id,
            "champion",
            &occupant_id,
            0,
        )?;
    }
    if occupancy.is_none() {
        occupancy = old_occupancy.filter(|occupancy| occupancy.occupant_kind == "champion");
    }
    if let Some(mut occupancy) = occupancy {
        occupancy.x = champion.x;
        occupancy.y = champion.y;
        occupancy.chunk_x = champion.chunk_x;
        occupancy.chunk_y = champion.chunk_y;
        occupancy.occupant_kind = "champion".to_string();
        occupancy.occupant_id_text = occupant_id;
        occupancy.occupant_cell_index = 0;
        occupancy.last_command_id = Some(command_id.key());
        map_visibility_occupancy::update_occupancy_cell(occupancy)?;
        return Ok(());
    }
    map_visibility_occupancy::create_occupancy_cell(
        session_id,
        champion.x,
        champion.y,
        champion.chunk_x,
        champion.chunk_y,
        "champion".to_string(),
        "champion".to_string(),
        occupant_id,
        0,
        true,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_movement_snapshot(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &PendingMovement,
    step_index: u16,
    from: MoveCoord,
    to: MoveCoord,
    movement_cost: u16,
    remaining_after: u16,
    outcome: &str,
    stop: Option<MovementPathStop>,
) -> Result<(), ApiError> {
    let interaction_kind = stop.as_ref().map(|stop| stop.subject_kind.clone());
    let interaction_id_text = stop.as_ref().map(|stop| stop.subject_id_text.clone());
    if movement::find_movement_snapshot(command_id, pending_move.intent.id(), step_index)?.is_none()
    {
        movement::create_movement_snapshot(
            session.id(),
            command_id,
            pending_move.intent.id(),
            pending_move.champion.id(),
            pending_move.participant.id(),
            session.current_turn,
            step_index,
            from.x,
            from.y,
            to.x,
            to.y,
            movement_cost,
            remaining_after,
            outcome.to_string(),
            interaction_kind.clone(),
            interaction_id_text.clone(),
        )?;
    }

    Ok(())
}

fn mark_champion_encounter_pending(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    attacker_index: usize,
    defender_index: usize,
    coord: MoveCoord,
) -> Result<Option<domm_game::ApiEventView>, ApiError> {
    let Some(battle) = battle_start::start_champion_battle(
        session,
        command_id,
        &pending[attacker_index].champion,
        pending[attacker_index].participant.id(),
        &pending[defender_index].champion,
        coord,
    )?
    else {
        return Ok(None);
    };
    for index in [attacker_index, defender_index] {
        pending[index].champion.status = "in_battle".to_string();
        pending[index].champion.in_battle_id = Some(battle.id().key());
        pending[index].champion.last_command_id = Some(command_id.key());
        pending[index].champion =
            champions_artifacts::update_champion(pending[index].champion.clone())?;
    }
    command_response::append_public_event(
        session,
        command_id,
        format!(
            "champion_contact:{}:{}:{}",
            pending[attacker_index].champion.id(),
            pending[defender_index].champion.id(),
            session.current_turn
        ),
        "champion_encounter_pending".to_string(),
        Some("champion".to_string()),
        Some(pending[defender_index].champion.id().to_string()),
        format!(
            r#"{{"battle_id":"{}","attacker_champion_id":"{}","defender_champion_id":"{}","x":{},"y":{}}}"#,
            battle.id(),
            pending[attacker_index].champion.id(),
            pending[defender_index].champion.id(),
            coord.x,
            coord.y
        ),
    )
    .map(Some)
}

fn mark_stationary_champion_encounter_pending(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    attacker_index: usize,
    defender: &mut Champion,
    coord: MoveCoord,
) -> Result<Option<domm_game::ApiEventView>, ApiError> {
    let Some(battle) = battle_start::start_champion_battle(
        session,
        command_id,
        &pending[attacker_index].champion,
        pending[attacker_index].participant.id(),
        defender,
        coord,
    )?
    else {
        return Ok(None);
    };
    pending[attacker_index].champion.status = "in_battle".to_string();
    pending[attacker_index].champion.in_battle_id = Some(battle.id().key());
    pending[attacker_index].champion.last_command_id = Some(command_id.key());
    defender.status = "in_battle".to_string();
    defender.in_battle_id = Some(battle.id().key());
    defender.last_command_id = Some(command_id.key());
    champions_artifacts::update_champion(defender.clone())?;
    command_response::append_public_event(
        session,
        command_id,
        format!(
            "champion_contact:{}:{}:{}",
            pending[attacker_index].champion.id(),
            defender.id(),
            session.current_turn
        ),
        "champion_encounter_pending".to_string(),
        Some("champion".to_string()),
        Some(defender.id().to_string()),
        format!(
            r#"{{"battle_id":"{}","attacker_champion_id":"{}","defender_champion_id":"{}","x":{},"y":{}}}"#,
            battle.id(),
            pending[attacker_index].champion.id(),
            defender.id(),
            coord.x,
            coord.y
        ),
    )
    .map(Some)
}

fn mark_neutral_encounter_pending(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    pending: &mut [PendingMovement],
    pending_index: usize,
    neutral_id: Id<NeutralArmy>,
    object_id: String,
    coord: MoveCoord,
) -> Result<Option<domm_game::ApiEventView>, ApiError> {
    let Some(battle) = start_neutral_battle(
        session,
        command_id,
        &mut pending[pending_index],
        neutral_id,
        &object_id,
        coord,
    )?
    else {
        return Ok(None);
    };
    pending[pending_index].champion.status = "in_battle".to_string();
    pending[pending_index].champion.in_battle_id = Some(battle.id().key());
    pending[pending_index].champion.last_command_id = Some(command_id.key());
    pending[pending_index].champion =
        champions_artifacts::update_champion(pending[pending_index].champion.clone())?;
    command_response::append_public_event(
        session,
        command_id,
        format!(
            "neutral_contact:{}:{}:{}",
            pending[pending_index].champion.id(),
            object_id,
            session.current_turn
        ),
        "neutral_encounter_pending".to_string(),
        Some("neutral_army".to_string()),
        Some(neutral_id.to_string()),
        format!(
            r#"{{"battle_id":"{}","champion_id":"{}","neutral_army_id":"{}","object_id":"{}","x":{},"y":{}}}"#,
            battle.id(),
            pending[pending_index].champion.id(),
            neutral_id,
            object_id,
            coord.x,
            coord.y
        ),
    )
    .map(Some)
}

fn start_neutral_battle(
    session: &GameSession,
    command_id: Id<GameCommand>,
    pending_move: &mut PendingMovement,
    neutral_id: Id<NeutralArmy>,
    object_id: &str,
    coord: MoveCoord,
) -> Result<Option<Battle>, ApiError> {
    if let Some(existing) = battles::find_battle_by_attacker(pending_move.champion.id())? {
        if existing.defender_neutral_army_id == Some(neutral_id.key())
            && (existing.state == "active" || existing.state.starts_with("starting"))
        {
            if existing.state.starts_with("starting") {
                return continue_neutral_battle_start(
                    session,
                    command_id,
                    existing,
                    pending_move,
                    neutral_id,
                    object_id,
                );
            }
            ensure_neutral_battle_started_effect(
                session, command_id, &existing, neutral_id, object_id,
            )?;
            battle_runtime::adopt_active_battle_from_rows(session, existing.clone())?;
            return Ok(Some(existing));
        }
    }

    let turn_seed = neutral_battle_seed(session, &pending_move.champion, neutral_id, coord);
    let action_deadline_at =
        Timestamp::from_millis(Timestamp::now().as_millis().saturating_add(
            i64::try_from(domm_game::BATTLE_ACTION_DEADLINE_MS).unwrap_or(i64::MAX),
        ));
    battles::create_battle(
        session.id(),
        "starting".to_string(),
        "neutral".to_string(),
        Some(pending_move.champion.id()),
        None,
        None,
        Some(neutral_id),
        "attacker".to_string(),
        domm_game::BATTLE_GRID_WIDTH,
        domm_game::BATTLE_GRID_HEIGHT,
        domm_game::BATTLE_MAX_ROUNDS,
        turn_seed,
        session.current_turn,
        Some(action_deadline_at),
        command_id,
    )?;
    Ok(None)
}

fn continue_neutral_battle_start(
    session: &GameSession,
    command_id: Id<GameCommand>,
    mut battle: Battle,
    pending_move: &PendingMovement,
    neutral_id: Id<NeutralArmy>,
    object_id: &str,
) -> Result<Option<Battle>, ApiError> {
    let attacker_stacks = battles::page_battle_stacks_by_side(
        battle.id(),
        "attacker",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let defender_stacks = battles::page_battle_stacks_by_side(
        battle.id(),
        "defender",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    match battle.state.as_str() {
        "starting" => {
            if attacker_stacks.items.is_empty() {
                create_neutral_battle_attacker_stacks(command_id, &battle, pending_move)?;
            }
            battle.state = "starting_attacker".to_string();
            battles::update_battle(battle)?;
            return Ok(None);
        }
        "starting_attacker" => {
            if defender_stacks.items.is_empty() {
                create_neutral_battle_defender_stacks(command_id, &battle, neutral_id)?;
            }
            battle.state = "starting_defender".to_string();
            battles::update_battle(battle)?;
            return Ok(None);
        }
        "starting_defender" => {
            create_initial_battle_obstacles(command_id, &battle)?;
            battle.state = "starting_obstacles".to_string();
            battles::update_battle(battle)?;
            Ok(None)
        }
        "starting_obstacles" => {
            let mut stacks = attacker_stacks.items;
            stacks.extend(defender_stacks.items);
            if let Some(active_stack) = select_initial_active_stack(session, &battle, &mut stacks) {
                battle.active_stack_id = Some(active_stack.id().key());
                battle.active_side = active_stack.side.clone();
            }
            battle.state = "active".to_string();
            battle.action_deadline_at = Some(battle_start::fresh_action_deadline_at());
            battle = battles::update_battle(battle)?;
            battle_service::schedule_battle_timeout_job(session.id(), &battle)?;

            let mut neutral = neutrals::load_neutral_army(neutral_id)?.ok_or_else(|| {
                public_error("neutral_army_not_found", "neutral army not found", true)
            })?;
            neutral.state = "in_battle".to_string();
            neutral.last_command_id = Some(command_id.key());
            neutrals::update_neutral_army(neutral)?;
            ensure_neutral_battle_started_effect(
                session, command_id, &battle, neutral_id, object_id,
            )?;
            battle_runtime::adopt_active_battle_from_rows(session, battle.clone())?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn ensure_neutral_battle_started_effect(
    session: &GameSession,
    command_id: Id<GameCommand>,
    battle: &Battle,
    neutral_id: Id<NeutralArmy>,
    object_id: &str,
) -> Result<(), ApiError> {
    let effect_key = format!("battle:{}", battle.id());
    if commands_events_effects::find_applied_command_effect_by_session_key(
        session.id(),
        &effect_key,
    )?
    .is_some()
    {
        return Ok(());
    }
    command_response::ensure_command_effect(
        session.id(),
        command_id,
        effect_key,
        "battle_started".to_string(),
        "battle".to_string(),
        battle.id().to_string(),
        format!(
            r#"{{"battle_id":"{}","battle_type":"neutral","neutral_army_id":"{}","object_id":"{}"}}"#,
            battle.id(),
            neutral_id,
            command_response::escape_json(object_id)
        ),
    )
}

fn create_neutral_battle_attacker_stacks(
    command_id: Id<GameCommand>,
    battle: &Battle,
    pending_move: &PendingMovement,
) -> Result<Vec<BattleStack>, ApiError> {
    let mut stacks = Vec::new();
    let spell_status_keys = battle_spell_status_keys(&pending_move.champion)?;
    for stack in champions_artifacts::page_champion_army_stacks(
        pending_move.champion.id(),
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    .into_iter()
    .filter(|stack| stack.status == "active" && stack.quantity > 0)
    {
        let unit_id = Id::<UnitDefinition>::from_key(stack.unit_id);
        let unit = content::load_unit(unit_id)?
            .ok_or_else(|| public_error("unit_not_found", "battle unit not found", true))?;
        let y = match stack.slot_index {
            0 => 3,
            1 => 6,
            value => 2_u8
                .saturating_add(value)
                .min(domm_game::BATTLE_GRID_HEIGHT - 1),
        };
        let mut battle_stack = create_battle_stack_from_unit(
            command_id,
            battle.id(),
            &unit,
            Some(pending_move.participant.id()),
            "attacker",
            stack.slot_index,
            "champion_army",
            Some(stack.id().to_string()),
            stack.slot_index,
            stack.quantity,
            stack.front_hp,
            1,
            y,
        )?;
        if !spell_status_keys.is_empty() {
            battle_stack.status_keys = spell_status_keys.clone();
            battle_stack = battles::update_battle_stack(battle_stack)?;
        }
        battles::create_battle_occupancy(
            battle.id(),
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        stacks.push(battle_stack);
    }
    Ok(stacks)
}

fn battle_spell_status_keys(champion: &Champion) -> Result<Vec<String>, ApiError> {
    if !champion.skill_keys.iter().any(|key| key == "sour_sorcery") {
        return Ok(Vec::new());
    }
    let mut status_keys =
        champions_artifacts::page_champion_spells(champion.id(), domm_game::MAX_LIST_LIMIT, None)?
            .items
            .into_iter()
            .filter_map(|known| known.spell_slug)
            .filter(|slug| !slug.is_empty())
            .map(|slug| format!("battle_spell:{slug}"))
            .collect::<Vec<_>>();
    status_keys.sort();
    status_keys.dedup();
    Ok(status_keys)
}

fn create_neutral_battle_defender_stacks(
    command_id: Id<GameCommand>,
    battle: &Battle,
    neutral_id: Id<NeutralArmy>,
) -> Result<Vec<BattleStack>, ApiError> {
    let mut stacks = Vec::new();
    for stack in neutrals::page_neutral_army_stacks(neutral_id, domm_game::MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .filter(|stack| stack.quantity > 0)
    {
        let unit_id = Id::<UnitDefinition>::from_key(stack.unit_id);
        let unit = content::load_unit(unit_id)?
            .ok_or_else(|| public_error("unit_not_found", "battle unit not found", true))?;
        let y = 4_u8
            .saturating_add(stack.slot_index)
            .min(domm_game::BATTLE_GRID_HEIGHT - 1);
        let battle_stack = create_battle_stack_from_unit(
            command_id,
            battle.id(),
            &unit,
            None,
            "defender",
            stack.slot_index,
            "neutral_army",
            Some(stack.id().to_string()),
            stack.slot_index,
            stack.quantity,
            stack.front_hp,
            domm_game::BATTLE_GRID_WIDTH - 2,
            y,
        )?;
        battles::create_battle_occupancy(
            battle.id(),
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        stacks.push(battle_stack);
    }
    Ok(stacks)
}

#[allow(clippy::too_many_arguments)]
fn create_battle_stack_from_unit(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    unit: &UnitDefinition,
    owner_participant_id: Option<Id<GameParticipant>>,
    side: &str,
    slot_index: u8,
    origin_kind: &str,
    origin_stack_id_text: Option<String>,
    origin_slot_index: u8,
    quantity: u32,
    front_hp: u16,
    battle_x: u8,
    battle_y: u8,
) -> Result<BattleStack, ApiError> {
    battles::create_battle_stack(
        battle_id,
        unit.id(),
        owner_participant_id,
        side.to_string(),
        slot_index,
        origin_kind.to_string(),
        origin_stack_id_text,
        origin_slot_index,
        unit.attack,
        unit.defense,
        unit.damage_min,
        unit.damage_max,
        unit.max_hp,
        unit.speed,
        unit.initiative,
        unit.ranged,
        unit.flying,
        quantity,
        front_hp,
        unit.shots,
        battle_x,
        battle_y,
        command_id,
    )
    .map_err(Into::into)
}

fn create_initial_battle_obstacles(
    command_id: Id<GameCommand>,
    battle: &Battle,
) -> Result<(), ApiError> {
    battles::create_battle_obstacle(battle.id(), "rubble".to_string(), 5, 4, command_id)?;
    battles::create_battle_obstacle(battle.id(), "broken-cart".to_string(), 6, 5, command_id)?;
    Ok(())
}

fn select_initial_active_stack(
    session: &GameSession,
    battle: &Battle,
    stacks: &mut [BattleStack],
) -> Option<BattleStack> {
    stacks.sort_by(|left, right| {
        right
            .initiative
            .cmp(&left.initiative)
            .then_with(|| right.speed.cmp(&left.speed))
            .then_with(|| {
                battle_stack_tie_break(session, battle, left)
                    .cmp(&battle_stack_tie_break(session, battle, right))
            })
            .then_with(|| left.id().to_string().cmp(&right.id().to_string()))
    });
    stacks.first().cloned()
}

fn neutral_battle_seed(
    session: &GameSession,
    champion: &Champion,
    neutral_id: Id<NeutralArmy>,
    coord: MoveCoord,
) -> u64 {
    let hash = command_response::payload_hash(
        "battle_turn_seed",
        &session.seed.to_string(),
        &session.current_turn.to_string(),
        &format!(
            "{}:{}:{}:{}:{}",
            champion.id(),
            neutral_id,
            coord.x,
            coord.y,
            session.id()
        ),
    );
    u64::from_str_radix(hash.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

fn battle_stack_tie_break(session: &GameSession, battle: &Battle, stack: &BattleStack) -> u64 {
    let hash = command_response::payload_hash(
        "battle_initiative_tie",
        &session.seed.to_string(),
        &battle.current_round.to_string(),
        &format!("{}:{}:{}", battle.id(), stack.id(), stack.side),
    );
    u64::from_str_radix(hash.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

fn tile_conflict_winner(group: &[MoveCandidate]) -> MoveCandidate {
    let mut sorted = group.to_vec();
    sorted.sort_by(|left, right| {
        right
            .remaining_before
            .cmp(&left.remaining_before)
            .then_with(|| left.path_distance.cmp(&right.path_distance))
            .then_with(|| right.tie_break.cmp(&left.tie_break))
            .then_with(|| left.pending_index.cmp(&right.pending_index))
    });
    sorted
        .into_iter()
        .next()
        .expect("tile conflict group must be non-empty")
}

fn movement_tie_break(seed: u64, turn_number: u32, champion_id: &str, coord: MoveCoord) -> u64 {
    let hash = command_response::payload_hash(
        "movement.tile_conflict",
        &seed.to_string(),
        &turn_number.to_string(),
        &format!("{champion_id}:{}:{}", coord.x, coord.y),
    );
    u64::from_str_radix(hash.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

fn apply_world_object_at(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    participant: &mut GameParticipant,
    champion: &Champion,
    coord: MoveCoord,
) -> Result<ObjectInteractionOutcome, ApiError> {
    let Some(mut object) =
        map_visibility_occupancy::find_world_object_by_session_xy(session.id(), coord.x, coord.y)?
    else {
        return Ok(ObjectInteractionOutcome {
            event: None,
            stop_path: false,
            participant_resources_changed: false,
            object_changed: false,
        });
    };
    if object.guarded_neutral_army_id.is_some() {
        return Ok(ObjectInteractionOutcome {
            event: None,
            stop_path: true,
            participant_resources_changed: false,
            object_changed: false,
        });
    }
    if object.state == "collected"
        || (object.scoring_kind == "mine"
            && object.owner_participant_id == Some(participant.id().key())
            && object.state == "captured")
    {
        return Ok(ObjectInteractionOutcome {
            event: None,
            stop_path: true,
            participant_resources_changed: false,
            object_changed: false,
        });
    }

    object.last_visited_turn = session.current_turn;
    object.last_command_id = Some(command_id.key());

    let event_type = if object.scoring_kind == "resource_pile" {
        apply_reward_json(
            session.id(),
            participant,
            command_id,
            session.current_turn,
            &object,
        )?;
        object.state = "collected".to_string();
        "resource_picked_up"
    } else if object.scoring_kind == "mine" {
        object.owner_participant_id = Some(participant.id().key());
        object.state = "captured".to_string();
        object.captured_turn = session.current_turn;
        object.income_started_turn = session.current_turn.saturating_add(1);
        "mine_captured"
    } else {
        "world_object_visited"
    };

    if event_type != "resource_picked_up"
        && let Some(subject_id_text) = object
            .instance_json
            .as_deref()
            .and_then(|json| json_string_field(json, "scenario_key"))
    {
        create_known_object_if_missing(
            session,
            participant.id(),
            "world_object",
            &subject_id_text,
            object.x,
            object.y,
            object.chunk_x,
            object.chunk_y,
            known_redacted_json("world_object", &subject_id_text),
        )?;
    }

    map_visibility_occupancy::create_participant_object_visit(
        session.id(),
        object.id(),
        participant.id(),
        "once".to_string(),
        object.scoring_kind.clone(),
        session.current_turn,
    )?;
    map_visibility_occupancy::update_world_object(object.clone())?;
    let event = command_response::append_public_event(
        session,
        command_id,
        format!("object_visit:{}:{}", participant.id(), object.id()),
        event_type.to_string(),
        Some("world_object".to_string()),
        Some(object.id().to_string()),
        format!(
            r#"{{"champion_id":"{}","object_id":"{}"}}"#,
            champion.id(),
            object.id()
        ),
    )
    .map(Some)?;
    Ok(ObjectInteractionOutcome {
        event,
        stop_path: true,
        participant_resources_changed: event_type == "resource_picked_up",
        object_changed: true,
    })
}

#[allow(dead_code)]
fn hide_known_world_object(
    participant_id: Id<GameParticipant>,
    object: &WorldObject,
    turn_number: u32,
) -> Result<(), ApiError> {
    let Some(subject_id_text) = object
        .instance_json
        .as_deref()
        .and_then(|json| json_string_field(json, "scenario_key"))
    else {
        return Ok(());
    };
    let Some(mut known) = map_visibility_occupancy::find_known_object(
        participant_id,
        "world_object",
        &subject_id_text,
    )?
    else {
        return Ok(());
    };
    known.visibility = "hidden".to_string();
    known.last_seen_turn = turn_number;
    map_visibility_occupancy::update_known_object(known)?;
    Ok(())
}

fn materialize_income(
    session: &mut domm_degens_schema::schema::GameSession,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
    participant: &mut GameParticipant,
    turn_number: u32,
) -> Result<Vec<domm_game::ApiEventView>, ApiError> {
    if participant.last_income_turn >= turn_number {
        return Ok(Vec::new());
    }
    let mut gold_income = 0_u32;
    for object in map_visibility_occupancy::page_world_objects_by_owner_scoring_state(
        session.id(),
        participant.id(),
        "mine",
        "captured",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    {
        if object.income_started_turn <= turn_number {
            gold_income = gold_income.saturating_add(250);
        }
    }
    participant.last_income_turn = turn_number;
    if gold_income == 0 {
        return Ok(Vec::new());
    }
    apply_resource_delta(
        session.id(),
        participant,
        command_id,
        format!("income:turn:{turn_number}:gold"),
        turn_number,
        "gold",
        i64::from(gold_income),
        "income",
    )?;
    economy::create_resource_turn_summary(
        session.id(),
        participant.id(),
        turn_number,
        format!(r#"{{"kind":"income","gold":{gold_income}}}"#),
    )
    .ok();
    command_response::append_public_event(
        session,
        command_id,
        format!("income:{}:{turn_number}", participant.id()),
        "income_materialized".to_string(),
        Some("participant".to_string()),
        Some(participant.id().to_string()),
        format!(r#"{{"gold":{gold_income}}}"#),
    )
    .map(|event| vec![event])
}

fn apply_reward_json(
    session_id: Id<GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<GameCommand>,
    turn_number: u32,
    object: &WorldObject,
) -> Result<(), ApiError> {
    let json = object.instance_json.as_deref().unwrap_or("{}");
    for (resource_key, delta) in [
        ("gold", json_u32_field(json, "gold")),
        ("wood", json_u32_field(json, "wood")),
        ("stone", json_u32_field(json, "stone")),
        ("iron", json_u32_field(json, "iron")),
        ("crystal", json_u32_field(json, "crystal")),
        ("ember", json_u32_field(json, "ember")),
        ("aether", json_u32_field(json, "aether")),
    ] {
        if delta == 0 {
            continue;
        }
        apply_resource_delta(
            session_id,
            participant,
            command_id,
            format!("pickup:{}:{resource_key}", object.id()),
            turn_number,
            resource_key,
            i64::from(delta),
            "pickup",
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_resource_delta(
    session_id: Id<domm_degens_schema::schema::GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<domm_degens_schema::schema::GameCommand>,
    ledger_key: String,
    turn_number: u32,
    resource_key: &str,
    delta: i64,
    reason: &str,
) -> Result<(), ApiError> {
    if let Some(entry) = economy::find_resource_ledger_entry(command_id, &ledger_key)? {
        reconcile_resource_balance(participant, resource_key, entry.balance_after)?;
        participant.last_resource_command_id = Some(command_id.key());
        return Ok(());
    }
    let balance_after = match resource_key {
        "gold" => {
            participant.gold = apply_u64_delta(participant.gold, delta)?;
            participant.gold
        }
        "wood" => {
            participant.wood = apply_u32_delta(participant.wood, delta)?;
            u64::from(participant.wood)
        }
        "stone" => {
            participant.stone = apply_u32_delta(participant.stone, delta)?;
            u64::from(participant.stone)
        }
        "iron" => {
            participant.iron = apply_u32_delta(participant.iron, delta)?;
            u64::from(participant.iron)
        }
        "crystal" => {
            participant.crystal = apply_u32_delta(participant.crystal, delta)?;
            u64::from(participant.crystal)
        }
        "ember" => {
            participant.ember = apply_u32_delta(participant.ember, delta)?;
            u64::from(participant.ember)
        }
        "aether" => {
            participant.aether = apply_u32_delta(participant.aether, delta)?;
            u64::from(participant.aether)
        }
        _ => {
            return Err(public_error(
                "unknown_resource",
                "unknown resource key",
                false,
            ));
        }
    };
    economy::create_resource_ledger_entry(
        session_id,
        participant.id(),
        command_id,
        ledger_key,
        turn_number,
        resource_key.to_string(),
        delta,
        balance_after,
        reason.to_string(),
        "applied".to_string(),
    )?;
    participant.last_resource_command_id = Some(command_id.key());
    Ok(())
}

fn reconcile_resource_balance(
    participant: &mut GameParticipant,
    resource_key: &str,
    balance_after: u64,
) -> Result<(), ApiError> {
    match resource_key {
        "gold" => {
            participant.gold = balance_after;
            Ok(())
        }
        "wood" => {
            participant.wood = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "stone" => {
            participant.stone = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "iron" => {
            participant.iron = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "crystal" => {
            participant.crystal = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "ember" => {
            participant.ember = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        "aether" => {
            participant.aether = u32::try_from(balance_after).map_err(|_| {
                public_error("resource_cap_exceeded", "resource cap exceeded", false)
            })?;
            Ok(())
        }
        _ => Err(public_error(
            "unknown_resource",
            "unknown resource key",
            false,
        )),
    }
}

fn resolve_owned_champion(
    context: &session_context::SessionCallerContext,
    champion_id: &str,
) -> Result<Champion, ApiError> {
    if let Some(champion) = runtime_owned_active_champion(context, champion_id) {
        return Ok(champion);
    }
    let champion = resolve_champion(&context.session, champion_id)?;
    if champion.participant_id != context.participant.id().key() {
        return Err(public_error(
            "not_champion_owner",
            "caller does not own this champion",
            false,
        ));
    }
    if champion.status != "active" {
        return Err(public_error(
            "champion_not_active",
            "champion is not active for movement",
            false,
        ));
    }
    Ok(champion)
}

fn runtime_owned_active_champion(
    context: &session_context::SessionCallerContext,
    champion_id: &str,
) -> Option<Champion> {
    let champion_id = Ulid::from_str(champion_id).ok()?;
    let session_id = context.session.id().to_string();
    session_turn_runtime::with_runtime(&session_id, context.session.current_turn, |runtime| {
        runtime
            .intents
            .iter()
            .filter_map(|intent| intent.champion.as_ref())
            .find(|champion| {
                champion.id().key() == champion_id
                    && champion.session_id == context.session.id().key()
                    && champion.participant_id == context.participant.id().key()
                    && champion.status == "active"
            })
            .cloned()
    })
    .flatten()
}

fn resolve_champion(
    session: &domm_degens_schema::schema::GameSession,
    champion_id: &str,
) -> Result<Champion, ApiError> {
    if let Ok(id) = Ulid::from_str(champion_id).map(Id::<Champion>::from_key) {
        return champions_artifacts::load_champion(id)?
            .ok_or_else(|| public_error("not_found", "champion not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.champion_key == champion_id)
        .ok_or_else(|| public_error("not_found", "champion not found", false))?;
    champions_artifacts::find_champion_by_session_xy(
        session.id(),
        start.champion_x,
        start.champion_y,
    )?
    .ok_or_else(|| public_error("not_found", "champion not found", false))
}

fn load_champion_by_text(
    session: &GameSession,
    champion_id: &str,
) -> Result<Option<Champion>, ApiError> {
    if let Ok(id) = Ulid::from_str(champion_id).map(Id::<Champion>::from_key) {
        return champions_artifacts::load_champion(id);
    }
    let scenario = domm_game::first_playable_scenario();
    let Some(start) = scenario
        .starts
        .iter()
        .find(|start| start.champion_key == champion_id)
    else {
        return Ok(None);
    };
    champions_artifacts::find_champion_by_session_xy(
        session.id(),
        start.champion_x,
        start.champion_y,
    )
}

fn load_town_by_text(session: &GameSession, town_id: &str) -> Result<Town, ApiError> {
    if let Ok(id) = Ulid::from_str(town_id).map(Id::<Town>::from_key) {
        return towns::load_town(id)?
            .ok_or_else(|| public_error("town_not_found", "town not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.town_key == town_id)
        .ok_or_else(|| public_error("town_not_found", "town not found", false))?;
    towns::find_town_by_session_xy(session.id(), start.town_x, start.town_y)?
        .ok_or_else(|| public_error("town_not_found", "town not found", false))
}

fn validate_path_limit(path: &[MoveCoord]) -> Result<(), ApiError> {
    if path.is_empty() {
        return Err(public_error(
            "movement_path_empty",
            "movement path must include at least one step",
            false,
        ));
    }
    if path.len() > domm_game::MAX_MOVE_PATH_STEPS_LIMIT {
        return Err(ApiError::new(
            "movement_path_too_long",
            "movement path exceeds the v1 public query limit",
            false,
        )
        .with_details(format!(
            r#"{{"path_len":{},"max_len":{}}}"#,
            path.len(),
            domm_game::MAX_MOVE_PATH_STEPS_LIMIT
        )));
    }
    Ok(())
}

fn validate_path_cost(
    session: &GameSession,
    champion: &Champion,
    path: &[MoveCoord],
) -> Result<u16, ApiError> {
    let mut total = 0_u16;
    for coord in path {
        let flags = flags_at(session, *coord)?;
        if flags & domm_game::MAP_FLAG_BLOCKING_TERRAIN != 0 {
            return Err(public_error(
                "movement_path_impassable",
                "movement path crosses impassable terrain",
                false,
            ));
        }
        total = total.saturating_add(u16::from(movement_cost_at(session, *coord)?));
    }
    let available = effective_movement(champion, session.current_turn);
    if total > available {
        return Err(public_error(
            "movement_path_too_expensive",
            "movement path exceeds available movement",
            false,
        ));
    }
    Ok(total)
}

fn validate_path_bounds(session: &GameSession, path: &[MoveCoord]) -> Result<(), ApiError> {
    for coord in path {
        if coord.x >= session.map_width || coord.y >= session.map_height {
            return Err(public_error(
                "movement_path_out_of_bounds",
                "movement path leaves the session map",
                false,
            ));
        }
    }
    Ok(())
}

fn validate_path_adjacency(start_x: u16, start_y: u16, path: &[MoveCoord]) -> Result<(), ApiError> {
    let mut previous = MoveCoord::new(start_x, start_y);
    for coord in path {
        if !coord.is_adjacent_to(previous) {
            return Err(public_error(
                "movement_path_not_adjacent",
                "movement path contains a non-adjacent step",
                false,
            ));
        }
        previous = *coord;
    }
    Ok(())
}

fn chunks_touched(session: &domm_degens_schema::schema::GameSession, path: &[MoveCoord]) -> u32 {
    let chunk_size = u16::from(session.chunk_size);
    path.iter()
        .map(|coord| (coord.x / chunk_size, coord.y / chunk_size))
        .collect::<BTreeSet<_>>()
        .len()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn movement_cost_at(session: &GameSession, coord: MoveCoord) -> Result<u8, ApiError> {
    if let Some(cost) = static_first_playable_movement_cost_at(session, coord) {
        return Ok(cost);
    }
    chunk_cell_blob_value(session, coord, |chunk| chunk.movement_blob.as_slice())
}

fn flags_at(session: &GameSession, coord: MoveCoord) -> Result<u8, ApiError> {
    if let Some(flags) = static_first_playable_flags_at(session, coord) {
        return Ok(flags);
    }
    chunk_cell_blob_value(session, coord, |chunk| chunk.flags_blob.as_slice())
}

fn static_first_playable_movement_cost_at(session: &GameSession, coord: MoveCoord) -> Option<u8> {
    static_first_playable_map_value(session, coord, |map, coord| {
        map.movement_cost_at(coord.x, coord.y)
    })
}

fn static_first_playable_flags_at(session: &GameSession, coord: MoveCoord) -> Option<u8> {
    static_first_playable_map_value(session, coord, |map, coord| map.flags_at(coord.x, coord.y))
}

fn static_first_playable_map_value(
    session: &GameSession,
    coord: MoveCoord,
    value: impl FnOnce(&domm_game::FirstPlayableMapState, MoveCoord) -> Option<u8>,
) -> Option<u8> {
    if session.map_width != domm_game::FIRST_PLAYABLE_MAP_WIDTH
        || session.map_height != domm_game::FIRST_PLAYABLE_MAP_HEIGHT
        || session.chunk_size != domm_game::FIRST_PLAYABLE_CHUNK_SIZE
    {
        return None;
    }
    let map = domm_game::build_first_playable_map_state();
    value(&map, coord)
}

fn chunk_cell_blob_value(
    session: &GameSession,
    coord: MoveCoord,
    blob: impl Fn(&domm_degens_schema::schema::MapChunk) -> &[u8],
) -> Result<u8, ApiError> {
    validate_path_bounds(session, &[coord])?;
    let chunk_size = u16::from(session.chunk_size);
    let chunk_x = coord.x / chunk_size;
    let chunk_y = coord.y / chunk_size;
    let Some(chunk) = map_visibility_occupancy::find_map_chunk(session.id(), chunk_x, chunk_y)?
    else {
        return Err(public_error(
            "movement_path_missing_chunk",
            "movement path references an unloaded map chunk",
            true,
        ));
    };
    let local_x = coord.x % chunk_size;
    let local_y = coord.y % chunk_size;
    let index = usize::from(local_y) * usize::from(chunk.width) + usize::from(local_x);
    blob(&chunk).get(index).copied().ok_or_else(|| {
        public_error(
            "movement_path_missing_chunk_cell",
            "movement path references a missing map cell",
            true,
        )
    })
}

fn effective_movement(champion: &Champion, turn_number: u32) -> u16 {
    if champion.movement_turn == turn_number {
        champion.movement_remaining
    } else {
        champion.movement_max
    }
}

fn chunk_coord(session: &GameSession, value: u16) -> u16 {
    value / u16::from(session.chunk_size)
}

fn chunk_width(session: &GameSession, chunk_x: u16) -> u16 {
    let chunk_size = u16::from(session.chunk_size);
    let origin_x = chunk_x.saturating_mul(chunk_size);
    session.map_width.saturating_sub(origin_x).min(chunk_size)
}

fn path_text(path: &[MoveCoord]) -> String {
    path.iter()
        .map(|coord| format!("{},{}", coord.x, coord.y))
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_path_text(value: &str) -> Result<Vec<MoveCoord>, ApiError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(';')
        .map(|part| {
            let (x, y) = part.split_once(',').ok_or_else(|| {
                public_error(
                    "invalid_movement_path",
                    "stored movement path is invalid",
                    true,
                )
            })?;
            Ok(MoveCoord::new(
                x.parse().map_err(|_| {
                    public_error(
                        "invalid_movement_path",
                        "stored movement path is invalid",
                        true,
                    )
                })?,
                y.parse().map_err(|_| {
                    public_error(
                        "invalid_movement_path",
                        "stored movement path is invalid",
                        true,
                    )
                })?,
            ))
        })
        .collect()
}

fn json_u32_field(json: &str, field: &str) -> u32 {
    let needle = format!(r#""{field}":"#);
    let Some(start) = json.find(&needle).map(|index| index + needle.len()) else {
        return 0;
    };
    let rest = &json[start..];
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    rest.get(..end)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!(r#""{field}":""#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn apply_u64_delta(value: u64, delta: i64) -> Result<u64, ApiError> {
    if delta.is_negative() {
        value
            .checked_sub(delta.unsigned_abs())
            .ok_or_else(|| public_error("insufficient_resources", "not enough resources", false))
    } else {
        Ok(value.saturating_add(delta as u64))
    }
}

fn apply_u32_delta(value: u32, delta: i64) -> Result<u32, ApiError> {
    let value = apply_u64_delta(u64::from(value), delta)?;
    u32::try_from(value)
        .map_err(|_| public_error("resource_cap_exceeded", "resource cap exceeded", false))
}

fn turn_deadline() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(i64::try_from(domm_game::TURN_DURATION_MS).unwrap_or(i64::MAX)),
    )
}

fn partial_retry_at() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(PARTIAL_TURN_RETRY_DELAY_MS),
    )
}

fn manual_sync_retry_at() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(PARTIAL_TURN_RETRY_DELAY_MS),
    )
}

fn timestamp_to_u64(timestamp: Timestamp) -> u64 {
    u64::try_from(timestamp.as_millis()).unwrap_or(0)
}
