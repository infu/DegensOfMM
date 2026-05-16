use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{Battle, BattleStack, Champion, GameCommand, GameSession};
use domm_game::{
    ApiError, BattleActionInput, BattleActionReceipt, BattleCommandBudget, BattleCoord,
    BattleError, BattleSyncOutcome, BattleView, CommandResponse, CommandResult, RollKey,
    legal_actions_for_stack,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::{battles, champions_artifacts, commands_events_effects, content};

use super::{
    battle_aftermath, battle_rows,
    command_response::{self, GameCommandAction},
    session_context::{self, public_error},
};

pub(crate) fn get_battle_state(
    caller: CandidPrincipal,
    session_id: String,
    battle_id: String,
    now_ms: u64,
) -> Result<BattleView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let state = battle_rows::load_battle_state(&context.session, &battle_id)?;
    domm_game::battle_view_for_participant(
        &state,
        &battle_id,
        &context.participant.id().to_string(),
        now_ms,
    )
    .map_err(map_battle_error)
}

pub(crate) fn submit_battle_action(
    caller: CandidPrincipal,
    session_id: String,
    input: BattleActionInput,
    client_nonce: String,
    now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let battle = battle_rows::load_battle_row(&context.session, &input.battle_id)?;
    let payload_json = battle_action_payload_json(&input);
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "submit_battle_action",
        &client_nonce,
        battle.attacker_champion_id.map(Id::from_key),
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let recovery_result = recover_applying_battle_commands(
        &mut context.session,
        battle.id(),
        command.id(),
        &mut events,
        &mut changed_subjects,
    );
    if let Err(error) = recovery_result {
        return command_response::fail_command(caller, &context, command, &client_nonce, error);
    }
    let sync_result = apply_due_timeouts(
        &mut context.session,
        battle.id(),
        now_ms,
        BattleCommandBudget::default().max_timeout_actions,
        &mut events,
        &mut changed_subjects,
    );
    if let Err(error) = sync_result {
        return command_response::fail_command(caller, &context, command, &client_nonce, error);
    }
    let sync_incomplete = sync_result.unwrap_or(false);
    if sync_incomplete {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            public_error(
                "battle_sync_incomplete",
                "battle timeout sync is incomplete; call sync_battle before submitting an action",
                true,
            ),
        );
    }

    let action_result = apply_player_action(
        &mut context.session,
        &context.participant.id().to_string(),
        command.clone(),
        &input,
        now_ms,
        &mut events,
        &mut changed_subjects,
    );
    let receipt = match action_result {
        Ok(receipt) => receipt,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };

    if let Some(updated_session) = crate::repos::sessions::load_session(context.session.id())? {
        context.session = updated_session;
    }
    let result_json = battle_action_result_json(&receipt);
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
        &client_nonce,
        result_json,
        events,
        changed_subjects,
        CommandResult::BattleAction(receipt),
    )
}

