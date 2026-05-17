use super::actions::{apply_stack_attack, reachable_tiles};
use super::initiative::select_active_stack_id;
use super::occupancy::validate_battle_occupancy;
use super::types::{
    BATTLE_ACTION_DEADLINE_MS, BATTLE_SIDE_ATTACKER, BattleActionReceipt, BattleCommandBudget,
    BattleCommandRecord, BattleCoord, BattleError, BattleEventRecord, BattleState,
    BattleSyncOutcome,
};

pub fn submit_battle_action(
    state: &mut BattleState,
    battle_id: &str,
    participant_id: &str,
    battle_stack_id: &str,
    action: &str,
    target_stack_id: Option<&str>,
    destination: Option<BattleCoord>,
    client_nonce: &str,
    now_ms: u64,
) -> Result<BattleActionReceipt, BattleError> {
    let budget = BattleCommandBudget::default();
    let recovered = recover_applying_battle_commands(state, battle_id, budget.max_recoveries)?;
    let sync = sync_battle(state, battle_id, now_ms, budget)?;
    if sync.battle_sync_incomplete {
        return Err(BattleError::TimeoutBudgetExhausted);
    }
    if recovered > budget.max_recoveries {
        return Err(BattleError::RecoveryBudgetExhausted);
    }

    let payload_hash = battle_action_payload_hash(action, target_stack_id, destination);
    if let Some(existing) = state.commands.iter().find(|command| {
        !command.system
            && command.battle_id == battle_id
            && command.actor_participant_id.as_deref() == Some(participant_id)
            && command.battle_stack_id.as_deref() == Some(battle_stack_id)
            && command.client_nonce == client_nonce
    }) {
        if existing.payload_hash != payload_hash {
            return Err(BattleError::DuplicateCommandPayloadMismatch {
                client_nonce: client_nonce.to_string(),
            });
        }
        return command_receipt(state, &existing.command_id);
    }

    validate_player_action(state, battle_id, participant_id, battle_stack_id, now_ms)?;
    let command_id = format!("command:battle:{battle_id}:{participant_id}:{client_nonce}");
    state.commands.push(BattleCommandRecord {
        command_id: command_id.clone(),
        battle_id: battle_id.to_string(),
        actor_participant_id: Some(participant_id.to_string()),
        battle_stack_id: Some(battle_stack_id.to_string()),
        client_nonce: client_nonce.to_string(),
        payload_hash,
        action: action.to_string(),
        target_stack_id: target_stack_id.map(str::to_string),
        destination,
        system: false,
        status: "applying".to_string(),
        created_at: now_ms,
        applied_at: None,
        retryable_error: None,
    });
    apply_battle_command_by_id(state, &command_id, now_ms)?;
    command_receipt(state, &command_id)
}

pub fn sync_battle(
    state: &mut BattleState,
    battle_id: &str,
    now_ms: u64,
    budget: BattleCommandBudget,
) -> Result<BattleSyncOutcome, BattleError> {
    let recovered_commands =
        recover_applying_battle_commands(state, battle_id, budget.max_recoveries)?;
    let mut timeout_actions_applied = 0_u32;
    let mut battle_sync_incomplete = false;

    loop {
        let Some((deadline, active_stack_id)) = due_timeout(state, battle_id, now_ms)? else {
            break;
        };
        if timeout_actions_applied >= budget.max_timeout_actions {
            battle_sync_incomplete = true;
            break;
        }
        apply_timeout_defend(state, battle_id, &active_stack_id, deadline)?;
        timeout_actions_applied = timeout_actions_applied.saturating_add(1);
    }

    Ok(BattleSyncOutcome {
        battle_id: battle_id.to_string(),
        timeout_actions_applied,
        recovered_commands,
        battle_sync_incomplete,
        active_stack_id: state.battle(battle_id)?.active_stack_id.clone(),
    })
}

