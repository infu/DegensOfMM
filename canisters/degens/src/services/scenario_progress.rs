use std::cell::RefCell;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    GameCommand, GameParticipant, GameSession, ObjectiveProgress, QuestState, ScenarioRuleState,
    SystemJob, WorldEventState, WorldObject,
};
use domm_game::{
    AdvancedScenarioReceipt, ApiError, CommandResponse, CommandResult, OPENING_QUEST_KEY,
    OPENING_QUEST_OBJECTIVE_KEY, OPENING_QUEST_REWARD_GOLD, ObjectiveProgressRecord,
    ObjectiveProgressView, QuestPreview, QuestProgressView, ResourceBalances, ScenarioRuleView,
    ScenarioRulesView, WorldEventView, WorldEventsView,
};
use icydb::types::Timestamp;
use icydb::{traits::EntityValue, types::Id};

#[cfg(not(feature = "benchmark"))]
use crate::repos::commands_events_effects;
use crate::repos::{
    map_visibility_occupancy, scenario_progress, sessions, system_jobs as system_job_repo,
};

use super::system_jobs as system_job_service;
use super::{
    command_response,
    session_context::{self, public_error},
    session_turn_runtime,
};

thread_local! {
    static OBJECTIVE_ROW_CACHE: RefCell<Vec<CachedObjectiveRows>> = const { RefCell::new(Vec::new()) };
    static SCENARIO_RULE_ROW_CACHE: RefCell<Vec<CachedScenarioRuleRows>> = const { RefCell::new(Vec::new()) };
    static WORLD_EVENT_ROW_CACHE: RefCell<Vec<WorldEventState>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
struct CachedObjectiveRows {
    session_id: String,
    complete: bool,
    rows: Vec<ObjectiveProgress>,
}

#[derive(Clone)]
struct CachedScenarioRuleRows {
    session_id: String,
    complete: bool,
    rows: Vec<ScenarioRuleState>,
}

pub(crate) fn get_objective_progress(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<ObjectiveProgressView, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let objectives = objective_rows_for_session(context.session.id())?
        .into_iter()
        .map(objective_view)
        .collect();
    Ok(ObjectiveProgressView {
        session_id: context.session.id().to_string(),
        objectives,
    })
}

pub(crate) fn get_scenario_rules(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<ScenarioRulesView, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let rules = scenario_rule_rows_for_session(&context.session)?
        .into_iter()
        .map(rule_view)
        .collect();
    Ok(ScenarioRulesView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        rules,
    })
}

pub(crate) fn get_world_events(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<WorldEventsView, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let events = world_event_rows_by_status(context.session.id(), "active")?
        .into_iter()
        .map(world_event_view)
        .collect();
    Ok(WorldEventsView {
        session_id: context.session.id().to_string(),
        current_turn: context.session.current_turn,
        events,
    })
}

pub(crate) fn preview_quest(
    caller: CandidPrincipal,
    session_id: String,
    quest_key: String,
) -> Result<QuestPreview, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let quest = load_quest(&context, &quest_key)?;
    Ok(quest_preview(&context, quest))
}

pub(crate) fn accept_quest(
    caller: CandidPrincipal,
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    ensure_known_quest(&quest_key)?;
    let payload_json = format!(
        r#"{{"quest_key":"{}"}}"#,
        command_response::escape_json(&quest_key)
    );
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "accept_quest",
        &client_nonce,
        None,
        payload_json,
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    apply_accept_quest(
        caller,
        &mut context,
        quest_key,
        command,
        runtime_receipt,
        &client_nonce,
    )
}

pub(crate) fn claim_quest_reward(
    caller: CandidPrincipal,
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    ensure_known_quest(&quest_key)?;
    let payload_json = format!(
        r#"{{"quest_key":"{}"}}"#,
        command_response::escape_json(&quest_key)
    );
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "claim_quest_reward",
        &client_nonce,
        None,
        payload_json,
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    apply_claim_quest_reward(
        caller,
        &mut context,
        quest_key,
        command,
        runtime_receipt,
        &client_nonce,
    )
}