pub(crate) fn sync_battle(
    caller: CandidPrincipal,
    session_id: String,
    battle_id: String,
    now_ms: u64,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let battle = battle_rows::load_battle_row(&context.session, &battle_id)?;
    let payload_json = format!(
        r#"{{"battle_id":"{}"}}"#,
        command_response::escape_json(&battle_id)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "sync_battle",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let recovered = match recover_applying_battle_commands(
        &mut context.session,
        battle.id(),
        command.id(),
        &mut events,
        &mut changed_subjects,
    ) {
        Ok(recovered) => recovered,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };
    let sync_incomplete = match apply_due_timeouts(
        &mut context.session,
        battle.id(),
        now_ms,
        BattleCommandBudget::default().max_timeout_actions,
        &mut events,
        &mut changed_subjects,
    ) {
        Ok(sync_incomplete) => sync_incomplete,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };
    if let Err(error) = battle_aftermath::apply_resolved_battle_aftermath(
        &mut context.session,
        command.id(),
        battle.id(),
        &mut events,
        &mut changed_subjects,
    ) {
        return command_response::fail_command(caller, &context, command, &client_nonce, error);
    }
    let active_stack_id = battles::load_battle(battle.id())?
        .and_then(|battle| battle.active_stack_id)
        .map(|id| Id::<BattleStack>::from_key(id).to_string());
    let outcome = BattleSyncOutcome {
        battle_id,
        timeout_actions_applied: events
            .iter()
            .filter(|event| event.event_type == "battle_timeout_auto_defend")
            .count()
            .try_into()
            .unwrap_or(u32::MAX),
        recovered_commands: recovered,
        battle_sync_incomplete: sync_incomplete,
        active_stack_id,
    };

    if let Some(updated_session) = crate::repos::sessions::load_session(context.session.id())? {
        context.session = updated_session;
    }
    let result_json = battle_sync_result_json(&outcome);
    command_response::apply_command_with_result(
        caller,
        &context,
        command,
        &client_nonce,
        result_json,
        events,
        changed_subjects,
        CommandResult::BattleSync(outcome),
    )
}

fn apply_player_action(
    session: &mut GameSession,
    participant_id: &str,
    mut command: GameCommand,
    input: &BattleActionInput,
    now_ms: u64,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<BattleActionReceipt, ApiError> {
    command.status = "applying".to_string();
    command.phase = "applying".to_string();
    command = commands_events_effects::update_game_command(command)?;

    let mut state = battle_rows::load_battle_state(session, &input.battle_id)?;
    validate_player_action(&state, participant_id, input, now_ms)?;
    if input.action == "CastAbility" {
        return apply_cast_ability_command(
            session,
            participant_id,
            command,
            input,
            events,
            changed_subjects,
        );
    }
    let battle_command = battle_rows::battle_action_command(
        &command,
        &input.battle_id,
        Some(participant_id.to_string()),
        input.battle_stack_id.clone(),
        input.action.clone(),
        input.target_stack_id.clone(),
        input.destination,
        false,
    );
    state.commands.push(battle_command);
    domm_game::apply_battle_command_by_id(&mut state, &command.id().to_string(), now_ms)
        .map_err(map_battle_error)?;
    battle_rows::persist_battle_state(&state, command.id())?;
    append_new_battle_events(session, command.id(), &state, events)?;
    battle_aftermath::apply_resolved_battle_aftermath(
        session,
        command.id(),
        session_context::parse_id::<Battle>(&input.battle_id, "battle_id")?,
        events,
        changed_subjects,
    )?;
    changed_subjects.push(command_response::changed(
        "battle",
        &input.battle_id,
        "update",
    ));

    battle_action_receipt(&state, &command.id().to_string()).map_err(map_battle_error)
}

fn apply_cast_ability_command(
    session: &mut GameSession,
    participant_id: &str,
    command: GameCommand,
    input: &BattleActionInput,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<BattleActionReceipt, ApiError> {
    let ability_key = input.ability_key.as_deref().ok_or_else(|| {
        public_error(
            "battle_ability_required",
            "CastAbility requires an ability_key",
            false,
        )
    })?;
    let spell_slug = ability_key.strip_prefix("spell:").ok_or_else(|| {
        public_error(
            "battle_ability_not_supported",
            "only spell abilities can be cast in checkpoint 22",
            false,
        )
    })?;
    let target_stack_id = input.target_stack_id.as_deref().ok_or_else(|| {
        public_error(
            "battle_target_required",
            "CastAbility requires a target stack",
            false,
        )
    })?;
    let mut state = battle_rows::load_battle_state(session, &input.battle_id)?;
    state.commands.push(battle_rows::battle_action_command(
        &command,
        &input.battle_id,
        Some(participant_id.to_string()),
        input.battle_stack_id.clone(),
        input.action.clone(),
        input.target_stack_id.clone(),
        input.destination,
        false,
    ));
    let battle = state
        .battle(&input.battle_id)
        .map_err(map_battle_error)?
        .clone();
    let caster_stack = state
        .stack(&input.battle_stack_id)
        .map_err(map_battle_error)?
        .clone();
    let caster_champion_id = caster_champion_for_battle(&battle, &caster_stack)?;
    let champion_id = session_context::parse_id::<Champion>(&caster_champion_id, "champion_id")?;
    let mut champion = champions_artifacts::load_champion(champion_id)?.ok_or_else(|| {
        public_error(
            "champion_not_found",
            "casting champion was not found",
            false,
        )
    })?;
    if Id::<domm_degens_schema::schema::GameParticipant>::from_key(champion.participant_id)
        .to_string()
        != participant_id
    {
        return Err(public_error(
            "champion_not_owned",
            "casting champion does not belong to the caller",
            false,
        ));
    }
    let spell = content::find_spell_by_ruleset_slug(Id::from_key(session.ruleset_id), spell_slug)?
        .ok_or_else(|| public_error("spell_not_found", "spell definition was not found", false))?;
    if spell.target_type != "enemy_battle_stack" {
        return Err(public_error(
            "invalid_spell_target",
            "battle spell must target an enemy battle stack",
            false,
        ));
    }
    if champions_artifacts::find_champion_spell(champion.id(), spell.id())?.is_none() {
        return Err(public_error(
            "spell_not_learned",
            "champion has not learned this spell",
            false,
        ));
    }
    if champion.last_command_id == Some(command.id().key()) {
        return battle_action_receipt(&state, &command.id().to_string()).map_err(map_battle_error);
    }
    let available_mana = if champion.mana_turn == session.current_turn {
        champion.mana
    } else {
        champion.mana_max
    };
    if spell.mana_cost > available_mana {
        return Err(public_error(
            "insufficient_mana",
            "champion does not have enough mana",
            false,
        ));
    }

    let roll = RollKey::new(
        session.seed.to_string(),
        "battle_spell_damage",
        u32::from(battle.current_round),
        &command.id().to_string(),
        &input.battle_stack_id,
        target_stack_id,
        0,
    )
    .roll_between_inclusive(12, 18)
    .map_err(|error| public_error("rng_error", error.to_string(), true))?;
    let damage = (roll.value + champion.wisdom.max(0) as u64).min(u64::from(u32::MAX)) as u32;
    domm_game::apply_damage_to_stack(
        &mut state,
        target_stack_id,
        damage,
        &command.id().to_string(),
    )
    .map_err(map_battle_error)?;
    let status_key = format!(
        "hexed_until_round:{}",
        battle
            .current_round
            .saturating_add(u16::from(spell.duration_rounds))
    );
    {
        let target = state.stack_mut(target_stack_id).map_err(map_battle_error)?;
        if !target.status_keys.iter().any(|key| key == &status_key) {
            target.status_keys.push(status_key.clone());
            target.status_keys.sort();
        }
        domm_game::validate_battle_stack_status_keys(target).map_err(map_battle_error)?;
    }
    {
        let caster = state
            .stack_mut(&input.battle_stack_id)
            .map_err(map_battle_error)?;
        caster.cast_round = battle.current_round;
        caster.acted_round = battle.current_round;
        caster.last_command_id = Some(command.id().to_string());
    }
    domm_game::append_battle_event(
        &mut state,
        &input.battle_id,
        &command.id().to_string(),
        "battle_spell_cast",
        &input.battle_stack_id,
        &format!(
            r#"{{"spell_slug":"{}","target_stack_id":"{}","damage":{},"roll":{}}}"#,
            command_response::escape_json(spell_slug),
            command_response::escape_json(target_stack_id),
            damage,
            roll.value
        ),
    );
    champion.mana_turn = session.current_turn;
    champion.mana = available_mana - spell.mana_cost;
    champion.last_command_id = Some(command.id().key());
    champions_artifacts::update_champion(champion)?;
    battle_rows::persist_battle_state(&state, command.id())?;
    command_response::ensure_command_effect(
        session.id(),
        command.id(),
        format!("battle_spell:{spell_slug}:{}", input.battle_stack_id),
        "battle_spell_cast".to_string(),
        "battle_stack".to_string(),
        target_stack_id.to_string(),
        format!(
            r#"{{"spell_slug":"{}","damage":{},"status_key":"{}"}}"#,
            command_response::escape_json(spell_slug),
            damage,
            command_response::escape_json(&status_key)
        ),
    )?;
    append_new_battle_events(session, command.id(), &state, events)?;
    battle_aftermath::apply_resolved_battle_aftermath(
        session,
        command.id(),
        session_context::parse_id::<Battle>(&input.battle_id, "battle_id")?,
        events,
        changed_subjects,
    )?;
    changed_subjects.push(command_response::changed(
        "battle",
        &input.battle_id,
        "update",
    ));
    changed_subjects.push(command_response::changed(
        "champion",
        &caster_champion_id,
        "update",
    ));
    battle_action_receipt(&state, &command.id().to_string()).map_err(map_battle_error)
}

fn caster_champion_for_battle(
    battle: &domm_game::BattleRecord,
    caster_stack: &domm_game::BattleStackRecord,
) -> Result<String, ApiError> {
    if caster_stack.side == domm_game::BATTLE_SIDE_ATTACKER {
        battle.attacker_champion_id.clone().ok_or_else(|| {
            public_error(
                "caster_champion_missing",
                "attacker champion is missing for battle cast",
                false,
            )
        })
    } else {
        battle.defender_champion_id.clone().ok_or_else(|| {
            public_error(
                "caster_champion_missing",
                "defender champion is missing for battle cast",
                false,
            )
        })
    }
}

fn recover_applying_battle_commands(
    session: &mut GameSession,
    battle_id: Id<Battle>,
    current_command_id: Id<GameCommand>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<u32, ApiError> {
    let commands = commands_events_effects::page_game_commands_by_session_status(
        session.id(),
        "applying",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let mut recovered = 0_u32;
    for command in commands.items {
        if command.id() == current_command_id || !command_mentions_battle(&command, battle_id) {
            continue;
        }
        if recovered >= BattleCommandBudget::default().max_recoveries {
            return Err(public_error(
                "battle_recovery_budget_exhausted",
                "battle command recovery budget was exhausted",
                true,
            ));
        }
        match command.command_type.as_str() {
            "submit_battle_action" => {
                let input = parse_battle_action_input(&command.payload_json)?;
                let participant_id = command.actor_participant_id.map(|id| {
                    Id::<domm_degens_schema::schema::GameParticipant>::from_key(id).to_string()
                });
                let receipt = if input.action == "CastAbility" {
                    let participant_id = participant_id.ok_or_else(|| {
                        public_error(
                            "battle_actor_missing",
                            "cast recovery is missing participant",
                            true,
                        )
                    })?;
                    apply_cast_ability_command(
                        session,
                        &participant_id,
                        command.clone(),
                        &input,
                        events,
                        changed_subjects,
                    )?
                } else {
                    let mut state = battle_rows::load_battle_state(session, &input.battle_id)?;
                    let battle_command = battle_rows::battle_action_command(
                        &command,
                        &input.battle_id,
                        participant_id,
                        input.battle_stack_id,
                        input.action,
                        input.target_stack_id,
                        input.destination,
                        false,
                    );
                    state.commands.push(battle_command);
                    domm_game::apply_battle_command_by_id(
                        &mut state,
                        &command.id().to_string(),
                        command.created_at.as_millis().try_into().unwrap_or(0),
                    )
                    .map_err(map_battle_error)?;
                    battle_rows::persist_battle_state(&state, command.id())?;
                    append_new_battle_events(session, command.id(), &state, events)?;
                    battle_aftermath::apply_resolved_battle_aftermath(
                        session,
                        command.id(),
                        battle_id,
                        events,
                        changed_subjects,
                    )?;
                    battle_action_receipt(&state, &command.id().to_string())
                        .map_err(map_battle_error)?
                };
                let result_json = battle_action_result_json(&receipt);
                let mut applied = command;
                applied.status = "applied".to_string();
                applied.phase = "complete".to_string();
                applied.result_json = Some(result_json);
                applied.applied_at = Some(Timestamp::now());
                commands_events_effects::update_game_command(applied)?;
                recovered = recovered.saturating_add(1);
            }
            "battle_timeout_auto_defend" => {
                let mut state = battle_rows::load_battle_state_from_row(
                    session,
                    battles::load_battle(battle_id)?.ok_or_else(|| {
                        public_error("battle_not_found", "battle not found during recovery", true)
                    })?,
                )?;
                let timeout = parse_timeout_payload(&command.payload_json)?;
                let battle_command = battle_rows::battle_action_command(
                    &command,
                    &battle_id.to_string(),
                    None,
                    timeout.stack_id,
                    "AutoDefend".to_string(),
                    None,
                    None,
                    true,
                );
                state.commands.push(battle_command);
                domm_game::apply_battle_command_by_id(
                    &mut state,
                    &command.id().to_string(),
                    timeout.deadline_ms,
                )
                .map_err(map_battle_error)?;
                battle_rows::persist_battle_state(&state, command.id())?;
                append_new_battle_events(session, command.id(), &state, events)?;
                battle_aftermath::apply_resolved_battle_aftermath(
                    session,
                    command.id(),
                    battle_id,
                    events,
                    changed_subjects,
                )?;
                let mut applied = command;
                applied.status = "applied".to_string();
                applied.phase = "complete".to_string();
                applied.result_json = Some(command_response::result_json(
                    "battle_timeout_auto_defend",
                    session.current_turn,
                ));
                applied.applied_at = Some(Timestamp::now());
                commands_events_effects::update_game_command(applied)?;
                recovered = recovered.saturating_add(1);
            }
            _ => {}
        }
    }
    Ok(recovered)
}

fn apply_due_timeouts(
    session: &mut GameSession,
    battle_id: Id<Battle>,
    now_ms: u64,
    max_timeout_actions: u32,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<bool, ApiError> {
    let mut applied = 0_u32;
    loop {
        let battle = battles::load_battle(battle_id)?.ok_or_else(|| {
            public_error(
                "battle_not_found",
                "battle not found during timeout sync",
                true,
            )
        })?;
        if battle.state != "active" {
            return Ok(false);
        }
        let Some(deadline) = battle
            .action_deadline_at
            .and_then(|deadline| deadline.as_millis().try_into().ok())
        else {
            return Ok(false);
        };
        if deadline > now_ms {
            return Ok(false);
        }
        let Some(active_stack_id) = battle
            .active_stack_id
            .map(|id| Id::<BattleStack>::from_key(id).to_string())
        else {
            return Ok(false);
        };
        if applied >= max_timeout_actions {
            return Ok(true);
        }
        let timeout_command =
            begin_timeout_command(session, battle_id, &active_stack_id, deadline)?;
        apply_timeout_command(
            session,
            timeout_command,
            battle_id,
            active_stack_id,
            deadline,
            events,
            changed_subjects,
        )?;
        applied = applied.saturating_add(1);
    }
}

fn begin_timeout_command(
    session: &GameSession,
    battle_id: Id<Battle>,
    active_stack_id: &str,
    deadline_ms: u64,
) -> Result<GameCommand, ApiError> {
    let nonce_text = format!("timeout:{battle_id}:{active_stack_id}:{deadline_ms}");
    let client_nonce = command_response::nonce_u64("battle_timeout_auto_defend", &nonce_text);
    if let Some(command) = commands_events_effects::find_game_command_by_idempotency(
        session.id(),
        "system",
        &format!("battle_timeout:{battle_id}"),
        client_nonce,
    )? {
        return Ok(command);
    }
    let payload = format!(
        r#"{{"battle_id":"{}","stack_id":"{}","deadline_ms":{deadline_ms}}}"#,
        battle_id,
        command_response::escape_json(active_stack_id)
    );
    let hash = command_response::payload_hash(
        "battle_timeout_auto_defend",
        &format!("battle_timeout:{battle_id}"),
        &nonce_text,
        &payload,
    );
    commands_events_effects::create_game_command(
        session.id(),
        "system".to_string(),
        format!("battle_timeout:{battle_id}"),
        None,
        None,
        None,
        session.current_turn,
        client_nonce,
        "battle_timeout_auto_defend".to_string(),
        hash,
        payload,
    )
}

fn apply_timeout_command(
    session: &mut GameSession,
    mut command: GameCommand,
    battle_id: Id<Battle>,
    active_stack_id: String,
    deadline_ms: u64,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    if command.status == "applied" {
        return Ok(());
    }
    command.status = "applying".to_string();
    command.phase = "applying".to_string();
    command = commands_events_effects::update_game_command(command)?;

    let battle = battles::load_battle(battle_id)?.ok_or_else(|| {
        public_error(
            "battle_not_found",
            "battle not found during timeout apply",
            true,
        )
    })?;
    let mut state = battle_rows::load_battle_state_from_row(session, battle)?;
    let battle_command = battle_rows::battle_action_command(
        &command,
        &battle_id.to_string(),
        None,
        active_stack_id,
        "AutoDefend".to_string(),
        None,
        None,
        true,
    );
    state.commands.push(battle_command);
    domm_game::apply_battle_command_by_id(&mut state, &command.id().to_string(), deadline_ms)
        .map_err(map_battle_error)?;
    battle_rows::persist_battle_state(&state, command.id())?;
    append_new_battle_events(session, command.id(), &state, events)?;
    battle_aftermath::apply_resolved_battle_aftermath(
        session,
        command.id(),
        battle_id,
        events,
        changed_subjects,
    )?;
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(command_response::result_json(
        "battle_timeout_auto_defend",
        session.current_turn,
    ));
    command.applied_at = Some(Timestamp::now());
    commands_events_effects::update_game_command(command)?;
    changed_subjects.push(command_response::changed(
        "battle",
        &battle_id.to_string(),
        "timeout",
    ));
    Ok(())
}

fn validate_player_action(
    state: &domm_game::BattleState,
    participant_id: &str,
    input: &BattleActionInput,
    now_ms: u64,
) -> Result<(), ApiError> {
    let battle = state.battle(&input.battle_id).map_err(map_battle_error)?;
    if battle.state != "active" {
        return Err(public_error(
            "battle_not_active",
            "battle is not active",
            false,
        ));
    }
    if battle.active_stack_id.as_deref() != Some(&input.battle_stack_id) {
        return Err(map_battle_error(BattleError::StackNotActive {
            battle_stack_id: input.battle_stack_id.clone(),
        }));
    }
    if battle
        .action_deadline_at
        .is_some_and(|deadline| now_ms >= deadline)
    {
        return Err(map_battle_error(BattleError::ActionAfterDeadline));
    }
    let stack = state
        .stack(&input.battle_stack_id)
        .map_err(map_battle_error)?;
    if stack.owner_participant_id.as_deref() != Some(participant_id) {
        return Err(map_battle_error(BattleError::StackNotOwned {
            participant_id: participant_id.to_string(),
        }));
    }
    if input.action == "CastAbility" {
        return validate_cast_ability_action(state, input);
    }
    let legal_actions = legal_actions_for_stack(state, &input.battle_id, &input.battle_stack_id)
        .map_err(map_battle_error)?;
    let Some(action) = legal_actions
        .iter()
        .find(|action| action.action == input.action && action.enabled)
    else {
        return Err(public_error(
            "battle_action_not_legal",
            format!("battle action is not legal: {}", input.action),
            false,
        ));
    };
    match input.action.as_str() {
        "Move" => {
            let Some(destination) = input.destination else {
                return Err(public_error(
                    "battle_destination_required",
                    "move action requires a destination",
                    false,
                ));
            };
            if !action.path.contains(&destination) {
                return Err(public_error(
                    "battle_destination_not_reachable",
                    "move destination is not reachable",
                    false,
                ));
            }
        }
        "MeleeAttack" | "RangedAttack" | "Attack" => {
            let Some(target) = input.target_stack_id.as_deref() else {
                return Err(public_error(
                    "battle_target_required",
                    "attack action requires a target stack",
                    false,
                ));
            };
            if !action.targets.iter().any(|candidate| candidate == target) {
                return Err(public_error(
                    "battle_target_not_legal",
                    "attack target is not legal",
                    false,
                ));
            }
        }
        "Defend" | "Wait" => {}
        "CastAbility" => {}
        other => {
            return Err(public_error(
                "battle_action_not_supported",
                format!("battle action is not supported: {other}"),
                false,
            ));
        }
    }
    Ok(())
}

fn validate_cast_ability_action(
    state: &domm_game::BattleState,
    input: &BattleActionInput,
) -> Result<(), ApiError> {
    let ability_key = input.ability_key.as_deref().ok_or_else(|| {
        public_error(
            "battle_ability_required",
            "CastAbility requires an ability_key",
            false,
        )
    })?;
    if !ability_key.starts_with("spell:") {
        return Err(public_error(
            "battle_ability_not_supported",
            "only learned spell abilities are supported by CastAbility",
            false,
        ));
    }
    let target_id = input.target_stack_id.as_deref().ok_or_else(|| {
        public_error(
            "battle_target_required",
            "CastAbility requires a target stack",
            false,
        )
    })?;
    let battle = state.battle(&input.battle_id).map_err(map_battle_error)?;
    let caster = state
        .stack(&input.battle_stack_id)
        .map_err(map_battle_error)?;
    if caster.cast_round >= battle.current_round {
        return Err(public_error(
            "battle_stack_already_cast",
            "this stack already cast this round",
            false,
        ));
    }
    let target = state.stack(target_id).map_err(map_battle_error)?;
    if caster.side == target.side || !target.is_living() {
        return Err(public_error(
            "battle_target_not_legal",
            "CastAbility target must be a living enemy stack",
            false,
        ));
    }
    Ok(())
}

fn append_new_battle_events(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    state: &domm_game::BattleState,
    events: &mut Vec<domm_game::ApiEventView>,
) -> Result<(), ApiError> {
    for event in state
        .events
        .iter()
        .filter(|event| event.command_id == command_id.to_string())
    {
        let event = command_response::append_public_event(
            session,
            command_id,
            format!("battle:{}:{}", event.battle_id, event.event_key),
            event.event_type.clone(),
            Some("battle".to_string()),
            Some(event.battle_id.clone()),
            format!(
                r#"{{"battle_id":"{}","subject_id_text":"{}","payload":{}}}"#,
                command_response::escape_json(&event.battle_id),
                command_response::escape_json(&event.subject_id_text),
                json_string(&event.payload)
            ),
        )?;
        events.push(event);
    }
    Ok(())
}

fn battle_action_receipt(
    state: &domm_game::BattleState,
    command_id: &str,
) -> Result<BattleActionReceipt, BattleError> {
    let command = state
        .commands
        .iter()
        .find(|command| command.command_id == command_id)
        .ok_or_else(|| BattleError::InvalidAction {
            action: format!("missing_command:{command_id}"),
        })?;
    let battle = state.battle(&command.battle_id)?;
    Ok(BattleActionReceipt {
        command_id: command.command_id.clone(),
        status: "applied".to_string(),
        current_round: battle.current_round,
        active_stack_id: battle.active_stack_id.clone(),
        event_seq: state
            .events
            .iter()
            .find(|event| event.command_id == command_id)
            .map(|event| event.event_seq),
    })
}

fn command_mentions_battle(command: &GameCommand, battle_id: Id<Battle>) -> bool {
    command.payload_json.contains(&battle_id.to_string())
}

fn battle_action_payload_json(input: &BattleActionInput) -> String {
    format!(
        r#"{{"battle_id":"{}","battle_stack_id":"{}","action":"{}","ability_key":{},"target_stack_id":{},"destination":{}}}"#,
        command_response::escape_json(&input.battle_id),
        command_response::escape_json(&input.battle_stack_id),
        command_response::escape_json(&input.action),
        input
            .ability_key
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        input
            .target_stack_id
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        input
            .destination
            .map(|coord| format!(r#"{{"x":{},"y":{}}}"#, coord.x, coord.y))
            .unwrap_or_else(|| "null".to_string())
    )
}

fn parse_battle_action_input(payload: &str) -> Result<BattleActionInput, ApiError> {
    Ok(BattleActionInput {
        battle_id: json_field(payload, "battle_id").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "battle action payload is missing battle_id",
                true,
            )
        })?,
        battle_stack_id: json_field(payload, "battle_stack_id").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "battle action payload is missing battle_stack_id",
                true,
            )
        })?,
        action: json_field(payload, "action").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "battle action payload is missing action",
                true,
            )
        })?,
        ability_key: json_nullable_field(payload, "ability_key"),
        target_stack_id: json_nullable_field(payload, "target_stack_id"),
        destination: parse_destination(payload),
    })
}