pub fn recover_applying_battle_commands(
    state: &mut BattleState,
    battle_id: &str,
    max_recoveries: u32,
) -> Result<u32, BattleError> {
    let applying = state
        .commands
        .iter()
        .filter(|command| command.battle_id == battle_id && command.status == "applying")
        .map(|command| command.command_id.clone())
        .collect::<Vec<_>>();
    if applying.len() as u32 > max_recoveries {
        return Err(BattleError::RecoveryBudgetExhausted);
    }
    let mut recovered = 0_u32;
    for command_id in applying {
        let created_at = state
            .commands
            .iter()
            .find(|command| command.command_id == command_id)
            .map_or(0, |command| command.created_at);
        apply_battle_command_by_id(state, &command_id, created_at)?;
        recovered = recovered.saturating_add(1);
    }
    Ok(recovered)
}

pub fn apply_battle_command_by_id(
    state: &mut BattleState,
    command_id: &str,
    deadline_base_ms: u64,
) -> Result<(), BattleError> {
    if state
        .events
        .iter()
        .any(|event| event.command_id == command_id)
    {
        mark_command_applied(state, command_id, deadline_base_ms)?;
        return Ok(());
    }

    let command = state
        .commands
        .iter()
        .find(|command| command.command_id == command_id)
        .cloned()
        .ok_or_else(|| BattleError::InvalidAction {
            action: format!("missing_command:{command_id}"),
        })?;
    let stack_id = command
        .battle_stack_id
        .clone()
        .ok_or_else(|| BattleError::InvalidAction {
            action: command.action.clone(),
        })?;
    let battle_id = command.battle_id.clone();
    let current_round = state.battle(&battle_id)?.current_round;

    match command.action.as_str() {
        "Defend" | "AutoDefend" | "RoundAutoDefend" => {
            let stack = state.stack_mut(&stack_id)?;
            stack.defended_round = current_round;
            stack.acted_round = current_round;
            stack.last_command_id = Some(command.command_id.clone());
        }
        "Wait" => {
            let stack = state.stack_mut(&stack_id)?;
            stack.waited_round = current_round;
            stack.last_command_id = Some(command.command_id.clone());
        }
        "MeleeAttack" | "RangedAttack" | "Attack" => {
            let target =
                command
                    .target_stack_id
                    .as_deref()
                    .ok_or_else(|| BattleError::InvalidAction {
                        action: command.action.clone(),
                    })?;
            apply_stack_attack(state, &battle_id, &stack_id, target, &command.command_id, 0)?;
            let stack = state.stack_mut(&stack_id)?;
            stack.acted_round = current_round;
            stack.last_command_id = Some(command.command_id.clone());
        }
        "Move" => {
            let destination = command
                .destination
                .ok_or_else(|| BattleError::InvalidAction {
                    action: command.action.clone(),
                })?;
            move_stack_to(
                state,
                &battle_id,
                &stack_id,
                destination,
                &command.command_id,
            )?;
            let stack = state.stack_mut(&stack_id)?;
            stack.acted_round = current_round;
            stack.last_command_id = Some(command.command_id.clone());
        }
        other => {
            return Err(BattleError::InvalidAction {
                action: other.to_string(),
            });
        }
    }

    let event_type = if command.system && command.action == "RoundAutoDefend" {
        "battle_round_auto_defend"
    } else if command.system {
        "battle_timeout_auto_defend"
    } else {
        "battle_action_applied"
    };
    append_battle_event(
        state,
        &battle_id,
        &command.command_id,
        event_type,
        &stack_id,
        &format!(
            "{{\"action\":\"{}\",\"stack_id\":\"{}\"}}",
            escape_json(&command.action),
            escape_json(&stack_id)
        ),
    );
    mark_command_applied(state, &command.command_id, deadline_base_ms)?;
    advance_active_stack(state, &battle_id, deadline_base_ms)?;
    validate_battle_occupancy(state, &battle_id)?;
    Ok(())
}