pub(crate) fn sync_objectives(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "sync_objectives",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    let objective_summary = sync_objective_rows(&mut context, Some(command.id()))?;
    let session_id_text = context.session.id().to_string();
    session_turn_runtime::with_runtime_mut(
        &session_id_text,
        context.session.current_turn,
        |runtime| {
            runtime.central_objectives_completed = Some(objective_summary.completed);
        },
    );
    let receipt = receipt(
        command.id().to_string(),
        "sync_objectives",
        None,
        None,
        None,
        Some("rule:central-objectives".to_string()),
        context.session.current_turn,
        0,
        "objectives_synced",
        None,
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_runtime_or_fresh_public_event(
        &context,
        command.id(),
        format!(
            "scenario:objectives:{}:{}",
            command.id(),
            objective_summary.touched
        ),
        "objectives_synced".to_string(),
        Some("session".to_string()),
        Some(session_id_text.clone()),
        result_json.clone(),
    )?;
    command_response::apply_runtime_command_with_result(
        caller,
        &context,
        command,
        runtime_receipt,
        &client_nonce,
        result_json,
        vec![event],
        vec![command_response::changed(
            "objective_progress",
            &context.session.id().to_string(),
            "updated",
        )],
        CommandResult::AdvancedScenario(receipt),
    )
}

pub(crate) fn sync_world_events(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "sync_world_events",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    let event_row = ensure_current_world_event(&context.session, Some(command.id()))?;
    let event_key = event_row.event_key.clone();
    let receipt = receipt(
        command.id().to_string(),
        "sync_world_events",
        None,
        None,
        Some(event_key.clone()),
        None,
        context.session.current_turn,
        0,
        "world_events_synced",
        None,
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_runtime_or_fresh_public_event(
        &context,
        command.id(),
        format!("scenario:world_event:{event_key}:{}", command.id()),
        "world_event_synced".to_string(),
        Some("world_event".to_string()),
        Some(event_key.clone()),
        result_json.clone(),
    )?;
    command_response::apply_runtime_command_with_result(
        caller,
        &context,
        command,
        runtime_receipt,
        &client_nonce,
        result_json,
        vec![event],
        vec![command_response::changed(
            "world_event",
            &context.session.id().to_string(),
            "updated",
        )],
        CommandResult::AdvancedScenario(receipt),
    )
}

pub(crate) fn sync_advanced_victory(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "sync_advanced_victory",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    let session_id_text = context.session.id().to_string();
    let completed_objectives = session_turn_runtime::with_runtime(
        &session_id_text,
        context.session.current_turn,
        |runtime| runtime.central_objectives_completed,
    )
    .flatten()
    .map_or_else(
        || sync_objective_rows(&mut context, Some(command.id())).map(|summary| summary.completed),
        Ok,
    )?;
    let updated = sync_scenario_rule_rows_for_session_with_completed_objectives(
        &context.session,
        Some(command.id()),
        Some(completed_objectives),
    )?;
    let receipt = receipt(
        command.id().to_string(),
        "sync_advanced_victory",
        None,
        None,
        None,
        Some("rule:advanced-victory".to_string()),
        context.session.current_turn,
        0,
        "advanced_victory_synced",
        None,
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_runtime_or_fresh_public_event(
        &context,
        command.id(),
        format!("scenario:victory:{}:{updated}", command.id()),
        "advanced_victory_synced".to_string(),
        Some("session".to_string()),
        Some(session_id_text.clone()),
        result_json.clone(),
    )?;
    command_response::apply_runtime_command_with_result(
        caller,
        &context,
        command,
        runtime_receipt,
        &client_nonce,
        result_json,
        vec![event],
        vec![command_response::changed(
            "scenario_rule",
            &context.session.id().to_string(),
            "updated",
        )],
        CommandResult::AdvancedScenario(receipt),
    )
}

pub(crate) fn schedule_turn_maintenance_jobs(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<(), ApiError> {
    schedule_turn_maintenance_jobs_durable(session, command_id)
}

fn schedule_turn_maintenance_jobs_durable(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<(), ApiError> {
    if session.state != "active" {
        return Ok(());
    }

    schedule_scenario_job(session, command_id, "scenario_objectives")?;
    Ok(())
}

fn schedule_scenario_job(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
    job_kind: &str,
) -> Result<(), ApiError> {
    let due_at = Timestamp::now();
    let job_key = format!("{job_kind}:{}:{}", session.id(), session.current_turn);
    let job = system_job_repo::upsert_system_job(system_job_repo::SystemJobDraft {
        job_key,
        job_kind: job_kind.to_string(),
        session_id: session.id(),
        battle_id: None,
        turn_number: Some(session.current_turn),
        due_at,
        command_id,
        cursor_json: None,
    })?;

    if job.status == system_job_repo::STATUS_COMPLETED {
        let mut job = job;
        job.command_id = command_id.map(|id| id.key());
        system_job_repo::reschedule_system_job(job, due_at, None)?;
    }

    system_job_service::schedule_nearest_due_job()
}

pub(crate) fn process_scenario_maintenance_job(job: SystemJob) -> Result<(), ApiError> {
    let fallback = job.clone();
    if let Err(error) = process_scenario_maintenance_job_inner(job) {
        system_job_repo::fail_system_job(fallback, error.retryable, error.message.clone())?;
        return Err(error);
    }
    Ok(())
}

#[cfg(feature = "benchmark")]
fn process_scenario_maintenance_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(
            job,
            false,
            "scenario maintenance session row not found".to_string(),
        )?;
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

    match job.job_kind.as_str() {
        "scenario_objectives" => {
            sync_objective_rows_for_session(session.id(), None)?;
        }
        "world_events" => {
            ensure_current_world_event(&session, None)?;
        }
        "advanced_victory" => {
            let objective_summary = sync_objective_rows_for_session(session.id(), None)?;
            sync_scenario_rule_rows_for_session_with_completed_objectives(
                &session,
                None,
                Some(objective_summary.completed),
            )?;
        }
        _ => {
            system_job_repo::fail_system_job(
                job,
                false,
                "unsupported scenario maintenance job kind".to_string(),
            )?;
            return Ok(());
        }
    }

    let completed_job_kind = job.job_kind.clone();
    system_job_repo::complete_system_job(job)?;
    match completed_job_kind.as_str() {
        "scenario_objectives" => schedule_scenario_job(&session, None, "world_events")?,
        "world_events" => schedule_scenario_job(&session, None, "advanced_victory")?,
        _ => {}
    }
    Ok(())
}

#[cfg(not(feature = "benchmark"))]
fn process_scenario_maintenance_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let mut job = job;
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(
            job,
            false,
            "scenario maintenance session row not found".to_string(),
        )?;
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

    let mut command = ensure_system_scenario_command(&session, &job)?;
    job.command_id = Some(command.id().key());
    command.status = "applying".to_string();
    command.phase = "applying".to_string();
    command = commands_events_effects::update_game_command(command)?;

    let touched = match job.job_kind.as_str() {
        "scenario_objectives" => {
            sync_objective_rows_for_session(session.id(), Some(command.id()))?.touched
        }
        "world_events" => {
            ensure_current_world_event(&session, Some(command.id()))?;
            1
        }
        "advanced_victory" => {
            let objective_summary =
                sync_objective_rows_for_session(session.id(), Some(command.id()))?;
            sync_scenario_rule_rows_for_session_with_completed_objectives(
                &session,
                Some(command.id()),
                Some(objective_summary.completed),
            )?
        }
        _ => {
            system_job_repo::fail_system_job(
                job,
                false,
                "unsupported scenario maintenance job kind".to_string(),
            )?;
            return Ok(());
        }
    };

    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(format!(
        r#"{{"command_kind":"{}","current_turn":{},"touched":{},"command_count":1,"event_count":0}}"#,
        command_response::escape_json(&job.job_kind),
        session.current_turn,
        touched
    ));
    command.retryable = false;
    command.applied_at = Some(Timestamp::now());
    command.failed_at = None;
    commands_events_effects::update_game_command(command)?;
    let completed_job_kind = job.job_kind.clone();
    system_job_repo::complete_system_job(job)?;
    match completed_job_kind.as_str() {
        "scenario_objectives" => schedule_scenario_job(&session, None, "world_events")?,
        "world_events" => schedule_scenario_job(&session, None, "advanced_victory")?,
        _ => {}
    }
    Ok(())
}

#[cfg(not(feature = "benchmark"))]
fn ensure_system_scenario_command(
    session: &GameSession,
    job: &SystemJob,
) -> Result<GameCommand, ApiError> {
    let command_type = job.job_kind.as_str();
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

fn apply_accept_quest(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    quest_key: String,
    command: GameCommand,
    runtime_receipt: bool,
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let mut quest = load_quest(context, &quest_key)?;
    let transition = domm_game::quest_accept_transition(&quest.status);
    if !transition.allowed {
        return command_response::fail_runtime_command(
            caller,
            context,
            command,
            runtime_receipt,
            client_nonce,
            public_error(
                transition
                    .disabled_reason
                    .unwrap_or_else(|| "quest_disabled".to_string()),
                "quest cannot be accepted",
                false,
            ),
        );
    }
    quest.status = transition.next_status;
    quest.accepted_turn = context.session.current_turn;
    quest.accepted_command_id = Some(command.id().key());
    quest.last_command_id = Some(command.id().key());
    let quest = persist_or_mirror_active_quest(context, quest)?;
    let receipt = receipt(
        command.id().to_string(),
        "accept_quest",
        Some(quest.quest_key.clone()),
        Some(quest.objective_key.clone()),
        None,
        None,
        context.session.current_turn,
        0,
        quest.status.clone(),
        None,
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_runtime_or_fresh_public_event(
        context,
        command.id(),
        format!("scenario:quest_accept:{}:{}", quest.quest_key, command.id()),
        "quest_accepted".to_string(),
        Some("quest".to_string()),
        Some(quest.quest_key.clone()),
        result_json.clone(),
    )?;
    command_response::apply_runtime_command_with_result(
        caller,
        context,
        command,
        runtime_receipt,
        client_nonce,
        result_json,
        vec![event],
        vec![command_response::changed(
            "quest",
            &quest.quest_key,
            "updated",
        )],
        CommandResult::AdvancedScenario(receipt),
    )
}

fn apply_claim_quest_reward(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    quest_key: String,
    command: GameCommand,
    runtime_receipt: bool,
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let mut quest = load_quest(context, &quest_key)?;
    let transition = domm_game::quest_claim_transition(
        &quest.status,
        quest.progress_value,
        quest.required_value,
        quest.reward_gold,
    );
    if !transition.allowed {
        return command_response::fail_runtime_command(
            caller,
            context,
            command,
            runtime_receipt,
            client_nonce,
            public_error(
                transition
                    .disabled_reason
                    .unwrap_or_else(|| "quest_disabled".to_string()),
                "quest reward cannot be claimed",
                false,
            ),
        );
    }
    apply_gold_reward(
        context.session.id(),
        &mut context.participant,
        command.id(),
        &format!("quest_reward:{quest_key}:gold"),
        context.session.current_turn,
        u64::from(transition.reward_gold_delta),
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    context.participant.last_resource_command_id = Some(command.id().key());
    context.participant =
        persist_or_mirror_active_participant(&context.session, context.participant.clone())?;
    quest.status = transition.next_status;
    quest.claimed_turn = context.session.current_turn;
    quest.claimed_command_id = Some(command.id().key());
    quest.last_command_id = Some(command.id().key());
    let quest = persist_or_mirror_active_quest(context, quest)?;
    sync_quest_victory_rule_after_claim(&context.session, command.id())?;
    let resources_after = balances_from_participant(&context.participant);
    let receipt = receipt(
        command.id().to_string(),
        "claim_quest_reward",
        Some(quest.quest_key.clone()),
        Some(quest.objective_key.clone()),
        None,
        Some("rule:quest-victory".to_string()),
        context.session.current_turn,
        transition.reward_gold_delta,
        quest.status.clone(),
        Some(resources_after),
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_runtime_or_fresh_public_event(
        context,
        command.id(),
        format!("scenario:quest_claim:{}:{}", quest.quest_key, command.id()),
        "quest_reward_claimed".to_string(),
        Some("quest".to_string()),
        Some(quest.quest_key.clone()),
        result_json.clone(),
    )?;
    command_response::apply_runtime_command_with_result(
        caller,
        context,
        command,
        runtime_receipt,
        client_nonce,
        result_json,
        vec![event],
        vec![
            command_response::changed("quest", &quest.quest_key, "updated"),
            command_response::changed(
                "participant",
                &context.participant.id().to_string(),
                "updated",
            ),
        ],
        CommandResult::AdvancedScenario(receipt),
    )
}

pub(crate) fn ensure_seeded_scenario_progress(
    session: &GameSession,
    participants: &[GameParticipant],
) -> Result<(), ApiError> {
    ensure_initial_objectives(session)?;
    ensure_current_world_event(session, None)?;
    ensure_initial_rules(session)?;
    for participant in participants {
        ensure_opening_quest(session.id(), participant.id())?;
    }
    Ok(())
}

fn ensure_initial_objectives(session: &GameSession) -> Result<(), ApiError> {
    for seed in &domm_game::first_playable_scenario().central_objectives {
        let Some(object) = map_visibility_occupancy::find_world_object_by_session_xy(
            session.id(),
            seed.x,
            seed.y,
        )?
        else {
            continue;
        };
        ensure_objective_row_for_object(session.id(), &object, &seed.key, None)?;
    }
    mark_objective_rows_complete(session.id());
    Ok(())
}

fn ensure_initial_rules(session: &GameSession) -> Result<(), ApiError> {
    for (rule_key, rule_type, status, victory_state, required, disabled) in [
        ("rule:conquest", "conquest", "active", "active", 1, None),
        (
            "rule:central-objectives",
            "central_objectives",
            "active",
            "active",
            2,
            None,
        ),
        (
            "rule:quest-victory",
            "quest_victory",
            "active",
            "active",
            3,
            None,
        ),
        (
            "rule:max-turn",
            "max_turn",
            "active",
            "active",
            session.max_turns,
            None,
        ),
        (
            "rule:artifact-victory",
            "artifact_victory",
            "disabled",
            "disabled",
            1,
            Some("checkpoint_24_schema_only"),
        ),
        (
            "rule:king-of-the-hill",
            "king_of_the_hill",
            "disabled",
            "disabled",
            1,
            Some("checkpoint_24_schema_only"),
        ),
        (
            "rule:survival",
            "survival",
            "disabled",
            "disabled",
            1,
            Some("checkpoint_24_schema_only"),
        ),
        (
            "rule:scenario-defeat",
            "scenario_specific_defeat",
            "disabled",
            "disabled",
            1,
            Some("checkpoint_24_schema_only"),
        ),
    ] {
        let row = if let Some(row) =
            scenario_progress::find_scenario_rule_by_key(session.id(), rule_key)?
        {
            row
        } else {
            scenario_progress::create_scenario_rule_state(
                session.id(),
                rule_key.to_string(),
                rule_type.to_string(),
                status.to_string(),
                victory_state.to_string(),
                required,
                0,
                None,
                None,
                disabled.map(str::to_string),
                session.current_turn,
            )?
        };
        remember_scenario_rule_row(session.id(), row);
    }
    mark_scenario_rule_rows_complete(session.id());
    sync_scenario_rule_rows_for_session_with_completed_objectives(session, None, None).map(|_| ())
}

fn ensure_opening_quest(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
) -> Result<(), ApiError> {
    if scenario_progress::find_quest_by_participant_key(
        session_id,
        participant_id,
        OPENING_QUEST_KEY,
    )?
    .is_none()
    {
        scenario_progress::create_quest_state(
            session_id,
            participant_id,
            OPENING_QUEST_KEY.to_string(),
            domm_game::OPENING_QUEST_TITLE.to_string(),
            OPENING_QUEST_OBJECTIVE_KEY.to_string(),
            "available".to_string(),
            1,
            1,
            OPENING_QUEST_REWARD_GOLD,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct ObjectiveSyncSummary {
    touched: u32,
    completed: u32,
}

fn sync_objective_rows(
    context: &session_context::SessionCallerContext,
    command_id: Option<Id<GameCommand>>,
) -> Result<ObjectiveSyncSummary, ApiError> {
    let session_id_text = context.session.id().to_string();
    let skip_rows = session_turn_runtime::runtime_object_deltas_empty(
        &session_id_text,
        context.session.current_turn,
    );
    let command_id = if skip_rows { None } else { command_id };
    sync_objective_rows_for_session(context.session.id(), command_id)
}

fn sync_objective_rows_for_session(
    session_id: Id<GameSession>,
    command_id: Option<Id<GameCommand>>,
) -> Result<ObjectiveSyncSummary, ApiError> {
    let mut summary = ObjectiveSyncSummary::default();
    for seed in &domm_game::first_playable_scenario().central_objectives {
        let Some(object) = central_objective_world_object(session_id, seed.x, seed.y)? else {
            continue;
        };
        if object.owner_participant_id.is_some() {
            summary.completed = summary.completed.saturating_add(1);
        }
        if command_id.is_some() {
            ensure_objective_row_for_object(session_id, &object, &seed.key, command_id)?;
        }
        summary.touched = summary.touched.saturating_add(1);
    }
    Ok(summary)
}

fn central_objective_world_object(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
) -> Result<Option<WorldObject>, ApiError> {
    let session_id_text = session_id.to_string();
    if let Some(Some(object)) = session_turn_runtime::world_object_at(&session_id_text, x, y)
        && object.session_id == session_id.key()
    {
        return Ok(Some(object));
    }
    map_visibility_occupancy::find_world_object_by_session_xy(session_id, x, y)
}

fn ensure_objective_row_for_object(
    session_id: Id<GameSession>,
    object: &WorldObject,
    objective_key: &str,
    command_id: Option<Id<GameCommand>>,
) -> Result<ObjectiveProgress, ApiError> {
    let owner = object
        .owner_participant_id
        .map(Id::<GameParticipant>::from_key);
    let progress_value = u32::from(owner.is_some());
    let status = domm_game::objective_status(progress_value, 1).to_string();
    let row = match scenario_progress::find_objective_by_key(session_id, objective_key)? {
        Some(mut row) => {
            let previous_participant_id = row.participant_id;
            let previous_object_id = row.object_id;
            let previous_progress_value = row.progress_value;
            let previous_status = row.status.clone();
            let previous_last_scored_turn = row.last_scored_turn;
            row.participant_id = owner.map(|id| id.key());
            row.object_id = Some(object.id().key());
            row.progress_value = progress_value;
            row.status = status;
            row.last_scored_turn = object.captured_turn;
            if let Some(command_id) = command_id {
                row.last_command_id = Some(command_id.key());
            }
            if row.participant_id != previous_participant_id
                || row.object_id != previous_object_id
                || row.progress_value != previous_progress_value
                || row.status != previous_status
                || row.last_scored_turn != previous_last_scored_turn
            {
                scenario_progress::update_objective_progress(row)
            } else {
                Ok(row)
            }
        }
        None => scenario_progress::create_objective_progress(
            session_id,
            owner,
            Some(object.id()),
            objective_key.to_string(),
            "central_objective".to_string(),
            progress_value,
            1,
            status,
            "public".to_string(),
            object.captured_turn,
        ),
    }?;
    remember_objective_row(session_id, row.clone());
    Ok(row)
}

fn ensure_current_world_event(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<WorldEventState, ApiError> {
    let event = domm_game::deterministic_world_event(session.seed, session.current_turn);
    if let Some(row) = cached_world_event_by_key(session.id(), &event.event_key) {
        return Ok(row);
    }
    match scenario_progress::find_world_event_by_key(session.id(), &event.event_key)? {
        Some(row) => {
            remember_world_event_row(session.id(), row.clone());
            Ok(row)
        }
        None => {
            let mut row = scenario_progress::create_world_event_state(
                session.id(),
                event.event_key,
                event.event_type,
                event.event_window,
                event.starts_turn,
                event.ends_turn,
                event.status,
                event.payload.unwrap_or_else(|| "{}".to_string()),
            )?;
            if let Some(command_id) = command_id {
                row.last_command_id = Some(command_id.key());
                row = scenario_progress::update_world_event_state(row)?;
            }
            remember_world_event_row(session.id(), row.clone());
            Ok(row)
        }
    }
}

fn sync_quest_victory_rule_after_claim(
    session: &GameSession,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    let Some(mut rule) = load_scenario_rule(session, "rule:quest-victory")? else {
        return Ok(());
    };
    if rule.status != "active" {
        return Ok(());
    }
    let previous_current_value = rule.current_value;
    let previous_victory_state = rule.victory_state.clone();
    rule.current_value = rule.current_value.saturating_add(1);
    rule.victory_state = if rule.current_value >= rule.required_value {
        "complete".to_string()
    } else {
        "active".to_string()
    };
    rule.last_checked_turn = session.current_turn;
    rule.last_command_id = Some(command_id.key());
    if rule.current_value != previous_current_value || rule.victory_state != previous_victory_state
    {
        persist_or_mirror_active_scenario_rule(session, rule)?;
    }
    Ok(())
}

fn sync_scenario_rule_rows_for_session_with_completed_objectives(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
    completed_objectives: Option<u32>,
) -> Result<u32, ApiError> {
    let completed_objectives = match completed_objectives {
        Some(completed) => completed,
        None => {
            let objectives = objective_rows_for_session(session.id())?;
            objectives
                .iter()
                .filter(|row| row.status == "complete")
                .count() as u32
        }
    };
    let mut rules = if let Some(rows) = cached_scenario_rule_rows(session.id()) {
        rows
    } else {
        scenario_progress::page_scenario_rules_by_status(session.id(), "active")?.items
    };
    merge_runtime_scenario_rules(&mut rules, session);
    let mut touched = 0_u32;
    for mut rule in rules.into_iter().filter(|rule| rule.status == "active") {
        let previous_current_value = rule.current_value;
        let previous_victory_state = rule.victory_state.clone();
        let previous_winner_participant_id = rule.winner_participant_id;
        match rule.rule_key.as_str() {
            "rule:conquest" => {
                rule.current_value = u32::from(session.winner_participant_id.is_some());
                rule.winner_participant_id = session.winner_participant_id;
                rule.victory_state = if session.winner_participant_id.is_some() {
                    "complete".to_string()
                } else {
                    "active".to_string()
                };
            }
            "rule:central-objectives" => {
                rule.current_value = completed_objectives;
                rule.victory_state = if completed_objectives >= rule.required_value {
                    "complete".to_string()
                } else {
                    "active".to_string()
                };
            }
            "rule:quest-victory" => {
                let claimed = if session_turn_runtime::contains_runtime(
                    &session.id().to_string(),
                    session.current_turn,
                ) && rule.last_checked_turn == session.current_turn
                {
                    rule.current_value
                } else {
                    claimed_quest_count(session.id())?
                };
                rule.current_value = claimed;
                rule.victory_state = if claimed >= rule.required_value {
                    "complete".to_string()
                } else {
                    "active".to_string()
                };
            }
            "rule:max-turn" => {
                rule.current_value = session.current_turn;
                rule.victory_state =
                    domm_game::max_turn_rule_state(session.current_turn, session.max_turns)
                        .to_string();
            }
            _ => {}
        }
        rule.last_checked_turn = session.current_turn;
        if let Some(command_id) = command_id {
            rule.last_command_id = Some(command_id.key());
        }
        if rule.current_value != previous_current_value
            || rule.victory_state != previous_victory_state
            || rule.winner_participant_id != previous_winner_participant_id
        {
            persist_or_mirror_active_scenario_rule(session, rule)?;
        }
        touched = touched.saturating_add(1);
        if touched >= domm_game::MAX_ADVANCED_VICTORY_CHECKS_PER_UPDATE {
            break;
        }
    }
    Ok(touched)
}

fn objective_rows_for_session(
    session_id: Id<GameSession>,
) -> Result<Vec<ObjectiveProgress>, ApiError> {
    let mut rows = if let Some(rows) = cached_objective_rows(session_id) {
        rows
    } else {
        let mut rows = scenario_progress::page_objectives_by_status(session_id, "active")?.items;
        rows.extend(scenario_progress::page_objectives_by_status(session_id, "complete")?.items);
        rows
    };
    rows.sort_by(|left, right| left.objective_key.cmp(&right.objective_key));
    Ok(rows)
}

fn scenario_rule_rows_for_session(
    session: &GameSession,
) -> Result<Vec<ScenarioRuleState>, ApiError> {
    let mut rows = if let Some(rows) = cached_scenario_rule_rows(session.id()) {
        rows
    } else {
        let mut rows =
            scenario_progress::page_scenario_rules_by_status(session.id(), "active")?.items;
        rows.extend(
            scenario_progress::page_scenario_rules_by_status(session.id(), "disabled")?.items,
        );
        rows
    };
    merge_runtime_scenario_rules(&mut rows, session);
    rows.sort_by(|left, right| left.rule_key.cmp(&right.rule_key));
    Ok(rows)
}

fn merge_runtime_scenario_rules(rows: &mut Vec<ScenarioRuleState>, session: &GameSession) {
    for snapshot in session_turn_runtime::scenario_rule_snapshots(
        &session.id().to_string(),
        session.current_turn,
    ) {
        upsert_scenario_rule_row(rows, snapshot);
    }
}

fn load_scenario_rule(
    session: &GameSession,
    rule_key: &str,
) -> Result<Option<ScenarioRuleState>, ApiError> {
    if let Some(rule) = session_turn_runtime::scenario_rule_snapshot(
        &session.id().to_string(),
        session.current_turn,
        rule_key,
    ) {
        return Ok(Some(rule));
    }
    if let Some(rule) = cached_scenario_rule_rows(session.id())
        .and_then(|rows| rows.into_iter().find(|row| row.rule_key == rule_key))
    {
        return Ok(Some(rule));
    }
    scenario_progress::find_scenario_rule_by_key(session.id(), rule_key)
}

fn claimed_quest_count(session_id: Id<GameSession>) -> Result<u32, ApiError> {
    Ok(
        scenario_progress::page_quests_by_session_key(session_id, OPENING_QUEST_KEY)?
            .items
            .into_iter()
            .filter(|quest| quest.status == "claimed")
            .count() as u32,
    )
}

fn load_quest(
    context: &session_context::SessionCallerContext,
    quest_key: &str,
) -> Result<QuestState, ApiError> {
    ensure_known_quest(quest_key)?;
    if let Some(quest) = session_turn_runtime::quest_snapshot(
        &context.session.id().to_string(),
        context.session.current_turn,
        &context.participant.id().to_string(),
        quest_key,
    ) {
        return Ok(quest);
    }
    scenario_progress::find_quest_by_participant_key(
        context.session.id(),
        context.participant.id(),
        quest_key,
    )?
    .ok_or_else(|| public_error("quest_not_found", "quest was not found", false))
}

fn persist_or_mirror_active_quest(
    context: &session_context::SessionCallerContext,
    quest: QuestState,
) -> Result<QuestState, ApiError> {
    if session_turn_runtime::mirror_quest_snapshot(
        &context.session.id().to_string(),
        context.session.current_turn,
        quest.clone(),
    ) {
        Ok(quest)
    } else {
        scenario_progress::update_quest_state(quest)
    }
}

fn persist_or_mirror_active_scenario_rule(
    session: &GameSession,
    rule: ScenarioRuleState,
) -> Result<ScenarioRuleState, ApiError> {
    if session_turn_runtime::mirror_scenario_rule_snapshot(
        &session.id().to_string(),
        session.current_turn,
        rule.clone(),
    ) {
        remember_scenario_rule_row(session.id(), rule.clone());
        Ok(rule)
    } else {
        let rule = scenario_progress::update_scenario_rule_state(rule)?;
        remember_scenario_rule_row(session.id(), rule.clone());
        Ok(rule)
    }
}

fn cached_objective_rows(session_id: Id<GameSession>) -> Option<Vec<ObjectiveProgress>> {
    let key = session_id.to_string();
    OBJECTIVE_ROW_CACHE.with_borrow(|cache| {
        cache
            .iter()
            .find(|entry| entry.session_id == key && entry.complete)
            .map(|entry| entry.rows.clone())
    })
}

fn remember_objective_row(session_id: Id<GameSession>, row: ObjectiveProgress) {
    let key = session_id.to_string();
    OBJECTIVE_ROW_CACHE.with_borrow_mut(|cache| {
        let entry = objective_cache_entry_mut(cache, &key);
        upsert_objective_row(&mut entry.rows, row);
    });
}

fn mark_objective_rows_complete(session_id: Id<GameSession>) {
    let key = session_id.to_string();
    OBJECTIVE_ROW_CACHE.with_borrow_mut(|cache| {
        if let Some(entry) = cache.iter_mut().find(|entry| entry.session_id == key) {
            entry.complete = true;
        }
    });
}

fn cached_scenario_rule_rows(session_id: Id<GameSession>) -> Option<Vec<ScenarioRuleState>> {
    let key = session_id.to_string();
    SCENARIO_RULE_ROW_CACHE.with_borrow(|cache| {
        cache
            .iter()
            .find(|entry| entry.session_id == key && entry.complete)
            .map(|entry| entry.rows.clone())
    })
}

fn world_event_rows_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> Result<Vec<WorldEventState>, ApiError> {
    if let Some(rows) = cached_world_event_rows_by_status(session_id, status) {
        return Ok(rows);
    }
    let rows = scenario_progress::page_world_events_by_status(session_id, status)?.items;
    for row in &rows {
        remember_world_event_row(session_id, row.clone());
    }
    Ok(rows)
}

fn cached_world_event_rows_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> Option<Vec<WorldEventState>> {
    let key = session_id.key();
    WORLD_EVENT_ROW_CACHE.with_borrow(|cache| {
        let rows: Vec<_> = cache
            .iter()
            .filter(|row| row.session_id == key && row.status == status)
            .cloned()
            .collect();
        (!rows.is_empty()).then_some(rows)
    })
}

fn cached_world_event_by_key(
    session_id: Id<GameSession>,
    event_key: &str,
) -> Option<WorldEventState> {
    let key = session_id.key();
    WORLD_EVENT_ROW_CACHE.with_borrow(|cache| {
        cache
            .iter()
            .find(|row| row.session_id == key && row.event_key == event_key)
            .cloned()
    })
}

fn remember_world_event_row(session_id: Id<GameSession>, row: WorldEventState) {
    let key = session_id.key();
    WORLD_EVENT_ROW_CACHE.with_borrow_mut(|cache| {
        cache.retain(|cached| cached.session_id != key || cached.id() != row.id());
        cache.push(row);
    });
}

fn remember_scenario_rule_row(session_id: Id<GameSession>, row: ScenarioRuleState) {
    let key = session_id.to_string();
    SCENARIO_RULE_ROW_CACHE.with_borrow_mut(|cache| {
        let entry = scenario_rule_cache_entry_mut(cache, &key);
        upsert_scenario_rule_row(&mut entry.rows, row);
    });
}

fn mark_scenario_rule_rows_complete(session_id: Id<GameSession>) {
    let key = session_id.to_string();
    SCENARIO_RULE_ROW_CACHE.with_borrow_mut(|cache| {
        if let Some(entry) = cache.iter_mut().find(|entry| entry.session_id == key) {
            entry.complete = true;
        }
    });
}

fn objective_cache_entry_mut<'a>(
    cache: &'a mut Vec<CachedObjectiveRows>,
    key: &str,
) -> &'a mut CachedObjectiveRows {
    if let Some(index) = cache.iter().position(|entry| entry.session_id == key) {
        return &mut cache[index];
    }
    cache.push(CachedObjectiveRows {
        session_id: key.to_string(),
        complete: false,
        rows: Vec::new(),
    });
    cache.last_mut().expect("cache entry was just pushed")
}

fn scenario_rule_cache_entry_mut<'a>(
    cache: &'a mut Vec<CachedScenarioRuleRows>,
    key: &str,
) -> &'a mut CachedScenarioRuleRows {
    if let Some(index) = cache.iter().position(|entry| entry.session_id == key) {
        return &mut cache[index];
    }
    cache.push(CachedScenarioRuleRows {
        session_id: key.to_string(),
        complete: false,
        rows: Vec::new(),
    });
    cache.last_mut().expect("cache entry was just pushed")
}

fn upsert_objective_row(rows: &mut Vec<ObjectiveProgress>, row: ObjectiveProgress) {
    if let Some(existing) = rows.iter_mut().find(|existing| existing.id() == row.id()) {
        *existing = row;
    } else {
        rows.push(row);
    }
}

fn upsert_scenario_rule_row(rows: &mut Vec<ScenarioRuleState>, row: ScenarioRuleState) {
    if let Some(existing) = rows.iter_mut().find(|existing| existing.id() == row.id()) {
        *existing = row;
    } else {
        rows.push(row);
    }
}

fn ensure_known_quest(quest_key: &str) -> Result<(), ApiError> {
    if quest_key == OPENING_QUEST_KEY {
        Ok(())
    } else {
        Err(public_error(
            "quest_not_found",
            "quest was not found",
            false,
        ))
    }
}

fn apply_gold_reward(
    session_id: Id<GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<GameCommand>,
    ledger_key: &str,
    turn_number: u32,
    amount: u64,
) -> Result<(), ApiError> {
    #[cfg(feature = "benchmark")]
    let _ = command_id;

    let balance_after = participant.gold.saturating_add(amount);
    let delta = i64::try_from(amount).unwrap_or(i64::MAX);
    participant.gold = balance_after;
    #[cfg(feature = "benchmark")]
    {
        let _ = ledger_key;
        session_turn_runtime::record_resource_delta(
            &session_id.to_string(),
            turn_number,
            &participant.id().to_string(),
            "gold",
            delta,
        );
    }
    #[cfg(not(feature = "benchmark"))]
    {
        session_turn_runtime::record_resource_ledger_delta(
            &session_id.to_string(),
            turn_number,
            &participant.id().to_string(),
            &command_id.to_string(),
            ledger_key.to_string(),
            "gold",
            delta,
            balance_after,
            "quest_reward",
        );
    }
    Ok(())
}

fn persist_or_mirror_active_participant(
    session: &GameSession,
    participant: GameParticipant,
) -> Result<GameParticipant, ApiError> {
    if session_turn_runtime::contains_runtime(&session.id().to_string(), session.current_turn) {
        session_turn_runtime::mirror_participant_update(&participant);
        Ok(participant)
    } else {
        Ok(sessions::update_participant(participant)?)
    }
}

fn quest_preview(
    context: &session_context::SessionCallerContext,
    quest: QuestState,
) -> QuestPreview {
    let accept = domm_game::quest_accept_transition(&quest.status);
    let claim = domm_game::quest_claim_transition(
        &quest.status,
        quest.progress_value,
        quest.required_value,
        quest.reward_gold,
    );
    let disabled_reason = if accept.allowed || claim.allowed {
        None
    } else {
        claim.disabled_reason.or(accept.disabled_reason)
    };
    QuestPreview {
        can_accept: accept.allowed,
        can_claim: claim.allowed,
        disabled_reason,
        quest: quest_view(quest, &context.participant.id().to_string()),
    }
}

fn quest_view(quest: QuestState, viewer_participant_id: &str) -> QuestProgressView {
    let view = QuestProgressView {
        quest_key: quest.quest_key,
        title: quest.title,
        participant_id: Id::<GameParticipant>::from_key(quest.participant_id).to_string(),
        objective_key: quest.objective_key,
        status: quest.status.clone(),
        progress_value: quest.progress_value,
        required_value: quest.required_value,
        reward_gold: Some(quest.reward_gold),
        reward_claimed: quest.status == "claimed",
        accepted_turn: quest.accepted_turn,
        claimed_turn: quest.claimed_turn,
        redacted: false,
    };
    domm_game::redact_quest_for_viewer(view, viewer_participant_id)
}

fn objective_view(row: ObjectiveProgress) -> ObjectiveProgressRecord {
    ObjectiveProgressRecord {
        objective_key: row.objective_key,
        objective_type: row.objective_type,
        owner_participant_id: row
            .participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
        object_id: row
            .object_id
            .map(|id| Id::<WorldObject>::from_key(id).to_string()),
        progress_value: row.progress_value,
        required_value: row.required_value,
        status: row.status,
        visible_to: row.visible_to,
        last_scored_turn: row.last_scored_turn,
        redacted: false,
    }
}

fn world_event_view(row: WorldEventState) -> WorldEventView {
    WorldEventView {
        event_key: row.event_key,
        event_type: row.event_type,
        event_window: row.event_window,
        starts_turn: row.starts_turn,
        ends_turn: row.ends_turn,
        status: row.status,
        payload: Some(row.payload_json),
        redacted: false,
    }
}

fn rule_view(row: ScenarioRuleState) -> ScenarioRuleView {
    ScenarioRuleView {
        rule_key: row.rule_key,
        rule_type: row.rule_type,
        status: row.status,
        victory_state: row.victory_state,
        required_value: row.required_value,
        current_value: row.current_value,
        owner_participant_id: row
            .owner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
        winner_participant_id: row
            .winner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
        disabled_reason: row.disabled_reason,
        last_checked_turn: row.last_checked_turn,
    }
}

fn balances_from_participant(participant: &GameParticipant) -> ResourceBalances {
    ResourceBalances {
        gold: participant.gold,
        wood: participant.wood,
        stone: participant.stone,
        iron: participant.iron,
        crystal: participant.crystal,
        ember: participant.ember,
        aether: participant.aether,
    }
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    command_id: String,
    action: &str,
    quest_key: Option<String>,
    objective_key: Option<String>,
    event_key: Option<String>,
    rule_key: Option<String>,
    current_turn: u32,
    reward_gold: u32,
    state: impl Into<String>,
    resources_after: Option<ResourceBalances>,
) -> AdvancedScenarioReceipt {
    AdvancedScenarioReceipt {
        command_id,
        action: action.to_string(),
        quest_key,
        objective_key,
        event_key,
        rule_key,
        current_turn,
        reward_gold,
        state: state.into(),
        resources_after,
    }
}

fn receipt_json(receipt: &AdvancedScenarioReceipt) -> String {
    let resources = receipt
        .resources_after
        .as_ref()
        .map(|resources| {
            format!(
                "{{\"gold\":{},\"wood\":{},\"stone\":{},\"iron\":{},\"crystal\":{},\"ember\":{},\"aether\":{}}}",
                resources.gold,
                resources.wood,
                resources.stone,
                resources.iron,
                resources.crystal,
                resources.ember,
                resources.aether
            )
        })
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"action\":\"{}\",\"quest_key\":{},\"objective_key\":{},\"event_key\":{},\"rule_key\":{},\"current_turn\":{},\"reward_gold\":{},\"state\":\"{}\",\"resources_after\":{}}}",
        command_response::escape_json(&receipt.action),
        option_json(receipt.quest_key.as_deref()),
        option_json(receipt.objective_key.as_deref()),
        option_json(receipt.event_key.as_deref()),
        option_json(receipt.rule_key.as_deref()),
        receipt.current_turn,
        receipt.reward_gold,
        command_response::escape_json(&receipt.state),
        resources
    )
}

fn option_json(value: Option<&str>) -> String {
    value
        .map(|value| format!(r#""{}""#, command_response::escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}