struct TimeoutPayload {
    stack_id: String,
    deadline_ms: u64,
}

fn parse_timeout_payload(payload: &str) -> Result<TimeoutPayload, ApiError> {
    Ok(TimeoutPayload {
        stack_id: json_field(payload, "stack_id").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "timeout payload is missing stack_id",
                true,
            )
        })?,
        deadline_ms: json_u64_field(payload, "deadline_ms").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "timeout payload is missing deadline_ms",
                true,
            )
        })?,
    })
}

fn parse_destination(payload: &str) -> Option<BattleCoord> {
    let needle = r#""destination":{"#;
    let start = payload.find(needle)? + needle.len();
    let rest = payload.get(start..)?;
    let end = rest.find('}')?;
    let object = rest.get(..end)?;
    Some(BattleCoord {
        x: json_u64_field(object, "x")?.try_into().ok()?,
        y: json_u64_field(object, "y")?.try_into().ok()?,
    })
}

fn battle_action_result_json(receipt: &BattleActionReceipt) -> String {
    format!(
        r#"{{"command_kind":"submit_battle_action","current_round":{},"active_stack_id":{},"event_seq":{},"command_count":1,"event_count":{}}}"#,
        receipt.current_round,
        receipt
            .active_stack_id
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        receipt
            .event_seq
            .map(|seq| seq.to_string())
            .unwrap_or_else(|| "null".to_string()),
        u32::from(receipt.event_seq.is_some())
    )
}