pub fn append_battle_event(
    state: &mut BattleState,
    battle_id: &str,
    command_id: &str,
    event_type: &str,
    subject_id_text: &str,
    payload: &str,
) -> u64 {
    let event_key = format!("event:battle:{command_id}");
    if let Some(existing) = state
        .events
        .iter()
        .find(|event| event.event_key == event_key)
    {
        return existing.event_seq;
    }
    let event_seq = state
        .events
        .iter()
        .filter(|event| event.battle_id == battle_id)
        .map(|event| event.event_seq)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    state.events.push(BattleEventRecord {
        event_seq,
        battle_id: battle_id.to_string(),
        event_key,
        command_id: command_id.to_string(),
        event_type: event_type.to_string(),
        subject_id_text: subject_id_text.to_string(),
        payload: payload.to_string(),
    });
    event_seq
}

pub fn battle_action_payload_hash(
    action: &str,
    target_stack_id: Option<&str>,
    destination: Option<BattleCoord>,
) -> String {
    format!(
        "{action}|target={}|dest={}",
        target_stack_id.unwrap_or(""),
        destination.map_or_else(String::new, |coord| format!("{},{}", coord.x, coord.y))
    )
}

fn validate_player_action(
    state: &BattleState,
    battle_id: &str,
    participant_id: &str,
    battle_stack_id: &str,
    now_ms: u64,
) -> Result<(), BattleError> {
    let battle = state.battle(battle_id)?;
    if battle.state != "active" {
        return Err(BattleError::InvalidAction {
            action: "battle_not_active".to_string(),
        });
    }
    if battle.active_stack_id.as_deref() != Some(battle_stack_id) {
        return Err(BattleError::StackNotActive {
            battle_stack_id: battle_stack_id.to_string(),
        });
    }
    if battle
        .action_deadline_at
        .is_some_and(|deadline| now_ms >= deadline)
    {
        return Err(BattleError::ActionAfterDeadline);
    }
    let stack = state.stack(battle_stack_id)?;
    if stack.owner_participant_id.as_deref() != Some(participant_id) {
        return Err(BattleError::StackNotOwned {
            participant_id: participant_id.to_string(),
        });
    }
    Ok(())
}

fn apply_timeout_defend(
    state: &mut BattleState,
    battle_id: &str,
    active_stack_id: &str,
    deadline: u64,
) -> Result<(), BattleError> {
    let round = state.battle(battle_id)?.current_round;
    let command_id = format!("command:battle:{battle_id}:timeout:{round}:{active_stack_id}");
    if !state
        .commands
        .iter()
        .any(|command| command.command_id == command_id)
    {
        state.commands.push(BattleCommandRecord {
            command_id: command_id.clone(),
            battle_id: battle_id.to_string(),
            actor_participant_id: None,
            battle_stack_id: Some(active_stack_id.to_string()),
            client_nonce: format!("timeout:{round}:{active_stack_id}"),
            payload_hash: battle_action_payload_hash("AutoDefend", None, None),
            action: "AutoDefend".to_string(),
            target_stack_id: None,
            destination: None,
            system: true,
            status: "applying".to_string(),
            created_at: deadline,
            applied_at: None,
            retryable_error: None,
        });
    }
    apply_battle_command_by_id(state, &command_id, deadline)
}

fn due_timeout(
    state: &BattleState,
    battle_id: &str,
    now_ms: u64,
) -> Result<Option<(u64, String)>, BattleError> {
    let battle = state.battle(battle_id)?;
    if battle.state != "active" {
        return Ok(None);
    }
    let Some(deadline) = battle.action_deadline_at else {
        return Ok(None);
    };
    if deadline > now_ms {
        return Ok(None);
    }
    Ok(battle
        .active_stack_id
        .as_ref()
        .map(|stack_id| (deadline, stack_id.clone())))
}

