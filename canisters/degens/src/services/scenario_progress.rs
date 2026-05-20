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
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::{
    commands_events_effects, economy, map_visibility_occupancy, scenario_progress, sessions,
    system_jobs as system_job_repo,
};

use super::{
    command_response::{self, GameCommandAction},
    session_context::{self, public_error},
    system_jobs as system_job_service,
};

pub(crate) fn get_objective_progress(
    caller: CandidPrincipal,
    session_id: String,
) -> Result<ObjectiveProgressView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
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
    let context = session_context::require_session_caller(caller, &session_id)?;
    let rules = scenario_rule_rows_for_session(context.session.id())?
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
    let context = session_context::require_session_caller(caller, &session_id)?;
    let events = scenario_progress::page_world_events_by_status(context.session.id(), "active")?
        .items
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
    let context = session_context::require_session_caller(caller, &session_id)?;
    let quest = load_quest(&context, &quest_key)?;
    Ok(quest_preview(&context, quest))
}

pub(crate) fn accept_quest(
    caller: CandidPrincipal,
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    ensure_known_quest(&quest_key)?;
    let payload_json = format!(
        r#"{{"quest_key":"{}"}}"#,
        command_response::escape_json(&quest_key)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "accept_quest",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    apply_accept_quest(caller, &mut context, quest_key, command, &client_nonce)
}

pub(crate) fn claim_quest_reward(
    caller: CandidPrincipal,
    session_id: String,
    quest_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    ensure_known_quest(&quest_key)?;
    let payload_json = format!(
        r#"{{"quest_key":"{}"}}"#,
        command_response::escape_json(&quest_key)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "claim_quest_reward",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    apply_claim_quest_reward(caller, &mut context, quest_key, command, &client_nonce)
}

pub(crate) fn sync_objectives(
    caller: CandidPrincipal,
    session_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "sync_objectives",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    let touched = sync_objective_rows(&mut context, Some(command.id()))?;
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
    let session_id_text = context.session.id().to_string();
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("scenario:objectives:{}:{touched}", command.id()),
        "objectives_synced".to_string(),
        Some("session".to_string()),
        Some(session_id_text.clone()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        "scenario:sync_objectives".to_string(),
        "objective_sync".to_string(),
        "session".to_string(),
        session_id_text,
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
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
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "sync_world_events",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
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
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("scenario:world_event:{event_key}:{}", command.id()),
        "world_event_synced".to_string(),
        Some("world_event".to_string()),
        Some(event_key.clone()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("scenario:world_event:{event_key}"),
        "world_event_sync".to_string(),
        "world_event".to_string(),
        event_key,
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
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
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "sync_advanced_victory",
        &client_nonce,
        None,
        "{}".to_string(),
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    sync_objective_rows(&mut context, Some(command.id()))?;
    let updated = sync_scenario_rule_rows(&context, Some(command.id()))?;
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
    let session_id_text = context.session.id().to_string();
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("scenario:victory:{}:{updated}", command.id()),
        "advanced_victory_synced".to_string(),
        Some("session".to_string()),
        Some(session_id_text.clone()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        "scenario:sync_advanced_victory".to_string(),
        "advanced_victory_sync".to_string(),
        "session".to_string(),
        session_id_text,
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
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
        "scenario_objectives" => sync_objective_rows_for_session(session.id(), Some(command.id()))?,
        "world_events" => {
            ensure_current_world_event(&session, Some(command.id()))?;
            1
        }
        "advanced_victory" => {
            sync_objective_rows_for_session(session.id(), Some(command.id()))?;
            sync_scenario_rule_rows_for_session(&session, Some(command.id()))?
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
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let mut quest = load_quest(context, &quest_key)?;
    let transition = domm_game::quest_accept_transition(&quest.status);
    if !transition.allowed {
        return command_response::fail_command(
            caller,
            context,
            command,
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
    let quest = scenario_progress::update_quest_state(quest)?;
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
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("scenario:quest_accept:{}:{}", quest.quest_key, command.id()),
        "quest_accepted".to_string(),
        Some("quest".to_string()),
        Some(quest.quest_key.clone()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("scenario:quest_accept:{}", quest.quest_key),
        "quest_accept".to_string(),
        "quest".to_string(),
        quest.quest_key.clone(),
        result_json.clone(),
    )?;
    schedule_turn_maintenance_jobs(&context.session, Some(command.id()))?;
    command_response::apply_command_with_result(
        caller,
        context,
        command,
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
        return command_response::fail_command(
            caller,
            context,
            command,
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
    context.participant = sessions::update_participant(context.participant.clone())?;
    quest.status = transition.next_status;
    quest.claimed_turn = context.session.current_turn;
    quest.claimed_command_id = Some(command.id().key());
    quest.last_command_id = Some(command.id().key());
    let quest = scenario_progress::update_quest_state(quest)?;
    sync_scenario_rule_rows(context, Some(command.id()))?;
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
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("scenario:quest_claim:{}:{}", quest.quest_key, command.id()),
        "quest_reward_claimed".to_string(),
        Some("quest".to_string()),
        Some(quest.quest_key.clone()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("scenario:quest_claim:{}", quest.quest_key),
        "quest_reward".to_string(),
        "quest".to_string(),
        quest.quest_key.clone(),
        result_json.clone(),
    )?;
    schedule_turn_maintenance_jobs(&context.session, Some(command.id()))?;
    command_response::apply_command_with_result(
        caller,
        context,
        command,
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
    ensure_initial_world_event(session)?;
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
    Ok(())
}

fn ensure_initial_world_event(session: &GameSession) -> Result<(), ApiError> {
    ensure_current_world_event(session, None).map(|_| ())
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
        if scenario_progress::find_scenario_rule_by_key(session.id(), rule_key)?.is_none() {
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
            )?;
        }
    }
    sync_scenario_rule_rows_for_session(session, None).map(|_| ())
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

fn sync_objective_rows(
    context: &mut session_context::SessionCallerContext,
    command_id: Option<Id<GameCommand>>,
) -> Result<u32, ApiError> {
    sync_objective_rows_for_session(context.session.id(), command_id)
}

fn sync_objective_rows_for_session(
    session_id: Id<GameSession>,
    command_id: Option<Id<GameCommand>>,
) -> Result<u32, ApiError> {
    let mut touched = 0_u32;
    for seed in &domm_game::first_playable_scenario().central_objectives {
        let Some(object) =
            map_visibility_occupancy::find_world_object_by_session_xy(session_id, seed.x, seed.y)?
        else {
            continue;
        };
        ensure_objective_row_for_object(session_id, &object, &seed.key, command_id)?;
        touched = touched.saturating_add(1);
    }
    Ok(touched)
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
    match scenario_progress::find_objective_by_key(session_id, objective_key)? {
        Some(mut row) => {
            row.participant_id = owner.map(|id| id.key());
            row.object_id = Some(object.id().key());
            row.progress_value = progress_value;
            row.status = status;
            row.last_scored_turn = object.captured_turn;
            if let Some(command_id) = command_id {
                row.last_command_id = Some(command_id.key());
            }
            scenario_progress::update_objective_progress(row)
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
    }
}

fn ensure_current_world_event(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<WorldEventState, ApiError> {
    let event = domm_game::deterministic_world_event(session.seed, session.current_turn);
    match scenario_progress::find_world_event_by_key(session.id(), &event.event_key)? {
        Some(mut row) => {
            if let Some(command_id) = command_id {
                row.last_command_id = Some(command_id.key());
                row = scenario_progress::update_world_event_state(row)?;
            }
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
            Ok(row)
        }
    }
}

fn sync_scenario_rule_rows(
    context: &session_context::SessionCallerContext,
    command_id: Option<Id<GameCommand>>,
) -> Result<u32, ApiError> {
    sync_scenario_rule_rows_for_session(&context.session, command_id)
}

fn sync_scenario_rule_rows_for_session(
    session: &GameSession,
    command_id: Option<Id<GameCommand>>,
) -> Result<u32, ApiError> {
    let objectives = objective_rows_for_session(session.id())?;
    let completed_objectives = objectives
        .iter()
        .filter(|row| row.status == "complete")
        .count() as u32;
    let mut touched = 0_u32;
    for mut rule in scenario_progress::page_scenario_rules_by_status(session.id(), "active")?.items
    {
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
                let claimed = claimed_quest_count(session.id())?;
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
        scenario_progress::update_scenario_rule_state(rule)?;
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
    let mut rows = scenario_progress::page_objectives_by_status(session_id, "active")?.items;
    rows.extend(scenario_progress::page_objectives_by_status(session_id, "complete")?.items);
    rows.sort_by(|left, right| left.objective_key.cmp(&right.objective_key));
    Ok(rows)
}

fn scenario_rule_rows_for_session(
    session_id: Id<GameSession>,
) -> Result<Vec<ScenarioRuleState>, ApiError> {
    let mut rows = scenario_progress::page_scenario_rules_by_status(session_id, "active")?.items;
    rows.extend(scenario_progress::page_scenario_rules_by_status(session_id, "disabled")?.items);
    rows.sort_by(|left, right| left.rule_key.cmp(&right.rule_key));
    Ok(rows)
}

fn claimed_quest_count(session_id: Id<GameSession>) -> Result<u32, ApiError> {
    let participants = sessions::page_participants_by_session_status(
        session_id,
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items;
    let mut claimed = 0_u32;
    for participant in participants {
        claimed = claimed.saturating_add(
            scenario_progress::page_quests_by_participant_status(
                session_id,
                participant.id(),
                "claimed",
            )?
            .items
            .len() as u32,
        );
    }
    Ok(claimed)
}

fn load_quest(
    context: &session_context::SessionCallerContext,
    quest_key: &str,
) -> Result<QuestState, ApiError> {
    ensure_known_quest(quest_key)?;
    scenario_progress::find_quest_by_participant_key(
        context.session.id(),
        context.participant.id(),
        quest_key,
    )?
    .ok_or_else(|| public_error("quest_not_found", "quest was not found", false))
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
    if economy::find_resource_ledger_entry(command_id, ledger_key)?.is_some() {
        return Ok(());
    }
    let balance_after = participant.gold.saturating_add(amount);
    economy::create_resource_ledger_entry(
        session_id,
        participant.id(),
        command_id,
        ledger_key.to_string(),
        turn_number,
        "gold".to_string(),
        i64::try_from(amount).unwrap_or(i64::MAX),
        balance_after,
        "quest_reward".to_string(),
        "applied".to_string(),
    )?;
    participant.gold = balance_after;
    Ok(())
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