fn battle_sync_result_json(outcome: &BattleSyncOutcome) -> String {
    format!(
        r#"{{"command_kind":"sync_battle","battle_id":"{}","timeout_actions_applied":{},"recovered_commands":{},"battle_sync_incomplete":{},"active_stack_id":{},"command_count":1,"event_count":{}}}"#,
        command_response::escape_json(&outcome.battle_id),
        outcome.timeout_actions_applied,
        outcome.recovered_commands,
        outcome.battle_sync_incomplete,
        outcome
            .active_stack_id
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        outcome.timeout_actions_applied
    )
}

fn json_field(payload: &str, field: &str) -> Option<String> {
    let needle = format!(r#""{field}":"#);
    let start = payload.find(&needle)? + needle.len();
    let rest = payload.get(start..)?.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

fn json_nullable_field(payload: &str, field: &str) -> Option<String> {
    json_field(payload, field)
}

fn json_u64_field(payload: &str, field: &str) -> Option<u64> {
    let needle = format!(r#""{field}":"#);
    let start = payload.find(&needle)? + needle.len();
    let rest = payload.get(start..)?;
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    rest.get(..end)?.parse().ok()
}

fn json_string(value: &str) -> String {
    format!(r#""{}""#, command_response::escape_json(value))
}

fn map_battle_error(error: BattleError) -> ApiError {
    match error {
        BattleError::BattleNotFound { .. } => {
            ApiError::new("battle_not_found", error.to_string(), false)
        }
        BattleError::StackNotFound { .. } => {
            ApiError::new("battle_stack_not_found", error.to_string(), false)
        }
        BattleError::DuplicateCommandPayloadMismatch { .. } => {
            ApiError::new("duplicate_nonce_payload_mismatch", error.to_string(), false)
        }
        BattleError::RecoveryBudgetExhausted => {
            ApiError::new("battle_recovery_budget_exhausted", error.to_string(), true)
        }
        BattleError::TimeoutBudgetExhausted => {
            ApiError::new("battle_timeout_budget_exhausted", error.to_string(), true)
        }
        BattleError::ActionAfterDeadline => {
            ApiError::new("battle_action_after_deadline", error.to_string(), false)
        }
        BattleError::StackNotActive { .. } => {
            ApiError::new("battle_stack_not_active", error.to_string(), false)
        }
        BattleError::StackNotOwned { .. } => {
            ApiError::new("battle_stack_not_owned", error.to_string(), false)
        }
        _ => ApiError::new("battle_action_invalid", error.to_string(), false),
    }
}