fn move_stack_to(
    state: &mut BattleState,
    battle_id: &str,
    battle_stack_id: &str,
    destination: BattleCoord,
    command_id: &str,
) -> Result<(), BattleError> {
    if !reachable_tiles(state, battle_id, battle_stack_id)?.contains(&destination) {
        return Err(BattleError::ObstacleBlocked {
            battle_id: battle_id.to_string(),
            x: destination.x,
            y: destination.y,
        });
    }
    let stack = state.stack_mut(battle_stack_id)?;
    stack.battle_x = destination.x;
    stack.battle_y = destination.y;
    stack.last_command_id = Some(command_id.to_string());
    let occupancy = state
        .occupancy
        .iter_mut()
        .find(|occupancy| occupancy.battle_stack_id == battle_stack_id)
        .ok_or_else(|| BattleError::MissingStackOccupancy {
            battle_stack_id: battle_stack_id.to_string(),
        })?;
    occupancy.battle_x = destination.x;
    occupancy.battle_y = destination.y;
    occupancy.last_command_id = Some(command_id.to_string());
    Ok(())
}

fn advance_active_stack(
    state: &mut BattleState,
    battle_id: &str,
    deadline_base_ms: u64,
) -> Result<(), BattleError> {
    if resolve_if_winner(state, battle_id)? {
        return Ok(());
    }
    let mut active_stack_id = select_active_stack_id(state, battle_id)?;
    if active_stack_id.is_none() {
        let battle = state.battle_mut(battle_id)?;
        battle.current_round = battle.current_round.saturating_add(1);
        active_stack_id = select_active_stack_id(state, battle_id)?;
    }
    let active_side = active_stack_id
        .as_deref()
        .and_then(|stack_id| state.stack(stack_id).ok())
        .map(|stack| stack.side.clone())
        .unwrap_or_else(|| BATTLE_SIDE_ATTACKER.to_string());
    let battle = state.battle_mut(battle_id)?;
    battle.active_stack_id = active_stack_id;
    battle.active_side = active_side;
    battle.action_deadline_at = battle
        .active_stack_id
        .as_ref()
        .map(|_| deadline_base_ms.saturating_add(BATTLE_ACTION_DEADLINE_MS));
    Ok(())
}

fn resolve_if_winner(state: &mut BattleState, battle_id: &str) -> Result<bool, BattleError> {
    let attacker_alive = state.stacks.iter().any(|stack| {
        stack.battle_id == battle_id && stack.side == BATTLE_SIDE_ATTACKER && stack.is_living()
    });
    let defender_alive = state.stacks.iter().any(|stack| {
        stack.battle_id == battle_id && stack.side != BATTLE_SIDE_ATTACKER && stack.is_living()
    });
    if attacker_alive && defender_alive {
        return Ok(false);
    }
    let winner_participant_id = if attacker_alive {
        state
            .stacks
            .iter()
            .find(|stack| stack.battle_id == battle_id && stack.side == BATTLE_SIDE_ATTACKER)
            .and_then(|stack| stack.owner_participant_id.clone())
    } else {
        None
    };
    let battle = state.battle_mut(battle_id)?;
    battle.state = "resolved".to_string();
    battle.active_stack_id = None;
    battle.action_deadline_at = None;
    battle.winner_participant_id = winner_participant_id;
    Ok(true)
}

fn command_receipt(
    state: &BattleState,
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
        status: command.status.clone(),
        current_round: battle.current_round,
        active_stack_id: battle.active_stack_id.clone(),
        event_seq: state
            .events
            .iter()
            .find(|event| event.command_id == command_id)
            .map(|event| event.event_seq),
    })
}

fn mark_command_applied(
    state: &mut BattleState,
    command_id: &str,
    applied_at: u64,
) -> Result<(), BattleError> {
    let command = state
        .commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
        .ok_or_else(|| BattleError::InvalidAction {
            action: format!("missing_command:{command_id}"),
        })?;
    command.status = "applied".to_string();
    command.applied_at = Some(applied_at);
    Ok(())
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
