use std::collections::BTreeSet;

use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    Battle, BattleStack, Champion, GameCommand, GameParticipant, GameSession, SystemJob, Town,
};
use domm_game::{
    ApiError, BattleActionInput, BattleActionReceipt, BattleCommandBudget, BattleCoord,
    BattleError, BattleStackRecord, BattleSyncOutcome, BattleView, CommandPhase, CommandResponse,
    CommandResult, CommandStatus, LegalBattleAction, RollKey, legal_actions_for_stack,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp, Ulid},
};

use crate::repos::{
    battle_round_ready, battles, champions_artifacts, commands_events_effects, content, sessions,
    system_jobs as system_job_repo, towns,
};

use super::{
    battle_aftermath, battle_rows,
    battle_runtime::{
        self, BattleRuntime, BattleRuntimeCommandReceipt, BattleRuntimeEvent, BattleRuntimeReadyKey,
    },
    command_response::{self, GameCommandAction},
    session_context::{self, public_error},
    system_jobs as system_job_service,
};

const SYSTEM_JOB_PARTIAL_RETRY_DELAY_MS: i64 = 1_000;
const CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE: u32 = 1;
const CANISTER_MAX_BATTLE_ROUND_AUTO_DEFENDS_PER_UPDATE: u32 = 1;
const CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS: u64 = 15_000;
const CANISTER_MAX_BATTLE_STACKS_PER_BATTLE: u32 =
    (domm_game::MAX_STACKS_PER_BATTLE_SIDE as u32) * 2;
const CANISTER_RUNTIME_BATTLE_TRANSIENT_HISTORY_LIMIT: usize = 16;
const AUTO_ENEMY_TARGET_ID: &str = "auto:enemy";

#[derive(Clone, Debug)]
struct RuntimeBattleCommandContext {
    command_id: String,
    client_nonce_text: String,
    client_nonce: u64,
    payload_hash: String,
    created_at_ms: u64,
}

pub(crate) fn schedule_battle_timeout_job(
    session_id: Id<GameSession>,
    battle: &Battle,
) -> Result<(), ApiError> {
    if battle.state != "active" {
        return Ok(());
    }
    let Some(deadline) = battle.action_deadline_at else {
        return Ok(());
    };
    schedule_battle_timeout_job_at(session_id, battle.id(), battle.created_turn, deadline)
}

fn schedule_battle_timeout_job_at(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    created_turn: u32,
    deadline: Timestamp,
) -> Result<(), ApiError> {
    system_job_service::schedule_job(system_job_repo::SystemJobDraft {
        job_key: format!("battle_timeout:{battle_id}:{}", deadline.as_millis()),
        job_kind: "battle_timeout".to_string(),
        session_id,
        battle_id: Some(battle_id),
        turn_number: Some(created_turn),
        due_at: deadline,
        command_id: None,
        cursor_json: None,
    })?;
    Ok(())
}

pub(crate) fn get_battle_state(
    caller: CandidPrincipal,
    session_id: String,
    battle_id: String,
    now_ms: u64,
) -> Result<BattleView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let participant_id = context.participant.id().to_string();
    let canonical_session_id = context.session.id().to_string();
    if let Some(view) = battle_runtime::with_runtime(&battle_id, |runtime| {
        (runtime.session_id == canonical_session_id)
            .then(|| battle_view_from_runtime(runtime, &participant_id, now_ms))
    }) {
        if let Some(view) = view {
            return view;
        }
    }

    let battle = battle_rows::load_battle_row(&context.session, &battle_id)?;
    if battle.state == "active" {
        battle_runtime::adopt_active_battle_from_rows(&context.session, battle.clone())?;
        let canonical_battle_id = battle.id().to_string();
        if let Some(view) = battle_runtime::with_runtime(&canonical_battle_id, |runtime| {
            battle_view_from_runtime(runtime, &participant_id, now_ms)
        }) {
            return view;
        }
    }

    let stacks = battles::list_battle_stacks(battle.id(), CANISTER_MAX_BATTLE_STACKS_PER_BATTLE)?;
    if !battle_visible_to_participant_from_stacks(&stacks, context.participant.id()) {
        return Err(public_error(
            "battle_not_visible",
            "caller is not a participant in this battle",
            false,
        ));
    }
    let suppress_actions = battle.state != "active"
        || should_suppress_battle_actions(&battle, context.participant.id(), now_ms)?;
    let mut view =
        canister_battle_view_from_rows(&battle, &stacks, &participant_id, now_ms, suppress_actions);
    if !suppress_actions {
        enrich_battle_spell_actions_from_rows(
            &context.session,
            &battle,
            &stacks,
            &mut view,
            &participant_id,
        )?;
    }
    Ok(view)
}

fn battle_view_from_runtime(
    runtime: &BattleRuntime,
    caller_participant_id: &str,
    now_ms: u64,
) -> Result<BattleView, ApiError> {
    if !battle_visible_to_participant_from_runtime(runtime, caller_participant_id) {
        return Err(public_error(
            "battle_not_visible",
            "caller is not a participant in this battle",
            false,
        ));
    }

    let suppress_actions = runtime_battle_actions_suppressed(runtime, now_ms);
    let mut view = canister_battle_view_from_runtime(
        runtime,
        caller_participant_id,
        now_ms,
        suppress_actions,
    )?;
    if !suppress_actions {
        enrich_battle_spell_actions_from_runtime(runtime, &mut view, caller_participant_id)?;
    }
    Ok(view)
}

fn battle_visible_to_participant_from_runtime(
    runtime: &BattleRuntime,
    caller_participant_id: &str,
) -> bool {
    runtime.state.stacks.iter().any(|stack| {
        stack.battle_id == runtime.battle_id
            && stack.owner_participant_id.as_deref() == Some(caller_participant_id)
    })
}

fn runtime_battle_actions_suppressed(runtime: &BattleRuntime, now_ms: u64) -> bool {
    runtime
        .state
        .battle(&runtime.battle_id)
        .is_ok_and(|battle| battle.state != "active")
        || runtime
            .deadline
            .action_deadline_at_ms
            .is_some_and(|deadline| {
                now_ms > deadline.saturating_add(CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS)
            })
}

fn battle_visible_to_participant_from_stacks(
    stacks: &[BattleStack],
    participant_id: Id<GameParticipant>,
) -> bool {
    stacks
        .iter()
        .any(|stack| stack.owner_participant_id == Some(participant_id.key()))
}

fn should_suppress_battle_actions(
    battle: &Battle,
    _participant_id: Id<GameParticipant>,
    now_ms: u64,
) -> Result<bool, ApiError> {
    if battle
        .action_deadline_at
        .and_then(|deadline| u64::try_from(deadline.as_millis()).ok())
        .is_some_and(|deadline| {
            now_ms > deadline.saturating_add(CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS)
        })
    {
        return Ok(true);
    }
    Ok(false)
}

fn canister_battle_view_from_runtime(
    runtime: &BattleRuntime,
    caller_participant_id: &str,
    now_ms: u64,
    suppress_actions: bool,
) -> Result<BattleView, ApiError> {
    let battle = runtime
        .state
        .battle(&runtime.battle_id)
        .map_err(map_battle_error)?;
    let active_stack = battle
        .active_stack_id
        .as_deref()
        .and_then(|active_id| runtime.state.stack(active_id).ok());
    let active_participant_id = active_stack.and_then(|stack| stack.owner_participant_id.clone());
    let legal_actions_for_caller = if suppress_actions {
        Vec::new()
    } else {
        match active_stack {
            Some(stack) if stack.owner_participant_id.as_deref() == Some(caller_participant_id) => {
                cheap_legal_actions_for_stack_runtime(
                    &runtime.battle_id,
                    battle.current_round,
                    &runtime.state.stacks,
                    stack,
                )
            }
            _ => Vec::new(),
        }
    };
    let initiative_order = domm_game::initiative_order(&runtime.state, &runtime.battle_id)
        .map_err(map_battle_error)?
        .into_iter()
        .map(|entry| entry.battle_stack_id)
        .collect();

    Ok(BattleView {
        battle_id: battle.battle_id.clone(),
        state: battle.state.clone(),
        battle_type: battle.battle_type.clone(),
        current_round: battle.current_round,
        active_stack_id: battle.active_stack_id.clone(),
        active_participant_id,
        action_deadline_at: battle.action_deadline_at,
        remaining_ms: battle
            .action_deadline_at
            .map(|deadline| deadline.saturating_sub(now_ms)),
        grid: domm_game::BattleGridView {
            width: battle.grid_width,
            height: battle.grid_height,
        },
        obstacles: runtime
            .state
            .obstacles
            .iter()
            .filter(|obstacle| obstacle.battle_id == runtime.battle_id)
            .map(|obstacle| domm_game::BattleObstacleView {
                battle_obstacle_id: obstacle.battle_obstacle_id.clone(),
                obstacle_type: obstacle.obstacle_type.clone(),
                battle_x: obstacle.battle_x,
                battle_y: obstacle.battle_y,
                width: obstacle.width,
                height: obstacle.height,
                hp: obstacle.hp,
                state: obstacle.state.clone(),
            })
            .collect(),
        stacks: runtime
            .state
            .stacks
            .iter()
            .filter(|stack| stack.battle_id == runtime.battle_id)
            .map(|stack| domm_game::BattleStackView {
                battle_stack_id: stack.battle_stack_id.clone(),
                unit_id: stack.unit_id.clone(),
                side: stack.side.clone(),
                owner_participant_id: stack.owner_participant_id.clone(),
                battle_x: stack.battle_x,
                battle_y: stack.battle_y,
                quantity: stack.quantity,
                front_hp: stack.front_hp,
                shots_remaining: stack.shots_remaining,
                champion_might: stack.champion_might,
                champion_guard: stack.champion_guard,
                attack: stack.attack,
                defense: stack.defense,
                damage_min: stack.damage_min,
                damage_max: stack.damage_max,
                max_hp: stack.max_hp,
                speed: stack.speed,
                initiative: stack.initiative,
                ranged: stack.ranged,
                flying: stack.flying,
                acted_round: stack.acted_round,
                waited_round: stack.waited_round,
                defended_round: stack.defended_round,
                status: stack.status.clone(),
                status_keys: stack.status_keys.clone(),
            })
            .collect(),
        initiative_order,
        legal_actions_for_caller,
        events: runtime
            .state
            .events
            .iter()
            .filter(|event| event.battle_id == runtime.battle_id)
            .map(|event| domm_game::BattleEventView {
                event_seq: event.event_seq,
                event_key: event.event_key.clone(),
                event_type: event.event_type.clone(),
                subject_id_text: event.subject_id_text.clone(),
                payload: event.payload.clone(),
            })
            .collect(),
        next_event_seq: runtime
            .state
            .events
            .iter()
            .filter(|event| event.battle_id == runtime.battle_id)
            .map(|event| event.event_seq)
            .max()
            .unwrap_or(0)
            .saturating_add(1),
        morale_luck_policy: domm_game::v1_morale_luck_policy(),
    })
}

fn battle_action_submit_grace_applies(
    battle: &Battle,
    input: &BattleActionInput,
    now_ms: u64,
) -> bool {
    let Some(deadline_ms) = battle
        .action_deadline_at
        .and_then(|deadline| u64::try_from(deadline.as_millis()).ok())
    else {
        return false;
    };
    if now_ms < deadline_ms
        || now_ms > deadline_ms.saturating_add(CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS)
    {
        return false;
    }
    battle
        .active_stack_id
        .map(|id| Id::<BattleStack>::from_key(id).to_string())
        .as_deref()
        == Some(input.battle_stack_id.as_str())
}

#[allow(dead_code)]
fn canister_battle_view_from_rows(
    battle: &Battle,
    stacks: &[BattleStack],
    caller_participant_id: &str,
    now_ms: u64,
    suppress_actions: bool,
) -> BattleView {
    let battle_id = battle.id().to_string();
    let active_stack = battle
        .active_stack_id
        .map(Id::<BattleStack>::from_key)
        .and_then(|active_id| stacks.iter().find(|stack| stack.id() == active_id));
    let active_participant_id = active_stack.and_then(|stack| {
        stack
            .owner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string())
    });
    let legal_actions_for_caller = if suppress_actions {
        Vec::new()
    } else {
        match active_stack {
            Some(stack)
                if stack
                    .owner_participant_id
                    .map(|id| Id::<GameParticipant>::from_key(id).to_string())
                    .as_deref()
                    == Some(caller_participant_id) =>
            {
                cheap_legal_actions_for_stack_rows(&battle_id, battle.current_round, stacks, stack)
            }
            _ => Vec::new(),
        }
    };

    let mut initiative_order = stacks
        .iter()
        .filter(|stack| stack.status == "active" && stack.quantity > 0)
        .collect::<Vec<_>>();
    initiative_order.sort_by(|left, right| {
        right
            .initiative
            .cmp(&left.initiative)
            .then_with(|| right.speed.cmp(&left.speed))
            .then_with(|| left.id().cmp(&right.id()))
    });

    BattleView {
        battle_id,
        state: battle.state.clone(),
        battle_type: battle.battle_type.clone(),
        current_round: battle.current_round,
        active_stack_id: battle
            .active_stack_id
            .map(|id| Id::<BattleStack>::from_key(id).to_string()),
        active_participant_id,
        action_deadline_at: battle
            .action_deadline_at
            .and_then(|timestamp| u64::try_from(timestamp.as_millis()).ok()),
        remaining_ms: battle
            .action_deadline_at
            .and_then(|deadline| u64::try_from(deadline.as_millis()).ok())
            .map(|deadline| deadline.saturating_sub(now_ms)),
        grid: domm_game::BattleGridView {
            width: battle.grid_width,
            height: battle.grid_height,
        },
        obstacles: Vec::new(),
        stacks: stacks
            .iter()
            .map(|stack| domm_game::BattleStackView {
                battle_stack_id: stack.id().to_string(),
                unit_id: Id::<domm_degens_schema::schema::UnitDefinition>::from_key(stack.unit_id)
                    .to_string(),
                side: stack.side.clone(),
                owner_participant_id: stack
                    .owner_participant_id
                    .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
                battle_x: stack.battle_x,
                battle_y: stack.battle_y,
                quantity: stack.quantity,
                front_hp: stack.front_hp,
                shots_remaining: stack.shots_remaining,
                champion_might: 0,
                champion_guard: 0,
                attack: stack.attack,
                defense: stack.defense,
                damage_min: stack.damage_min,
                damage_max: stack.damage_max,
                max_hp: stack.max_hp,
                speed: stack.speed,
                initiative: stack.initiative,
                ranged: stack.ranged,
                flying: stack.flying,
                acted_round: stack.acted_round,
                waited_round: stack.waited_round,
                defended_round: stack.defended_round,
                status: stack.status.clone(),
                status_keys: stack.status_keys.clone(),
            })
            .collect(),
        initiative_order: initiative_order
            .into_iter()
            .map(|stack| stack.id().to_string())
            .collect(),
        legal_actions_for_caller,
        events: Vec::new(),
        next_event_seq: 1,
        morale_luck_policy: domm_game::v1_morale_luck_policy(),
    }
}

#[allow(dead_code)]
fn cheap_legal_actions_for_stack_rows(
    battle_id: &str,
    current_round: u16,
    stacks: &[BattleStack],
    stack: &BattleStack,
) -> Vec<LegalBattleAction> {
    let can_act =
        stack.status == "active" && stack.quantity > 0 && stack.acted_round < current_round;
    let move_path = cheap_move_candidates_for_stack_rows(stacks, stack, can_act);
    let enemies = stacks
        .iter()
        .filter(|target| {
            Id::<Battle>::from_key(target.battle_id).to_string() == battle_id
                && target.side != stack.side
                && target.status == "active"
                && target.quantity > 0
        })
        .collect::<Vec<_>>();
    let adjacent_targets = enemies
        .iter()
        .filter(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y) == 1
        })
        .map(|target| target.id().to_string())
        .collect::<Vec<_>>();
    let ranged_targets = enemies
        .iter()
        .filter(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y) > 1
        })
        .map(|target| target.id().to_string())
        .collect::<Vec<_>>();
    vec![
        LegalBattleAction {
            action: "Move".to_string(),
            ability_key: None,
            enabled: !move_path.is_empty(),
            disabled_reason: move_path
                .is_empty()
                .then(|| "no_reachable_tile".to_string()),
            targets: Vec::new(),
            path: move_path,
            damage_preview: None,
        },
        LegalBattleAction {
            action: "MeleeAttack".to_string(),
            ability_key: None,
            enabled: can_act && !adjacent_targets.is_empty(),
            disabled_reason: adjacent_targets
                .is_empty()
                .then(|| "no_adjacent_enemy".to_string()),
            targets: adjacent_targets,
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "RangedAttack".to_string(),
            ability_key: None,
            enabled: can_act && stack.ranged && !ranged_targets.is_empty(),
            disabled_reason: (!stack.ranged)
                .then(|| "stack_not_ranged".to_string())
                .or_else(|| {
                    ranged_targets
                        .is_empty()
                        .then(|| "no_non_adjacent_enemy".to_string())
                }),
            targets: ranged_targets,
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Defend".to_string(),
            ability_key: None,
            enabled: can_act,
            disabled_reason: None,
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Wait".to_string(),
            ability_key: None,
            enabled: can_act && stack.waited_round < current_round,
            disabled_reason: None,
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "CastAbility".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("no_learned_battle_spell".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Retreat".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("retreat_deferred_v1_no_rehire_flow".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Surrender".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("surrender_deferred_v1_no_payment_terms".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
    ]
}

fn cheap_legal_actions_for_stack_runtime(
    battle_id: &str,
    current_round: u16,
    stacks: &[BattleStackRecord],
    stack: &BattleStackRecord,
) -> Vec<LegalBattleAction> {
    let can_act =
        stack.status == "active" && stack.quantity > 0 && stack.acted_round < current_round;
    let move_path = cheap_move_candidates_for_stack_runtime(battle_id, stacks, stack, can_act);
    let enemies = stacks
        .iter()
        .filter(|target| {
            target.battle_id == battle_id
                && target.side != stack.side
                && target.status == "active"
                && target.quantity > 0
        })
        .collect::<Vec<_>>();
    let adjacent_targets = enemies
        .iter()
        .filter(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y) == 1
        })
        .map(|target| target.battle_stack_id.clone())
        .collect::<Vec<_>>();
    let ranged_targets = enemies
        .iter()
        .filter(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y) > 1
        })
        .map(|target| target.battle_stack_id.clone())
        .collect::<Vec<_>>();
    vec![
        LegalBattleAction {
            action: "Move".to_string(),
            ability_key: None,
            enabled: !move_path.is_empty(),
            disabled_reason: move_path
                .is_empty()
                .then(|| "no_reachable_tile".to_string()),
            targets: Vec::new(),
            path: move_path,
            damage_preview: None,
        },
        LegalBattleAction {
            action: "MeleeAttack".to_string(),
            ability_key: None,
            enabled: can_act && !adjacent_targets.is_empty(),
            disabled_reason: adjacent_targets
                .is_empty()
                .then(|| "no_adjacent_enemy".to_string()),
            targets: adjacent_targets,
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "RangedAttack".to_string(),
            ability_key: None,
            enabled: can_act && stack.ranged && !ranged_targets.is_empty(),
            disabled_reason: (!stack.ranged)
                .then(|| "stack_not_ranged".to_string())
                .or_else(|| {
                    ranged_targets
                        .is_empty()
                        .then(|| "no_non_adjacent_enemy".to_string())
                }),
            targets: ranged_targets,
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Defend".to_string(),
            ability_key: None,
            enabled: can_act,
            disabled_reason: None,
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Wait".to_string(),
            ability_key: None,
            enabled: can_act && stack.waited_round < current_round,
            disabled_reason: None,
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "CastAbility".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("no_learned_battle_spell".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Retreat".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("retreat_deferred_v1_no_rehire_flow".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
        LegalBattleAction {
            action: "Surrender".to_string(),
            ability_key: None,
            enabled: false,
            disabled_reason: Some("surrender_deferred_v1_no_payment_terms".to_string()),
            targets: Vec::new(),
            path: Vec::new(),
            damage_preview: None,
        },
    ]
}

fn cheap_move_candidates_for_stack_rows(
    stacks: &[BattleStack],
    stack: &BattleStack,
    can_act: bool,
) -> Vec<BattleCoord> {
    if !can_act || stack.speed == 0 {
        return Vec::new();
    }
    let Some(target) = stacks
        .iter()
        .filter(|target| {
            target.side != stack.side && target.status == "active" && target.quantity > 0
        })
        .min_by_key(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y)
        })
    else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    push_straight_line_move_candidates(
        stacks,
        stack,
        &mut candidates,
        target.battle_x.cmp(&stack.battle_x),
        std::cmp::Ordering::Equal,
    );
    if candidates.is_empty() {
        push_straight_line_move_candidates(
            stacks,
            stack,
            &mut candidates,
            std::cmp::Ordering::Equal,
            target.battle_y.cmp(&stack.battle_y),
        );
    }
    candidates
}

fn cheap_move_candidates_for_stack_runtime(
    battle_id: &str,
    stacks: &[BattleStackRecord],
    stack: &BattleStackRecord,
    can_act: bool,
) -> Vec<BattleCoord> {
    if !can_act || stack.speed == 0 {
        return Vec::new();
    }
    let Some(target) = stacks
        .iter()
        .filter(|target| {
            target.battle_id == battle_id
                && target.side != stack.side
                && target.status == "active"
                && target.quantity > 0
        })
        .min_by_key(|target| {
            stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y)
        })
    else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    push_straight_line_move_candidates_runtime(
        battle_id,
        stacks,
        stack,
        &mut candidates,
        target.battle_x.cmp(&stack.battle_x),
        std::cmp::Ordering::Equal,
    );
    if candidates.is_empty() {
        push_straight_line_move_candidates_runtime(
            battle_id,
            stacks,
            stack,
            &mut candidates,
            std::cmp::Ordering::Equal,
            target.battle_y.cmp(&stack.battle_y),
        );
    }
    candidates
}

fn push_straight_line_move_candidates(
    stacks: &[BattleStack],
    stack: &BattleStack,
    candidates: &mut Vec<BattleCoord>,
    x_order: std::cmp::Ordering,
    y_order: std::cmp::Ordering,
) {
    let dx = match x_order {
        std::cmp::Ordering::Greater => 1_i16,
        std::cmp::Ordering::Less => -1_i16,
        std::cmp::Ordering::Equal => 0_i16,
    };
    let dy = match y_order {
        std::cmp::Ordering::Greater => 1_i16,
        std::cmp::Ordering::Less => -1_i16,
        std::cmp::Ordering::Equal => 0_i16,
    };
    if dx == 0 && dy == 0 {
        return;
    }

    let mut x = i16::from(stack.battle_x);
    let mut y = i16::from(stack.battle_y);
    for _ in 0..stack.speed {
        x = x.saturating_add(dx);
        y = y.saturating_add(dy);
        if x < 0
            || y < 0
            || x >= i16::from(domm_game::BATTLE_GRID_WIDTH)
            || y >= i16::from(domm_game::BATTLE_GRID_HEIGHT)
        {
            break;
        }
        let coord = BattleCoord::new(x as u8, y as u8);
        if stacks.iter().any(|other| {
            other.id() != stack.id()
                && other.status == "active"
                && other.quantity > 0
                && other.battle_x == coord.x
                && other.battle_y == coord.y
        }) {
            break;
        }
        candidates.push(coord);
    }
}

fn push_straight_line_move_candidates_runtime(
    battle_id: &str,
    stacks: &[BattleStackRecord],
    stack: &BattleStackRecord,
    candidates: &mut Vec<BattleCoord>,
    x_order: std::cmp::Ordering,
    y_order: std::cmp::Ordering,
) {
    let dx = match x_order {
        std::cmp::Ordering::Greater => 1_i16,
        std::cmp::Ordering::Less => -1_i16,
        std::cmp::Ordering::Equal => 0_i16,
    };
    let dy = match y_order {
        std::cmp::Ordering::Greater => 1_i16,
        std::cmp::Ordering::Less => -1_i16,
        std::cmp::Ordering::Equal => 0_i16,
    };
    if dx == 0 && dy == 0 {
        return;
    }

    let mut x = i16::from(stack.battle_x);
    let mut y = i16::from(stack.battle_y);
    for _ in 0..stack.speed {
        x = x.saturating_add(dx);
        y = y.saturating_add(dy);
        if x < 0
            || y < 0
            || x >= i16::from(domm_game::BATTLE_GRID_WIDTH)
            || y >= i16::from(domm_game::BATTLE_GRID_HEIGHT)
        {
            break;
        }
        let coord = BattleCoord::new(x as u8, y as u8);
        if stacks.iter().any(|other| {
            other.battle_id == battle_id
                && other.battle_stack_id != stack.battle_stack_id
                && other.status == "active"
                && other.quantity > 0
                && other.battle_x == coord.x
                && other.battle_y == coord.y
        }) {
            break;
        }
        candidates.push(coord);
    }
}

#[allow(dead_code)]
fn enrich_battle_spell_actions_from_rows(
    _session: &GameSession,
    battle: &Battle,
    stacks: &[BattleStack],
    view: &mut BattleView,
    participant_id: &str,
) -> Result<(), ApiError> {
    let Some(active_stack_id) = view.active_stack_id.as_deref() else {
        return Ok(());
    };
    let Some(caster_stack) = stacks
        .iter()
        .find(|stack| stack.id().to_string() == active_stack_id)
    else {
        return Ok(());
    };
    let caster_participant_id = caster_stack
        .owner_participant_id
        .map(|id| Id::<GameParticipant>::from_key(id).to_string());
    if caster_participant_id.as_deref() != Some(participant_id) {
        return Ok(());
    }
    let spell_slugs = battle_spell_slugs_from_status_keys(&caster_stack.status_keys);
    if spell_slugs.is_empty() {
        return Ok(());
    }

    let targets = stacks
        .iter()
        .filter(|stack| {
            stack.side != caster_stack.side && stack.status == "active" && stack.quantity > 0
        })
        .map(|stack| stack.id().to_string())
        .collect::<Vec<_>>();
    let mut spell_actions = Vec::new();
    for spell_slug in spell_slugs {
        if !is_supported_battle_spell(&spell_slug) {
            continue;
        }
        let disabled_reason = if caster_stack.cast_round >= battle.current_round {
            Some("battle_stack_already_cast".to_string())
        } else if targets.is_empty() {
            Some("battle_target_not_legal".to_string())
        } else {
            None
        };
        spell_actions.push(domm_game::LegalBattleAction {
            action: "CastAbility".to_string(),
            ability_key: Some(format!("spell:{spell_slug}")),
            enabled: disabled_reason.is_none(),
            disabled_reason,
            targets: targets.clone(),
            path: Vec::new(),
            damage_preview: None,
        });
    }
    if !spell_actions.is_empty() {
        view.legal_actions_for_caller
            .retain(|action| action.action != "CastAbility" || action.ability_key.is_some());
        view.legal_actions_for_caller.extend(spell_actions);
    }
    Ok(())
}

fn enrich_battle_spell_actions_from_runtime(
    runtime: &BattleRuntime,
    view: &mut BattleView,
    participant_id: &str,
) -> Result<(), ApiError> {
    let battle = runtime
        .state
        .battle(&runtime.battle_id)
        .map_err(map_battle_error)?;
    let Some(active_stack_id) = view.active_stack_id.as_deref() else {
        return Ok(());
    };
    let Some(caster_stack) = runtime
        .state
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == active_stack_id)
    else {
        return Ok(());
    };
    if caster_stack.owner_participant_id.as_deref() != Some(participant_id) {
        return Ok(());
    }
    let spell_slugs = battle_spell_slugs_from_status_keys(&caster_stack.status_keys);
    if spell_slugs.is_empty() {
        return Ok(());
    }

    let targets = runtime
        .state
        .stacks
        .iter()
        .filter(|stack| {
            stack.battle_id == runtime.battle_id
                && stack.side != caster_stack.side
                && stack.status == "active"
                && stack.quantity > 0
        })
        .map(|stack| stack.battle_stack_id.clone())
        .collect::<Vec<_>>();
    let mut spell_actions = Vec::new();
    for spell_slug in spell_slugs {
        if !is_supported_battle_spell(&spell_slug) {
            continue;
        }
        let disabled_reason = if caster_stack.cast_round >= battle.current_round {
            Some("battle_stack_already_cast".to_string())
        } else if targets.is_empty() {
            Some("battle_target_not_legal".to_string())
        } else {
            None
        };
        spell_actions.push(domm_game::LegalBattleAction {
            action: "CastAbility".to_string(),
            ability_key: Some(format!("spell:{spell_slug}")),
            enabled: disabled_reason.is_none(),
            disabled_reason,
            targets: targets.clone(),
            path: Vec::new(),
            damage_preview: None,
        });
    }
    if !spell_actions.is_empty() {
        view.legal_actions_for_caller
            .retain(|action| action.action != "CastAbility" || action.ability_key.is_some());
        view.legal_actions_for_caller.extend(spell_actions);
    }
    Ok(())
}

fn battle_spell_slugs_from_status_keys(status_keys: &[String]) -> Vec<String> {
    let slugs = status_keys
        .iter()
        .filter_map(|key| key.strip_prefix("battle_spell:"))
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    slugs.into_iter().collect()
}

fn is_supported_battle_spell(spell_slug: &str) -> bool {
    matches!(spell_slug, "hex-spark")
}

pub(crate) fn submit_battle_action(
    caller: CandidPrincipal,
    session_id: String,
    input: BattleActionInput,
    client_nonce: String,
    now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    let mut context =
        crate::metrics::benchmark_phase("submit_battle_action", "auth_context", || {
            if input.action == "CastAbility" {
                session_context::require_active_session_caller(caller, &session_id)
            } else {
                session_context::require_cached_active_session_caller(caller, &session_id)
            }
        })?;
    if input.action != "CastAbility"
        && let Some(response) =
            submit_runtime_battle_action(caller, &mut context, &input, &client_nonce, now_ms)?
    {
        session_context::remember_active_session_caller(caller, &context);
        return Ok(response);
    }
    let battle = crate::metrics::benchmark_phase("submit_battle_action", "load_battle", || {
        battle_rows::load_battle_row(&context.session, &input.battle_id)
    })?;
    if battle.state == "active" {
        battle_runtime::adopt_active_battle_from_rows(&context.session, battle.clone())?;
    }
    if battle.state == "active"
        && input.action != "CastAbility"
        && let Some(response) =
            submit_runtime_battle_action(caller, &mut context, &input, &client_nonce, now_ms)?
    {
        session_context::remember_active_session_caller(caller, &context);
        return Ok(response);
    }
    let command =
        match crate::metrics::benchmark_phase("submit_battle_action", "command_begin", || {
            let payload_json = battle_action_payload_json(&input);
            command_response::begin_participant_command_guarded(
                caller,
                &context,
                "submit_battle_action",
                &client_nonce,
                battle.attacker_champion_id.map(Id::from_key),
                payload_json,
                || {
                    ensure_battle_round_accepts_new_action(
                        context.session.id(),
                        battle.id(),
                        context.participant.id(),
                        battle.current_round,
                    )
                },
            )
        })? {
            GameCommandAction::Apply(command) => command,
            GameCommandAction::Return(response) => return Ok(response),
        };

    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let response_participant_id = context.participant.id().to_string();
    let recovery_result =
        crate::metrics::benchmark_phase("submit_battle_action", "recovery", || {
            recover_applying_battle_commands(
                &mut context.session,
                battle.id(),
                command.id(),
                Some(&response_participant_id),
                &mut events,
                &mut changed_subjects,
            )
        });
    if let Err(error) = recovery_result {
        return command_response::fail_command(caller, &context, command, &client_nonce, error);
    }
    let skip_timeout_for_submit_grace = battle_action_submit_grace_applies(&battle, &input, now_ms);
    let sync_result = crate::metrics::benchmark_phase("submit_battle_action", "timeout", || {
        if skip_timeout_for_submit_grace {
            Ok(false)
        } else {
            match apply_due_runtime_timeouts_for_sync(
                &mut context.session,
                battle.id(),
                now_ms,
                CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE,
                command.id(),
                Some(&response_participant_id),
                &mut events,
                &mut changed_subjects,
            ) {
                Ok(Some((sync_incomplete, _applied))) => Ok(sync_incomplete),
                Ok(None) => apply_due_timeouts(
                    &mut context.session,
                    battle.id(),
                    now_ms,
                    CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE,
                    Some(&response_participant_id),
                    &mut events,
                    &mut changed_subjects,
                ),
                Err(error) => Err(error),
            }
        }
    });
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
                "battle_processing",
                "battle timeout work is still processing; retry after refreshing battle state",
                true,
            ),
        );
    }

    let normalized_input =
        crate::metrics::benchmark_phase("submit_battle_action", "normalize_input", || {
            normalize_battle_action_input(&context.session, &input)
        })?;
    let action_result = apply_player_action(
        &mut context.session,
        &context.participant.id().to_string(),
        command.clone(),
        &normalized_input,
        now_ms,
        &response_participant_id,
        &mut events,
        &mut changed_subjects,
    );
    let receipt = match action_result {
        Ok(receipt) => receipt,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };
    crate::metrics::benchmark_phase("submit_battle_action", "readiness_schedule", || {
        if let Some(readiness) = recompute_runtime_battle_round_readiness_and_schedule(
            context.session.id(),
            battle.id(),
            Some(command.id()),
            true,
            true,
        )? {
            changed_subjects.extend(readiness.changed_subjects);
        } else if let Some(updated_battle) = battles::load_battle(battle.id())? {
            let readiness = recompute_battle_round_readiness_and_schedule(
                context.session.id(),
                &updated_battle,
                Some(command.id()),
                true,
            )?;
            changed_subjects.extend(readiness.changed_subjects);
            schedule_battle_timeout_job(context.session.id(), &updated_battle)?;
        }
        Ok::<(), ApiError>(())
    })?;

    let result_json = battle_action_result_json(&receipt);
    let response =
        crate::metrics::benchmark_phase("submit_battle_action", "final_response", || {
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
        })?;
    session_context::remember_active_session_caller(caller, &context);
    Ok(response)
}

enum RuntimeBattleCommandAction {
    Apply(RuntimeBattleCommandContext),
    Return(CommandResponse),
}

fn submit_runtime_battle_action(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    input: &BattleActionInput,
    client_nonce: &str,
    now_ms: u64,
) -> Result<Option<CommandResponse>, ApiError> {
    let battle_id_text = input.battle_id.clone();
    let battle_id = session_context::parse_id::<Battle>(&battle_id_text, "battle_id")?;
    let Some(runtime) =
        crate::metrics::benchmark_phase("submit_battle_action", "load_battle_state", || {
            battle_runtime::with_runtime(&battle_id_text, Clone::clone)
        })
    else {
        return Ok(None);
    };
    if runtime.session_id != context.session.id().to_string() {
        return Ok(None);
    }
    if runtime_battle_timeout_due_without_submit_grace(&runtime, input, now_ms)? {
        return Ok(None);
    }

    let command =
        match crate::metrics::benchmark_phase("submit_battle_action", "command_begin", || {
            begin_runtime_battle_command(caller, context, battle_id, &runtime, input, client_nonce)
        })? {
            RuntimeBattleCommandAction::Apply(command) => command,
            RuntimeBattleCommandAction::Return(response) => return Ok(Some(response)),
        };

    crate::metrics::benchmark_phase(
        "submit_battle_action",
        "recovery",
        || Ok::<(), ApiError>(()),
    )?;
    crate::metrics::benchmark_phase("submit_battle_action", "timeout", || Ok::<(), ApiError>(()))?;
    let normalized_input =
        crate::metrics::benchmark_phase("submit_battle_action", "normalize_input", || {
            normalize_battle_action_input(&context.session, input)
        })?;

    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let response_participant_id = context.participant.id().to_string();
    let action_result = apply_player_action_from_runtime_parts(
        &mut context.session,
        &response_participant_id,
        &command,
        &normalized_input,
        now_ms,
        &response_participant_id,
        &mut events,
        &mut changed_subjects,
        runtime,
    );

    let response = match action_result {
        Ok(receipt) => {
            crate::metrics::benchmark_phase("submit_battle_action", "readiness_schedule", || {
                if let Some(readiness) = recompute_runtime_battle_round_readiness_and_schedule(
                    context.session.id(),
                    battle_id,
                    None,
                    true,
                    false,
                )? {
                    changed_subjects.extend(readiness.changed_subjects);
                }
                Ok::<(), ApiError>(())
            })?;
            let result = CommandResult::BattleAction(receipt);
            crate::metrics::benchmark_phase("submit_battle_action", "final_response", || {
                Ok::<CommandResponse, ApiError>(command_response::runtime_command_response(
                    caller,
                    context,
                    command.command_id.clone(),
                    "submit_battle_action".to_string(),
                    &command.client_nonce_text,
                    command.payload_hash.clone(),
                    CommandStatus::Applied,
                    CommandPhase::Complete,
                    false,
                    events,
                    changed_subjects,
                    result,
                    None,
                ))
            })?
        }
        Err(error) => {
            let retryable = error.retryable;
            crate::metrics::benchmark_phase("submit_battle_action", "final_response", || {
                Ok::<CommandResponse, ApiError>(command_response::runtime_command_response(
                    caller,
                    context,
                    command.command_id.clone(),
                    "submit_battle_action".to_string(),
                    &command.client_nonce_text,
                    command.payload_hash.clone(),
                    CommandStatus::Failed,
                    CommandPhase::Failed,
                    retryable,
                    Vec::new(),
                    Vec::new(),
                    CommandResult::None,
                    Some(error),
                ))
            })?
        }
    };

    insert_runtime_battle_command_receipt(
        &battle_id_text,
        &response_participant_id,
        command,
        response.clone(),
    );
    Ok(Some(response))
}

fn begin_runtime_battle_command(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    battle_id: Id<Battle>,
    runtime: &BattleRuntime,
    input: &BattleActionInput,
    client_nonce_text: &str,
) -> Result<RuntimeBattleCommandAction, ApiError> {
    let payload_json = battle_action_payload_json(input);
    let client_nonce = command_response::nonce_u64("submit_battle_action", client_nonce_text);
    let payload_hash = command_response::payload_hash(
        "submit_battle_action",
        &context.participant.id().to_string(),
        client_nonce_text,
        &payload_json,
    );
    if payload_json.len() > domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES {
        let response = runtime_battle_failed_response(
            caller,
            context,
            Ulid::generate().to_string(),
            client_nonce_text,
            payload_hash,
            public_error(
                "payload_too_large",
                "game command payload is too large",
                false,
            ),
        );
        return Ok(RuntimeBattleCommandAction::Return(response));
    }

    let canonical_session_id = context.session.id().to_string();
    let actor_participant_id = context.participant.id().to_string();
    if let Some(existing) = battle_runtime::command_receipt_by_nonce(
        &canonical_session_id,
        &actor_participant_id,
        client_nonce,
    ) {
        if existing.payload_hash != payload_hash {
            let response = runtime_battle_failed_response(
                caller,
                context,
                Ulid::generate().to_string(),
                client_nonce_text,
                payload_hash,
                public_error(
                    "duplicate_nonce_payload_mismatch",
                    format!("client nonce {client_nonce_text} was reused with a different payload"),
                    false,
                ),
            );
            return Ok(RuntimeBattleCommandAction::Return(response));
        }
        return Ok(RuntimeBattleCommandAction::Return(existing.response));
    }

    let battle = runtime
        .state
        .battle(&runtime.battle_id)
        .map_err(map_battle_error)?;
    ensure_battle_round_accepts_new_action(
        context.session.id(),
        battle_id,
        context.participant.id(),
        battle.current_round,
    )?;

    Ok(RuntimeBattleCommandAction::Apply(
        RuntimeBattleCommandContext {
            command_id: Ulid::generate().to_string(),
            client_nonce_text: client_nonce_text.to_string(),
            client_nonce,
            payload_hash,
            created_at_ms: Timestamp::now().as_millis().try_into().unwrap_or(0),
        },
    ))
}

fn runtime_battle_failed_response(
    caller: CandidPrincipal,
    context: &session_context::SessionCallerContext,
    command_id: String,
    client_nonce_text: &str,
    payload_hash: String,
    error: ApiError,
) -> CommandResponse {
    let retryable = error.retryable;
    command_response::runtime_command_response(
        caller,
        context,
        command_id,
        "submit_battle_action".to_string(),
        client_nonce_text,
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

fn insert_runtime_battle_command_receipt(
    battle_id: &str,
    actor_participant_id: &str,
    command: RuntimeBattleCommandContext,
    response: CommandResponse,
) {
    let receipt = BattleRuntimeCommandReceipt {
        command_id: command.command_id,
        command_type: "submit_battle_action".to_string(),
        actor_participant_id: actor_participant_id.to_string(),
        client_nonce_text: command.client_nonce_text,
        client_nonce: command.client_nonce,
        payload_hash: command.payload_hash,
        response,
    };
    battle_runtime::with_runtime_mut(battle_id, |runtime| {
        runtime.insert_command_receipt(receipt);
    });
}

fn runtime_battle_timeout_due_without_submit_grace(
    runtime: &BattleRuntime,
    input: &BattleActionInput,
    now_ms: u64,
) -> Result<bool, ApiError> {
    let battle = runtime
        .state
        .battle(&runtime.battle_id)
        .map_err(map_battle_error)?;
    let Some(deadline_ms) = battle.action_deadline_at else {
        return Ok(false);
    };
    if deadline_ms > now_ms {
        return Ok(false);
    }
    let grace_applies = now_ms
        <= deadline_ms.saturating_add(CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS)
        && battle.active_stack_id.as_deref() == Some(input.battle_stack_id.as_str());
    Ok(!grace_applies)
}

fn normalize_battle_action_input(
    session: &GameSession,
    input: &BattleActionInput,
) -> Result<BattleActionInput, ApiError> {
    if input.target_stack_id.as_deref() != Some(AUTO_ENEMY_TARGET_ID) {
        return Ok(input.clone());
    }
    let state = battle_runtime::with_runtime(&input.battle_id, |runtime| {
        (runtime.session_id == session.id().to_string()).then(|| runtime.state.clone())
    })
    .flatten()
    .map_or_else(
        || battle_rows::load_battle_state(session, &input.battle_id),
        Ok,
    )?;
    let caster = state
        .stack(&input.battle_stack_id)
        .map_err(map_battle_error)?;
    let target = state
        .stacks
        .iter()
        .find(|stack| stack.side != caster.side && stack.is_living())
        .ok_or_else(|| {
            public_error(
                "battle_target_not_legal",
                "no living enemy stack is available",
                false,
            )
        })?;
    let mut normalized = input.clone();
    normalized.target_stack_id = Some(target.battle_stack_id.clone());
    Ok(normalized)
}

fn mirror_battle_runtime_from_state(
    session: &GameSession,
    state: &domm_game::BattleState,
) -> Result<(), ApiError> {
    battle_runtime::replace_runtime_from_state(session, state.clone())?;
    Ok(())
}

fn apply_resolved_battle_aftermath_with_runtime_projection(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    let battle_id_text = battle_id.to_string();
    if let Some(runtime) = battle_runtime::with_runtime(&battle_id_text, Clone::clone) {
        let is_resolved = runtime
            .state
            .battle(&battle_id_text)
            .map_err(map_battle_error)?
            .state
            == "resolved";
        if is_resolved {
            battle_rows::persist_battle_state(&runtime.state, command_id)?;
            battle_runtime::archive_runtime_events(&runtime);
            changed_subjects.push(command_response::changed(
                "battle",
                &battle_id_text,
                "runtime_projection",
            ));
            battle_runtime::insert_runtime(runtime);
        }
    }

    battle_aftermath::apply_resolved_battle_aftermath(
        session,
        command_id,
        battle_id,
        events,
        changed_subjects,
    )?;
    if battles::load_battle(battle_id)?.is_some_and(|battle| battle.state != "active") {
        battle_runtime::remove_runtime(&battle_id_text);
    }
    Ok(())
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
    let response_participant_id = context.participant.id().to_string();
    let recovered = match recover_applying_battle_commands(
        &mut context.session,
        battle.id(),
        command.id(),
        Some(&response_participant_id),
        &mut events,
        &mut changed_subjects,
    ) {
        Ok(recovered) => recovered,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };
    let mut used_runtime_sync = false;
    let sync_result = match apply_due_runtime_timeouts_for_sync(
        &mut context.session,
        battle.id(),
        now_ms,
        CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE,
        command.id(),
        Some(&response_participant_id),
        &mut events,
        &mut changed_subjects,
    ) {
        Ok(Some((sync_incomplete, _applied))) => {
            used_runtime_sync = true;
            Ok(sync_incomplete)
        }
        Ok(None) => apply_due_timeouts(
            &mut context.session,
            battle.id(),
            now_ms,
            CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE,
            Some(&response_participant_id),
            &mut events,
            &mut changed_subjects,
        ),
        Err(error) => Err(error),
    };
    let sync_incomplete = match sync_result {
        Ok(sync_incomplete) => sync_incomplete,
        Err(error) => {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
    };
    if !used_runtime_sync && let Some(updated_battle) = battles::load_battle(battle.id())? {
        let readiness = recompute_battle_round_readiness_and_schedule(
            context.session.id(),
            &updated_battle,
            Some(command.id()),
            true,
        )?;
        changed_subjects.extend(readiness.changed_subjects);
    }
    if !used_runtime_sync {
        if let Err(error) = apply_resolved_battle_aftermath_with_runtime_projection(
            &mut context.session,
            command.id(),
            battle.id(),
            &mut events,
            &mut changed_subjects,
        ) {
            return command_response::fail_command(caller, &context, command, &client_nonce, error);
        }
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

pub(crate) fn end_battle_turn(
    caller: CandidPrincipal,
    session_id: String,
    battle_id: String,
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
        "end_battle_turn",
        &client_nonce,
        battle.attacker_champion_id.map(Id::from_key),
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };

    let stacks = battles::page_battle_stacks(battle.id(), domm_game::MAX_LIST_LIMIT, None)?;
    let caller_participant_id = context.participant.id();
    let caller_has_stack = stacks
        .items
        .iter()
        .any(|stack| stack.owner_participant_id == Some(caller_participant_id.key()));
    if !caller_has_stack {
        return command_response::fail_command(
            caller,
            &context,
            command,
            &client_nonce,
            public_error(
                "battle_not_visible",
                "caller does not control stacks in this battle",
                false,
            ),
        );
    }

    let ready = battle_round_ready::mark_battle_round_ready(
        context.session.id(),
        battle.id(),
        caller_participant_id,
        battle.current_round,
        Some(command.id()),
        "player_end_turn".to_string(),
        Timestamp::now(),
    )?;
    battle_runtime::with_runtime_mut(&battle.id().to_string(), |runtime| {
        if runtime.session_id == context.session.id().to_string() {
            runtime.mark_ready(caller_participant_id.to_string(), battle.current_round);
        }
    });
    let mut changed_subjects = vec![command_response::changed(
        "battle_participant_round_ready",
        &ready.id().to_string(),
        "upsert",
    )];
    let readiness = recompute_runtime_battle_round_readiness_and_schedule(
        context.session.id(),
        battle.id(),
        Some(command.id()),
        true,
        false,
    )?
    .map_or_else(
        || {
            recompute_battle_round_readiness_and_schedule(
                context.session.id(),
                &battle,
                Some(command.id()),
                true,
            )
        },
        Ok,
    )?;
    changed_subjects.extend(readiness.changed_subjects);

    let ready_count = readiness.ready_count;
    let participant_count = readiness.participant_count;
    let all_ready = readiness.all_ready;
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!(
            "end_battle_turn:{}:{}:{}",
            battle.id(),
            battle.current_round,
            context.participant.id()
        ),
        "battle_participant_round_ready".to_string(),
        Some("battle".to_string()),
        Some(battle.id().to_string()),
        format!(
            r#"{{"battle_id":"{}","round_number":{},"ready_count":{},"participant_count":{},"all_ready":{}}}"#,
            battle.id(),
            battle.current_round,
            ready_count,
            participant_count,
            all_ready
        ),
    )?;

    command_response::apply_command(
        caller,
        &context,
        command,
        &client_nonce,
        format!(
            r#"{{"command_kind":"end_battle_turn","battle_id":"{}","current_turn":{},"round_number":{},"ready_count":{},"participant_count":{},"all_ready":{},"command_count":1,"event_count":1}}"#,
            battle.id(),
            context.session.current_turn,
            battle.current_round,
            ready_count,
            participant_count,
            all_ready
        ),
        vec![event],
        changed_subjects,
    )
}

pub(crate) fn process_battle_timeout_job(job: SystemJob) -> Result<(), ApiError> {
    let fallback = job.clone();
    if let Err(error) = process_battle_timeout_job_inner(job) {
        system_job_repo::fail_system_job(fallback, error.retryable, error.message.clone())?;
        return Err(error);
    }
    Ok(())
}

fn process_battle_timeout_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(mut session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(
            job,
            false,
            "battle timeout session row not found".to_string(),
        )?;
        return Ok(());
    };
    let Some(battle_id_key) = job.battle_id else {
        system_job_repo::fail_system_job(
            job,
            false,
            "battle timeout job is missing battle_id".to_string(),
        )?;
        return Ok(());
    };
    let battle_id = Id::<Battle>::from_key(battle_id_key);
    if process_runtime_battle_timeout_job(&session, &job, battle_id)? {
        return Ok(());
    }
    let Some(battle) = battles::load_battle(battle_id)? else {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    };
    if battle.state != "active" {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }
    if battle.action_deadline_at != Some(job.due_at) {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }

    let now_ms = u64::try_from(Timestamp::now().as_millis()).unwrap_or(0);
    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let sync_incomplete = apply_due_timeouts(
        &mut session,
        battle_id,
        now_ms,
        CANISTER_MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE,
        None,
        &mut events,
        &mut changed_subjects,
    )?;
    if sync_incomplete {
        system_job_repo::reschedule_system_job(job, partial_retry_at(), None)?;
        return Ok(());
    }

    if let Some(updated_battle) = battles::load_battle(battle_id)? {
        recompute_battle_round_readiness_and_schedule(session.id(), &updated_battle, None, true)?;
    }

    system_job_repo::complete_system_job(job)?;
    if let Some(updated_battle) = battles::load_battle(battle_id)?
        && updated_battle.state == "active"
        && let Some(deadline) = updated_battle.action_deadline_at
    {
        system_job_service::schedule_job(system_job_repo::SystemJobDraft {
            job_key: format!("battle_timeout:{battle_id}:{}", deadline.as_millis()),
            job_kind: "battle_timeout".to_string(),
            session_id,
            battle_id: Some(battle_id),
            turn_number: Some(session.current_turn),
            due_at: deadline,
            command_id: None,
            cursor_json: None,
        })?;
    }
    Ok(())
}

fn process_runtime_battle_timeout_job(
    session: &GameSession,
    job: &SystemJob,
    battle_id: Id<Battle>,
) -> Result<bool, ApiError> {
    let battle_id_text = battle_id.to_string();
    let Some(runtime) = battle_runtime::with_runtime(&battle_id_text, Clone::clone) else {
        return Ok(false);
    };
    if runtime.session_id != session.id().to_string() {
        return Ok(false);
    }
    let battle = runtime
        .state
        .battle(&battle_id_text)
        .map_err(map_battle_error)?
        .clone();
    if battle.state != "active" {
        system_job_repo::complete_system_job(job.clone())?;
        return Ok(true);
    }
    let Some(deadline_ms) = battle.action_deadline_at else {
        system_job_repo::complete_system_job(job.clone())?;
        return Ok(true);
    };
    let deadline = Timestamp::from_millis(i64::try_from(deadline_ms).unwrap_or(i64::MAX));
    if deadline > Timestamp::now() {
        system_job_repo::reschedule_system_job(job.clone(), deadline, None)?;
        system_job_service::schedule_nearest_due_job()?;
        return Ok(true);
    }
    if let Some(active_stack_id) = battle.active_stack_id {
        let command = begin_timeout_command(session, battle_id, &active_stack_id, deadline_ms)?;
        battle_rows::persist_battle_header_from_state(&runtime.state, command.id())?;
    }
    Ok(false)
}

pub(crate) fn process_battle_round_advance_job(job: SystemJob) -> Result<(), ApiError> {
    let fallback = job.clone();
    if let Err(error) = process_battle_round_advance_job_inner(job) {
        system_job_repo::fail_system_job(fallback, error.retryable, error.message.clone())?;
        return Err(error);
    }
    Ok(())
}

fn process_battle_round_advance_job_inner(job: SystemJob) -> Result<(), ApiError> {
    let session_id = Id::<GameSession>::from_key(job.session_id);
    let Some(mut session) = sessions::load_session(session_id)? else {
        system_job_repo::fail_system_job(
            job,
            false,
            "battle round advance session row not found".to_string(),
        )?;
        return Ok(());
    };
    let Some(battle_id_key) = job.battle_id else {
        system_job_repo::fail_system_job(
            job,
            false,
            "battle round advance job is missing battle_id".to_string(),
        )?;
        return Ok(());
    };
    let Some(round_number) = battle_round_from_job_key(&job.job_key) else {
        system_job_repo::fail_system_job(
            job,
            false,
            "battle round advance job key is missing round_number".to_string(),
        )?;
        return Ok(());
    };
    let battle_id = Id::<Battle>::from_key(battle_id_key);
    let Some(battle) = battles::load_battle(battle_id)? else {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    };
    if battle.state != "active" || battle.current_round != round_number {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }

    let readiness = recompute_runtime_battle_round_readiness_and_schedule(
        session_id, battle_id, None, false, false,
    )?
    .map_or_else(
        || recompute_battle_round_readiness_and_schedule(session_id, &battle, None, false),
        Ok,
    )?;
    if !readiness.all_ready {
        system_job_repo::complete_system_job(job)?;
        return Ok(());
    }

    let mut events = Vec::new();
    let mut changed_subjects = Vec::new();
    let now_ms = u64::try_from(Timestamp::now().as_millis()).unwrap_or(0);
    let mut auto_defends = 0_u32;
    let mut round_incomplete = false;
    loop {
        if auto_defends >= CANISTER_MAX_BATTLE_ROUND_AUTO_DEFENDS_PER_UPDATE {
            round_incomplete = true;
            break;
        }
        let current_battle = battles::load_battle(battle_id)?.ok_or_else(|| {
            public_error(
                "battle_not_found",
                "battle disappeared during round advance",
                true,
            )
        })?;
        if current_battle.state != "active" || current_battle.current_round != round_number {
            break;
        }
        let battle_id_text = battle_id.to_string();
        let state = battle_runtime::with_runtime(&battle_id_text, |runtime| {
            (runtime.session_id == session.id().to_string()).then(|| runtime.state.clone())
        })
        .flatten()
        .map_or_else(
            || battle_rows::load_battle_state_from_row(&session, current_battle),
            Ok,
        )?;
        let Some(stack_id) = domm_game::select_active_stack_id(&state, &battle_id.to_string())
            .map_err(map_battle_error)?
        else {
            break;
        };
        let stack = state.stack(&stack_id).map_err(map_battle_error)?;
        if !stack.is_living() || stack.acted_round >= round_number {
            break;
        }
        let command =
            begin_round_auto_defend_command(&session, battle_id, &stack_id, round_number)?;
        apply_round_auto_defend_command(
            &mut session,
            command,
            battle_id,
            stack_id,
            round_number,
            now_ms,
            None,
            &mut events,
            &mut changed_subjects,
        )?;
        auto_defends = auto_defends.saturating_add(1);
    }

    if round_incomplete {
        system_job_repo::reschedule_system_job(job, Timestamp::now(), None)?;
        system_job_service::schedule_nearest_due_job()?;
        return Ok(());
    }

    system_job_repo::complete_system_job(job)?;
    if let Some(updated_battle) = battles::load_battle(battle_id)?
        && updated_battle.state == "active"
    {
        schedule_battle_timeout_job(session.id(), &updated_battle)?;
    }
    Ok(())
}

struct BattleRoundReadinessSummary {
    ready_count: usize,
    participant_count: usize,
    all_ready: bool,
    changed_subjects: Vec<domm_game::ChangedSubject>,
}

struct RuntimeBattleRoundReadinessSummary {
    summary: BattleRoundReadinessSummary,
    round_job_key: Option<String>,
    timeout_deadline: Option<(u32, u64)>,
}

fn recompute_battle_round_readiness_and_schedule(
    session_id: Id<GameSession>,
    battle: &Battle,
    command_id: Option<Id<GameCommand>>,
    schedule_if_ready: bool,
) -> Result<BattleRoundReadinessSummary, ApiError> {
    if battle.state != "active" {
        return Ok(BattleRoundReadinessSummary {
            ready_count: 0,
            participant_count: 0,
            all_ready: false,
            changed_subjects: Vec::new(),
        });
    }

    let session = sessions::load_session(session_id)?.ok_or_else(|| {
        public_error(
            "session_not_found",
            "session was not found while recomputing battle readiness",
            true,
        )
    })?;
    let battle_id_text = battle.id().to_string();
    let state = battle_runtime::with_runtime(&battle_id_text, |runtime| {
        (runtime.session_id == session.id().to_string()).then(|| runtime.state.clone())
    })
    .flatten()
    .map_or_else(
        || battle_rows::load_battle_state_from_row(&session, battle.clone()),
        Ok,
    )?;
    let mut changed_subjects = mark_auto_ready_participants(session_id, battle, &state)?;
    let participant_ids = alive_battle_participant_ids(&state, &battle.id().to_string())?;
    let ready_rows = battle_round_ready::page_battle_round_ready(
        session_id,
        battle.id(),
        battle.current_round,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    let ready_participant_ids = ready_rows
        .items
        .iter()
        .map(|row| row.participant_id)
        .collect::<BTreeSet<_>>();
    let all_ready = !participant_ids.is_empty()
        && participant_ids
            .iter()
            .all(|participant_id| ready_participant_ids.contains(participant_id));

    if all_ready && schedule_if_ready {
        let job = system_job_service::schedule_job(system_job_repo::SystemJobDraft {
            job_key: format!(
                "battle_round_advance:{}:{}",
                battle.id(),
                battle.current_round
            ),
            job_kind: "battle_round_advance".to_string(),
            session_id,
            battle_id: Some(battle.id()),
            turn_number: None,
            due_at: Timestamp::now(),
            command_id,
            cursor_json: None,
        })?;
        changed_subjects.push(command_response::changed(
            "system_job",
            &job.id().to_string(),
            "upsert",
        ));
    }

    Ok(BattleRoundReadinessSummary {
        ready_count: ready_rows.items.len(),
        participant_count: participant_ids.len(),
        all_ready,
        changed_subjects,
    })
}

fn recompute_runtime_battle_round_readiness_and_schedule(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    command_id: Option<Id<GameCommand>>,
    schedule_if_ready: bool,
    schedule_timeout: bool,
) -> Result<Option<BattleRoundReadinessSummary>, ApiError> {
    let battle_id_text = battle_id.to_string();
    let Some(runtime_summary) = battle_runtime::with_runtime_mut(&battle_id_text, |runtime| {
        if runtime.session_id != session_id.to_string() {
            return Ok(None);
        }

        let battle = runtime
            .state
            .battle(&battle_id_text)
            .map_err(map_battle_error)?
            .clone();
        if battle.state != "active" {
            return Ok(Some(RuntimeBattleRoundReadinessSummary {
                summary: BattleRoundReadinessSummary {
                    ready_count: 0,
                    participant_count: 0,
                    all_ready: false,
                    changed_subjects: Vec::new(),
                },
                round_job_key: None,
                timeout_deadline: None,
            }));
        }

        let participant_ids = alive_battle_participant_ids(&runtime.state, &battle_id_text)?;
        let mut changed_subjects = Vec::new();
        for participant_id_key in &participant_ids {
            let participant_id =
                Id::<domm_degens_schema::schema::GameParticipant>::from_key(*participant_id_key);
            let participant_text = participant_id.to_string();
            let ready_key = BattleRuntimeReadyKey {
                participant_id: participant_text.clone(),
                round_number: battle.current_round,
            };
            if runtime.ready_participants.contains(&ready_key) {
                continue;
            }
            let living_owned = runtime
                .state
                .stacks
                .iter()
                .filter(|stack| {
                    stack.battle_id == battle_id_text
                        && stack.owner_participant_id.as_deref() == Some(participant_text.as_str())
                        && stack.is_living()
                })
                .collect::<Vec<_>>();
            let all_acted = !living_owned.is_empty()
                && living_owned
                    .iter()
                    .all(|stack| stack.acted_round >= battle.current_round);
            let reason = if all_acted {
                Some("auto_all_stacks_acted")
            } else if !participant_has_meaningful_action(
                &runtime.state,
                &battle_id_text,
                &participant_text,
                battle.current_round,
            )? {
                Some("auto_no_actions")
            } else {
                None
            };
            if reason.is_none() {
                continue;
            }
            runtime.mark_ready(participant_text, battle.current_round);
            changed_subjects.push(command_response::changed(
                "battle_participant_round_ready",
                &format!(
                    "runtime:{battle_id}:{participant_id}:{}",
                    battle.current_round
                ),
                "upsert",
            ));
        }

        let ready_count = participant_ids
            .iter()
            .filter(|participant_id_key| {
                let participant_id = Id::<domm_degens_schema::schema::GameParticipant>::from_key(
                    **participant_id_key,
                )
                .to_string();
                runtime.ready_participants.contains(&BattleRuntimeReadyKey {
                    participant_id,
                    round_number: battle.current_round,
                })
            })
            .count();
        let all_ready = !participant_ids.is_empty() && ready_count == participant_ids.len();
        let round_job_key = (all_ready && schedule_if_ready).then(|| {
            format!(
                "battle_round_advance:{}:{}",
                battle_id, battle.current_round
            )
        });
        let timeout_deadline = (schedule_timeout && battle.state == "active")
            .then_some(battle.action_deadline_at)
            .flatten()
            .map(|deadline| (battle.created_turn, deadline));

        Ok(Some(RuntimeBattleRoundReadinessSummary {
            summary: BattleRoundReadinessSummary {
                ready_count,
                participant_count: participant_ids.len(),
                all_ready,
                changed_subjects,
            },
            round_job_key,
            timeout_deadline,
        }))
    }) else {
        return Ok(None);
    };

    let Some(mut runtime_summary) = runtime_summary? else {
        return Ok(None);
    };

    if let Some(round_job_key) = runtime_summary.round_job_key.clone() {
        let job = system_job_service::schedule_job(system_job_repo::SystemJobDraft {
            job_key: round_job_key.clone(),
            job_kind: "battle_round_advance".to_string(),
            session_id,
            battle_id: Some(battle_id),
            turn_number: None,
            due_at: Timestamp::now(),
            command_id,
            cursor_json: None,
        })?;
        battle_runtime::with_runtime_mut(&battle_id_text, |runtime| {
            runtime.deadline.round_job_key = Some(round_job_key);
            runtime.mark_dirty();
        });
        runtime_summary
            .summary
            .changed_subjects
            .push(command_response::changed(
                "system_job",
                &job.id().to_string(),
                "upsert",
            ));
    }

    if let Some((created_turn, deadline_ms)) = runtime_summary.timeout_deadline {
        schedule_battle_timeout_job_at(
            session_id,
            battle_id,
            created_turn,
            Timestamp::from_millis(i64::try_from(deadline_ms).unwrap_or(i64::MAX)),
        )?;
    }

    Ok(Some(runtime_summary.summary))
}

fn mark_auto_ready_participants(
    session_id: Id<GameSession>,
    battle: &Battle,
    state: &domm_game::BattleState,
) -> Result<Vec<domm_game::ChangedSubject>, ApiError> {
    let mut changed_subjects = Vec::new();
    let battle_id_text = battle.id().to_string();
    for participant_id_key in alive_battle_participant_ids(state, &battle_id_text)? {
        let participant_id =
            Id::<domm_degens_schema::schema::GameParticipant>::from_key(participant_id_key);
        if battle_round_ready::find_battle_round_ready(
            battle.id(),
            participant_id,
            battle.current_round,
        )?
        .is_some()
        {
            continue;
        }
        let participant_text = participant_id.to_string();
        let living_owned = state
            .stacks
            .iter()
            .filter(|stack| {
                stack.battle_id == battle_id_text
                    && stack.owner_participant_id.as_deref() == Some(participant_text.as_str())
                    && stack.is_living()
            })
            .collect::<Vec<_>>();
        let all_acted = !living_owned.is_empty()
            && living_owned
                .iter()
                .all(|stack| stack.acted_round >= battle.current_round);
        let reason = if all_acted {
            Some("auto_all_stacks_acted")
        } else if !participant_has_meaningful_action(
            state,
            &battle_id_text,
            &participant_text,
            battle.current_round,
        )? {
            Some("auto_no_actions")
        } else {
            None
        };
        let Some(reason) = reason else {
            continue;
        };
        let ready = battle_round_ready::mark_battle_round_ready(
            session_id,
            battle.id(),
            participant_id,
            battle.current_round,
            None,
            reason.to_string(),
            Timestamp::now(),
        )?;
        changed_subjects.push(command_response::changed(
            "battle_participant_round_ready",
            &ready.id().to_string(),
            "upsert",
        ));
    }
    Ok(changed_subjects)
}

fn alive_battle_participant_ids(
    state: &domm_game::BattleState,
    battle_id: &str,
) -> Result<BTreeSet<Ulid>, ApiError> {
    let mut participant_ids = BTreeSet::new();
    for stack in state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.is_living())
    {
        let Some(participant_id_text) = stack.owner_participant_id.as_deref() else {
            continue;
        };
        let participant_id = session_context::parse_id::<
            domm_degens_schema::schema::GameParticipant,
        >(participant_id_text, "participant_id")?;
        participant_ids.insert(participant_id.key());
    }
    Ok(participant_ids)
}

fn participant_has_meaningful_action(
    state: &domm_game::BattleState,
    battle_id: &str,
    participant_id: &str,
    round_number: u16,
) -> Result<bool, ApiError> {
    for stack in state.stacks.iter().filter(|stack| {
        stack.battle_id == battle_id
            && stack.owner_participant_id.as_deref() == Some(participant_id)
            && stack.is_living()
            && stack.acted_round < round_number
    }) {
        let actions = legal_actions_for_stack(state, battle_id, &stack.battle_stack_id)
            .map_err(map_battle_error)?;
        if actions.iter().any(|action| {
            action.enabled
                && matches!(
                    action.action.as_str(),
                    "Move" | "MeleeAttack" | "RangedAttack" | "Attack" | "CastAbility"
                )
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_battle_round_accepts_new_action(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    participant_id: Id<domm_degens_schema::schema::GameParticipant>,
    round_number: u16,
) -> Result<(), ApiError> {
    let participant_id_text = participant_id.to_string();
    if let Some(handled) = battle_runtime::with_runtime(&battle_id.to_string(), |runtime| {
        if runtime.session_id != session_id.to_string() {
            return None;
        }
        let runtime_round = match runtime.state.battle(&battle_id.to_string()) {
            Ok(battle) => battle.current_round,
            Err(error) => return Some(Err(map_battle_error(error))),
        };
        if runtime
            .ready_participants
            .contains(&BattleRuntimeReadyKey {
                participant_id: participant_id_text,
                round_number: runtime_round,
            })
        {
            return Some(Err(public_error(
                "battle_round_closed",
                "this participant has already ended the current battle round",
                false,
            )));
        }
        let round_job_key = format!("battle_round_advance:{battle_id}:{runtime_round}");
        if runtime.deadline.round_job_key.as_deref() == Some(round_job_key.as_str()) {
            return Some(Err(public_error(
                "battle_processing",
                "the current battle round is advancing; refresh before submitting another battle action",
                true,
            )));
        }
        Some(Ok(()))
    })
    .flatten()
    {
        return handled;
    }

    if battle_round_ready::find_battle_round_ready(battle_id, participant_id, round_number)?
        .is_some()
    {
        return Err(public_error(
            "battle_round_closed",
            "this participant has already ended the current battle round",
            false,
        ));
    }
    if battle_round_processing(battle_id, round_number)? {
        return Err(public_error(
            "battle_processing",
            "the current battle round is advancing; refresh before submitting another battle action",
            true,
        ));
    }
    Ok(())
}

fn battle_round_processing(battle_id: Id<Battle>, round_number: u16) -> Result<bool, ApiError> {
    let now = Timestamp::now();
    let job_key = format!("battle_round_advance:{battle_id}:{round_number}");
    Ok(
        system_job_repo::find_system_job_by_key(&job_key)?.is_some_and(|job| {
            job.job_kind == "battle_round_advance"
                && (job.status == system_job_repo::STATUS_RUNNING
                    || (job.status == system_job_repo::STATUS_SCHEDULED && job.due_at <= now))
        }),
    )
}

fn battle_round_from_job_key(job_key: &str) -> Option<u16> {
    job_key.rsplit(':').next()?.parse().ok()
}

fn apply_player_action(
    session: &mut GameSession,
    participant_id: &str,
    mut command: GameCommand,
    input: &BattleActionInput,
    now_ms: u64,
    response_participant_id: &str,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<BattleActionReceipt, ApiError> {
    command.status = "applying".to_string();
    command.phase = "applying".to_string();
    command =
        crate::metrics::benchmark_phase("submit_battle_action", "mark_command_applying", || {
            commands_events_effects::update_game_command(command)
        })?;

    if input.action != "CastAbility"
        && let Some(runtime) =
            crate::metrics::benchmark_phase("submit_battle_action", "load_battle_state", || {
                battle_runtime::with_runtime(&input.battle_id, Clone::clone)
            })
    {
        return apply_player_action_from_runtime(
            session,
            participant_id,
            command,
            input,
            now_ms,
            response_participant_id,
            events,
            changed_subjects,
            runtime,
        );
    }

    let mut state =
        crate::metrics::benchmark_phase("submit_battle_action", "load_battle_state", || {
            battle_rows::load_battle_state(session, &input.battle_id)
        })?;
    crate::metrics::benchmark_phase("submit_battle_action", "validate_action", || {
        validate_player_action(
            &state,
            participant_id,
            input,
            now_ms,
            CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS,
        )
    })?;
    if input.action == "CastAbility" {
        return apply_cast_ability_command(
            session,
            participant_id,
            command,
            input,
            response_participant_id,
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
    crate::metrics::benchmark_phase("submit_battle_action", "apply_rules", || {
        domm_game::apply_battle_command_by_id(&mut state, &command.id().to_string(), now_ms)
            .map_err(map_battle_error)
    })?;
    crate::metrics::benchmark_phase("submit_battle_action", "persist_battle_state", || {
        battle_rows::persist_battle_state(&state, command.id())
    })?;
    crate::metrics::benchmark_phase("submit_battle_action", "event_fanout", || {
        append_new_battle_events(
            session,
            command.id(),
            &state,
            Some(response_participant_id),
            events,
        )?;
        mirror_battle_runtime_from_state(session, &state)
    })?;
    changed_subjects.push(command_response::changed(
        "battle",
        &input.battle_id,
        "update",
    ));

    battle_action_receipt(&state, &command.id().to_string()).map_err(map_battle_error)
}

#[allow(clippy::too_many_arguments)]
fn apply_player_action_from_runtime(
    session: &mut GameSession,
    participant_id: &str,
    command: GameCommand,
    input: &BattleActionInput,
    now_ms: u64,
    response_participant_id: &str,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
    runtime: BattleRuntime,
) -> Result<BattleActionReceipt, ApiError> {
    let command_context = RuntimeBattleCommandContext {
        command_id: command.id().to_string(),
        client_nonce_text: command.client_nonce.to_string(),
        client_nonce: command.client_nonce,
        payload_hash: command.payload_hash.clone(),
        created_at_ms: command.created_at.as_millis().try_into().unwrap_or(0),
    };
    apply_player_action_from_runtime_parts(
        session,
        participant_id,
        &command_context,
        input,
        now_ms,
        response_participant_id,
        events,
        changed_subjects,
        runtime,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_player_action_from_runtime_parts(
    session: &mut GameSession,
    participant_id: &str,
    command: &RuntimeBattleCommandContext,
    input: &BattleActionInput,
    now_ms: u64,
    response_participant_id: &str,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
    mut runtime: BattleRuntime,
) -> Result<BattleActionReceipt, ApiError> {
    crate::metrics::benchmark_phase("submit_battle_action", "validate_action", || {
        validate_player_action(
            &runtime.state,
            participant_id,
            input,
            now_ms,
            CANISTER_BATTLE_ACTION_SUBMIT_GRACE_MS,
        )
    })?;
    let battle_command = battle_rows::battle_action_command_from_parts(
        &command.command_id,
        command.client_nonce.to_string(),
        command.payload_hash.clone(),
        command.created_at_ms,
        &input.battle_id,
        Some(participant_id.to_string()),
        input.battle_stack_id.clone(),
        input.action.clone(),
        input.target_stack_id.clone(),
        input.destination,
        false,
    );
    runtime.state.commands.push(battle_command);
    crate::metrics::benchmark_phase("submit_battle_action", "apply_rules", || {
        domm_game::apply_battle_command_by_id(&mut runtime.state, &command.command_id, now_ms)
            .map_err(map_battle_error)
    })?;
    crate::metrics::benchmark_phase("submit_battle_action", "persist_battle_state", || {
        Ok::<(), ApiError>(())
    })?;
    crate::metrics::benchmark_phase("submit_battle_action", "event_fanout", || {
        append_new_runtime_battle_events(
            session,
            &command.command_id,
            &mut runtime,
            Some(response_participant_id),
            events,
        )
    })?;
    refresh_runtime_metadata_from_state(&mut runtime, session, &input.battle_id)?;
    let receipt =
        battle_action_receipt(&runtime.state, &command.command_id).map_err(map_battle_error)?;
    trim_runtime_transient_battle_history(&mut runtime);
    battle_runtime::insert_runtime(runtime);
    changed_subjects.push(command_response::changed(
        "battle",
        &input.battle_id,
        "update",
    ));
    Ok(receipt)
}

fn trim_runtime_transient_battle_history(runtime: &mut BattleRuntime) {
    trim_to_recent(
        &mut runtime.state.commands,
        CANISTER_RUNTIME_BATTLE_TRANSIENT_HISTORY_LIMIT,
    );
    trim_to_recent(
        &mut runtime.state.events,
        CANISTER_RUNTIME_BATTLE_TRANSIENT_HISTORY_LIMIT,
    );
}

fn trim_to_recent<T>(values: &mut Vec<T>, limit: usize) {
    if values.len() <= limit {
        return;
    }
    let keep_from = values.len().saturating_sub(limit);
    values.drain(0..keep_from);
}

fn refresh_runtime_metadata_from_state(
    runtime: &mut BattleRuntime,
    session: &GameSession,
    battle_id: &str,
) -> Result<(), ApiError> {
    let battle = runtime.state.battle(battle_id).map_err(map_battle_error)?;
    runtime.deadline.action_deadline_at_ms = battle.action_deadline_at;
    runtime.deadline.timeout_job_key = battle
        .action_deadline_at
        .map(|deadline| format!("battle_timeout:{battle_id}:{deadline}"));
    runtime.session_event_sequence_cursor = session.next_event_seq;
    runtime.mark_dirty();
    Ok(())
}

fn apply_cast_ability_command(
    session: &mut GameSession,
    participant_id: &str,
    command: GameCommand,
    input: &BattleActionInput,
    response_participant_id: &str,
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
    append_new_battle_events(
        session,
        command.id(),
        &state,
        Some(response_participant_id),
        events,
    )?;
    append_cast_action_feed_events(
        session,
        command.id(),
        &state,
        input,
        response_participant_id,
        events,
    )?;
    mirror_battle_runtime_from_state(session, &state)?;
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
    response_participant_id: Option<&str>,
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
                        response_participant_id.unwrap_or(&participant_id),
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
                    append_new_battle_events(
                        session,
                        command.id(),
                        &state,
                        response_participant_id,
                        events,
                    )?;
                    mirror_battle_runtime_from_state(session, &state)?;
                    apply_resolved_battle_aftermath_with_runtime_projection(
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
                append_new_battle_events(
                    session,
                    command.id(),
                    &state,
                    response_participant_id,
                    events,
                )?;
                mirror_battle_runtime_from_state(session, &state)?;
                apply_resolved_battle_aftermath_with_runtime_projection(
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
            "battle_round_auto_defend" => {
                let round = parse_round_auto_defend_payload(&command.payload_json)?;
                let mut state = battle_rows::load_battle_state_from_row(
                    session,
                    battles::load_battle(battle_id)?.ok_or_else(|| {
                        public_error("battle_not_found", "battle not found during recovery", true)
                    })?,
                )?;
                let battle_command = battle_rows::battle_action_command(
                    &command,
                    &battle_id.to_string(),
                    None,
                    round.stack_id,
                    "RoundAutoDefend".to_string(),
                    None,
                    None,
                    true,
                );
                state.commands.push(battle_command);
                domm_game::apply_battle_command_by_id(
                    &mut state,
                    &command.id().to_string(),
                    command.created_at.as_millis().try_into().unwrap_or(0),
                )
                .map_err(map_battle_error)?;
                battle_rows::persist_battle_state(&state, command.id())?;
                append_new_battle_events(
                    session,
                    command.id(),
                    &state,
                    response_participant_id,
                    events,
                )?;
                mirror_battle_runtime_from_state(session, &state)?;
                apply_resolved_battle_aftermath_with_runtime_projection(
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
                    "battle_round_auto_defend",
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

#[allow(clippy::too_many_arguments)]
fn apply_due_runtime_timeouts_for_sync(
    session: &mut GameSession,
    battle_id: Id<Battle>,
    now_ms: u64,
    max_timeout_actions: u32,
    command_id: Id<GameCommand>,
    _response_participant_id: Option<&str>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<Option<(bool, u32)>, ApiError> {
    let battle_id_text = battle_id.to_string();
    let Some(mut runtime) = battle_runtime::with_runtime(&battle_id_text, Clone::clone) else {
        return Ok(None);
    };
    if runtime.session_id != session.id().to_string() {
        return Ok(None);
    }

    let mut applied = 0_u32;
    let mut needs_aftermath = false;
    loop {
        let battle = runtime
            .state
            .battle(&battle_id_text)
            .map_err(map_battle_error)?
            .clone();
        if battle.state != "active" {
            needs_aftermath = true;
            break;
        }
        let Some(deadline) = battle.action_deadline_at else {
            break;
        };
        if deadline > now_ms {
            break;
        }
        let Some(active_stack_id) = battle.active_stack_id else {
            break;
        };
        if applied >= max_timeout_actions {
            battle_runtime::insert_runtime(runtime);
            return Ok(Some((true, applied)));
        }

        let timeout_command_id = command_id.to_string();
        if runtime
            .state
            .events
            .iter()
            .any(|event| event.command_id == timeout_command_id)
        {
            break;
        }
        runtime.state.commands.push(domm_game::BattleCommandRecord {
            command_id: timeout_command_id.clone(),
            battle_id: battle_id_text.clone(),
            actor_participant_id: None,
            battle_stack_id: Some(active_stack_id),
            client_nonce: format!("timeout:{battle_id_text}:{deadline}"),
            payload_hash: "runtime_timeout_auto_defend".to_string(),
            action: "AutoDefend".to_string(),
            target_stack_id: None,
            destination: None,
            system: true,
            status: "applying".to_string(),
            created_at: deadline,
            applied_at: None,
            retryable_error: None,
        });
        domm_game::apply_battle_command_by_id(&mut runtime.state, &timeout_command_id, deadline)
            .map_err(map_battle_error)?;
        battle_rows::persist_battle_header_from_state(&runtime.state, command_id)?;
        append_runtime_timeout_public_event(session, command_id, &battle_id_text, events)?;
        refresh_runtime_metadata_from_state(&mut runtime, session, &battle_id_text)?;
        trim_runtime_transient_battle_history(&mut runtime);
        applied = applied.saturating_add(1);
    }

    battle_runtime::insert_runtime(runtime);
    if needs_aftermath {
        apply_resolved_battle_aftermath_with_runtime_projection(
            session,
            command_id,
            battle_id,
            events,
            changed_subjects,
        )?;
    }
    if applied > 0 {
        changed_subjects.push(command_response::changed(
            "battle",
            &battle_id_text,
            "timeout",
        ));
    }
    Ok(Some((false, applied)))
}

fn append_runtime_timeout_public_event(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle_id: &str,
    events: &mut Vec<domm_game::ApiEventView>,
) -> Result<(), ApiError> {
    let public_event = command_response::append_public_event(
        session,
        command_id,
        format!("battle:{battle_id}:runtime_timeout:{command_id}:public"),
        "battle_timeout_auto_defend".to_string(),
        Some("battle".to_string()),
        Some(battle_id.to_string()),
        format!(
            r#"{{"battle_id":"{}","event_type":"battle_timeout_auto_defend","redacted":true}}"#,
            command_response::escape_json(battle_id)
        ),
    )?;
    events.push(public_event);
    Ok(())
}

fn apply_due_timeouts(
    session: &mut GameSession,
    battle_id: Id<Battle>,
    now_ms: u64,
    max_timeout_actions: u32,
    response_participant_id: Option<&str>,
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
            response_participant_id,
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

#[allow(clippy::too_many_arguments)]
fn apply_system_battle_action_from_runtime(
    session: &mut GameSession,
    command: &GameCommand,
    battle_id: Id<Battle>,
    stack_id: String,
    action: &str,
    deadline_base_ms: u64,
    response_participant_id: Option<&str>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
    mut runtime: BattleRuntime,
    changed_action: &str,
) -> Result<(), ApiError> {
    let battle_id_text = battle_id.to_string();
    let battle_command = battle_rows::battle_action_command(
        command,
        &battle_id_text,
        None,
        stack_id,
        action.to_string(),
        None,
        None,
        true,
    );
    runtime.state.commands.push(battle_command);
    domm_game::apply_battle_command_by_id(
        &mut runtime.state,
        &command.id().to_string(),
        deadline_base_ms,
    )
    .map_err(map_battle_error)?;
    battle_rows::persist_battle_header_from_state(&runtime.state, command.id())?;
    let command_id_text = command.id().to_string();
    append_new_runtime_battle_events(
        session,
        &command_id_text,
        &mut runtime,
        response_participant_id,
        events,
    )?;
    refresh_runtime_metadata_from_state(&mut runtime, session, &battle_id_text)?;
    trim_runtime_transient_battle_history(&mut runtime);
    battle_runtime::insert_runtime(runtime);
    apply_resolved_battle_aftermath_with_runtime_projection(
        session,
        command.id(),
        battle_id,
        events,
        changed_subjects,
    )?;
    changed_subjects.push(command_response::changed(
        "battle",
        &battle_id_text,
        changed_action,
    ));
    Ok(())
}

fn apply_timeout_command(
    session: &mut GameSession,
    mut command: GameCommand,
    battle_id: Id<Battle>,
    active_stack_id: String,
    deadline_ms: u64,
    response_participant_id: Option<&str>,
    events: &mut Vec<domm_game::ApiEventView>,
    changed_subjects: &mut Vec<domm_game::ChangedSubject>,
) -> Result<(), ApiError> {
    if command.status == "applied" {
        return Ok(());
    }
    command.status = "applying".to_string();
    command.phase = "applying".to_string();
    command = commands_events_effects::update_game_command(command)?;

    if let Some(runtime) = battle_runtime::with_runtime(&battle_id.to_string(), Clone::clone) {
        apply_system_battle_action_from_runtime(
            session,
            &command,
            battle_id,
            active_stack_id,
            "AutoDefend",
            deadline_ms,
            response_participant_id,
            events,
            changed_subjects,
            runtime,
            "timeout",
        )?;
    } else {
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
        append_new_battle_events(
            session,
            command.id(),
            &state,
            response_participant_id,
            events,
        )?;
        mirror_battle_runtime_from_state(session, &state)?;
        apply_resolved_battle_aftermath_with_runtime_projection(
            session,
            command.id(),
            battle_id,
            events,
            changed_subjects,
        )?;
        changed_subjects.push(command_response::changed(
            "battle",
            &battle_id.to_string(),
            "timeout",
        ));
    }
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(command_response::result_json(
        "battle_timeout_auto_defend",
        session.current_turn,
    ));
    command.applied_at = Some(Timestamp::now());
    commands_events_effects::update_game_command(command)?;
    Ok(())
}

fn begin_round_auto_defend_command(
    session: &GameSession,
    battle_id: Id<Battle>,
    stack_id: &str,
    round_number: u16,
) -> Result<GameCommand, ApiError> {
    let command_type = "battle_round_auto_defend";
    let actor_key = format!("battle_round:{battle_id}:{round_number}");
    let nonce_text = format!("round_auto_defend:{battle_id}:{round_number}:{stack_id}");
    let client_nonce = command_response::nonce_u64(command_type, &nonce_text);
    if let Some(command) = commands_events_effects::find_game_command_by_idempotency(
        session.id(),
        "system",
        &actor_key,
        client_nonce,
    )? {
        return Ok(command);
    }
    let payload = format!(
        r#"{{"battle_id":"{}","stack_id":"{}","round_number":{round_number}}}"#,
        battle_id,
        command_response::escape_json(stack_id)
    );
    let hash = command_response::payload_hash(command_type, &actor_key, &nonce_text, &payload);
    commands_events_effects::create_game_command(
        session.id(),
        "system".to_string(),
        actor_key,
        None,
        None,
        None,
        session.current_turn,
        client_nonce,
        command_type.to_string(),
        hash,
        payload,
    )
}

fn apply_round_auto_defend_command(
    session: &mut GameSession,
    mut command: GameCommand,
    battle_id: Id<Battle>,
    stack_id: String,
    round_number: u16,
    deadline_base_ms: u64,
    response_participant_id: Option<&str>,
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
            "battle not found during round advance",
            true,
        )
    })?;
    if battle.current_round != round_number {
        command.status = "applied".to_string();
        command.phase = "complete".to_string();
        command.result_json = Some(command_response::result_json(
            "battle_round_auto_defend",
            session.current_turn,
        ));
        command.applied_at = Some(Timestamp::now());
        commands_events_effects::update_game_command(command)?;
        return Ok(());
    }
    if let Some(runtime) = battle_runtime::with_runtime(&battle_id.to_string(), Clone::clone) {
        apply_system_battle_action_from_runtime(
            session,
            &command,
            battle_id,
            stack_id.clone(),
            "RoundAutoDefend",
            deadline_base_ms,
            response_participant_id,
            events,
            changed_subjects,
            runtime,
            "round_auto_defend",
        )?;
    } else {
        let mut state = battle_rows::load_battle_state_from_row(session, battle)?;
        let battle_command = battle_rows::battle_action_command(
            &command,
            &battle_id.to_string(),
            None,
            stack_id.clone(),
            "RoundAutoDefend".to_string(),
            None,
            None,
            true,
        );
        state.commands.push(battle_command);
        domm_game::apply_battle_command_by_id(
            &mut state,
            &command.id().to_string(),
            deadline_base_ms,
        )
        .map_err(map_battle_error)?;
        battle_rows::persist_battle_state(&state, command.id())?;
        append_new_battle_events(
            session,
            command.id(),
            &state,
            response_participant_id,
            events,
        )?;
        mirror_battle_runtime_from_state(session, &state)?;
        apply_resolved_battle_aftermath_with_runtime_projection(
            session,
            command.id(),
            battle_id,
            events,
            changed_subjects,
        )?;
        changed_subjects.push(command_response::changed(
            "battle",
            &battle_id.to_string(),
            "round_auto_defend",
        ));
    }
    command.status = "applied".to_string();
    command.phase = "complete".to_string();
    command.result_json = Some(command_response::result_json(
        "battle_round_auto_defend",
        session.current_turn,
    ));
    command.applied_at = Some(Timestamp::now());
    commands_events_effects::update_game_command(command)?;
    Ok(())
}

fn validate_player_action(
    state: &domm_game::BattleState,
    participant_id: &str,
    input: &BattleActionInput,
    now_ms: u64,
    deadline_grace_ms: u64,
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
        .is_some_and(|deadline| now_ms > deadline.saturating_add(deadline_grace_ms))
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
    if !stack.is_living() || stack.acted_round >= battle.current_round {
        return Err(public_error(
            "battle_action_not_legal",
            format!("battle action is not legal: {}", input.action),
            false,
        ));
    }
    match input.action.as_str() {
        "Move" => {
            let Some(destination) = input.destination else {
                return Err(public_error(
                    "battle_destination_required",
                    "move action requires a destination",
                    false,
                ));
            };
            let legal_actions =
                legal_actions_for_stack(state, &input.battle_id, &input.battle_stack_id)
                    .map_err(map_battle_error)?;
            let Some(action) = legal_actions
                .iter()
                .find(|action| action.action == "Move" && action.enabled)
            else {
                return Err(public_error(
                    "battle_action_not_legal",
                    "battle action is not legal: Move",
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
            validate_attack_action_without_reachability(state, stack, input)?;
        }
        "Defend" => {}
        "Wait" => {
            if stack.waited_round >= battle.current_round {
                return Err(public_error(
                    "battle_action_not_legal",
                    "battle action is not legal: Wait",
                    false,
                ));
            }
        }
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

fn validate_attack_action_without_reachability(
    state: &domm_game::BattleState,
    stack: &BattleStackRecord,
    input: &BattleActionInput,
) -> Result<(), ApiError> {
    let Some(target_id) = input.target_stack_id.as_deref() else {
        return Err(public_error(
            "battle_target_required",
            "attack action requires a target stack",
            false,
        ));
    };
    let target = state.stack(target_id).map_err(map_battle_error)?;
    if target.battle_id != input.battle_id || target.side == stack.side || !target.is_living() {
        return Err(public_error(
            "battle_target_not_legal",
            "attack target is not legal",
            false,
        ));
    }

    let distance =
        stack.battle_x.abs_diff(target.battle_x) + stack.battle_y.abs_diff(target.battle_y);
    match input.action.as_str() {
        "MeleeAttack" => {
            if distance != 1 {
                return Err(public_error(
                    "battle_target_not_legal",
                    "melee target must be adjacent",
                    false,
                ));
            }
        }
        "RangedAttack" => validate_ranged_attack_without_reachability(state, stack, distance)?,
        "Attack" => {
            if distance == 1 {
                return Ok(());
            }
            validate_ranged_attack_without_reachability(state, stack, distance)?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_ranged_attack_without_reachability(
    state: &domm_game::BattleState,
    stack: &BattleStackRecord,
    distance: u8,
) -> Result<(), ApiError> {
    if !stack.ranged || distance <= 1 || stack.shots_remaining == 0 {
        return Err(public_error(
            "battle_target_not_legal",
            "ranged target is not legal",
            false,
        ));
    }
    let adjacent_enemy = state.stacks.iter().any(|candidate| {
        candidate.battle_id == stack.battle_id
            && candidate.side != stack.side
            && candidate.is_living()
            && stack.battle_x.abs_diff(candidate.battle_x)
                + stack.battle_y.abs_diff(candidate.battle_y)
                == 1
    });
    if adjacent_enemy {
        return Err(public_error(
            "battle_target_not_legal",
            "ranged attack is blocked by an adjacent enemy",
            false,
        ));
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

fn append_cast_action_feed_events(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    state: &domm_game::BattleState,
    input: &BattleActionInput,
    response_participant_id: &str,
    events: &mut Vec<domm_game::ApiEventView>,
) -> Result<(), ApiError> {
    let action_payload = format!(
        r#"{{"action":"{}","stack_id":"{}","ability_key":{},"target_stack_id":{}}}"#,
        command_response::escape_json(&input.action),
        command_response::escape_json(&input.battle_stack_id),
        input
            .ability_key
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        input
            .target_stack_id
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string())
    );
    let detailed_payload = format!(
        r#"{{"battle_id":"{}","subject_id_text":"{}","payload":{}}}"#,
        command_response::escape_json(&input.battle_id),
        command_response::escape_json(&input.battle_stack_id),
        action_payload
    );
    for participant_id in involved_battle_participant_ids(state, &input.battle_id)? {
        let audience_key = format!("participant:{participant_id}");
        let view = command_response::append_event_for_audience(
            session,
            command_id,
            format!(
                "battle:{}:{}:action_applied:{}",
                input.battle_id, command_id, audience_key
            ),
            audience_key,
            "battle_action_applied".to_string(),
            Some("battle".to_string()),
            Some(input.battle_id.clone()),
            detailed_payload.clone(),
        )?;
        if response_participant_id == participant_id {
            events.push(view);
        }
    }
    let public_event = command_response::append_public_event(
        session,
        command_id,
        format!(
            "battle:{}:{}:action_applied:public",
            input.battle_id, command_id
        ),
        "battle_action_applied".to_string(),
        Some("battle".to_string()),
        Some(input.battle_id.clone()),
        format!(
            r#"{{"battle_id":"{}","event_type":"battle_action_applied","redacted":true}}"#,
            command_response::escape_json(&input.battle_id)
        ),
    )?;
    events.push(public_event);
    Ok(())
}

fn append_new_battle_events(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    state: &domm_game::BattleState,
    response_participant_id: Option<&str>,
    events: &mut Vec<domm_game::ApiEventView>,
) -> Result<(), ApiError> {
    for event in state
        .events
        .iter()
        .filter(|event| event.command_id == command_id.to_string())
    {
        let detailed_payload = format!(
            r#"{{"battle_id":"{}","subject_id_text":"{}","payload":{}}}"#,
            command_response::escape_json(&event.battle_id),
            command_response::escape_json(&event.subject_id_text),
            json_string(&event.payload)
        );
        for participant_id in involved_battle_participant_ids(state, &event.battle_id)? {
            let audience_key = format!("participant:{participant_id}");
            let view = command_response::append_event_for_audience(
                session,
                command_id,
                format!(
                    "battle:{}:{}:{}",
                    event.battle_id, event.event_key, audience_key
                ),
                audience_key,
                event.event_type.clone(),
                Some("battle".to_string()),
                Some(event.battle_id.clone()),
                detailed_payload.clone(),
            )?;
            if response_participant_id == Some(participant_id.as_str()) {
                events.push(view);
            }
        }
        let public_event = command_response::append_public_event(
            session,
            command_id,
            format!("battle:{}:{}:public", event.battle_id, event.event_key),
            event.event_type.clone(),
            Some("battle".to_string()),
            Some(event.battle_id.clone()),
            format!(
                r#"{{"battle_id":"{}","event_type":"{}","redacted":true}}"#,
                command_response::escape_json(&event.battle_id),
                command_response::escape_json(&event.event_type)
            ),
        )?;
        events.push(public_event);
    }
    Ok(())
}

fn append_new_runtime_battle_events(
    session: &mut GameSession,
    command_id: &str,
    runtime: &mut BattleRuntime,
    response_participant_id: Option<&str>,
    events: &mut Vec<domm_game::ApiEventView>,
) -> Result<(), ApiError> {
    let command_id_text = command_id.to_string();
    let new_events = runtime
        .state
        .events
        .iter()
        .filter(|event| event.command_id == command_id_text)
        .cloned()
        .collect::<Vec<_>>();
    for event in new_events {
        let detailed_payload = format!(
            r#"{{"battle_id":"{}","subject_id_text":"{}","payload":{}}}"#,
            command_response::escape_json(&event.battle_id),
            command_response::escape_json(&event.subject_id_text),
            json_string(&event.payload)
        );
        for participant_id in runtime_involved_battle_participant_ids(runtime, &event.battle_id) {
            let audience_key = format!("participant:{participant_id}");
            let view = runtime_battle_event_view(
                session,
                format!(
                    "battle:{}:{}:{}",
                    event.battle_id, event.event_key, audience_key
                ),
                audience_key,
                event.event_type.clone(),
                Some("battle".to_string()),
                Some(event.battle_id.clone()),
                detailed_payload.clone(),
            )?;
            runtime.push_event(BattleRuntimeEvent {
                command_id: Some(command_id_text.clone()),
                event: view.clone(),
                flushed: false,
            });
            if response_participant_id == Some(participant_id.as_str()) {
                events.push(view);
            }
        }
        let public_event = runtime_battle_event_view(
            session,
            format!("battle:{}:{}:public", event.battle_id, event.event_key),
            "public".to_string(),
            event.event_type.clone(),
            Some("battle".to_string()),
            Some(event.battle_id.clone()),
            format!(
                r#"{{"battle_id":"{}","event_type":"{}","redacted":true}}"#,
                command_response::escape_json(&event.battle_id),
                command_response::escape_json(&event.event_type)
            ),
        )?;
        runtime.push_event(BattleRuntimeEvent {
            command_id: Some(command_id_text.clone()),
            event: public_event.clone(),
            flushed: false,
        });
        events.push(public_event);
    }
    Ok(())
}

fn runtime_battle_event_view(
    session: &mut GameSession,
    event_key: String,
    audience_key: String,
    event_type: String,
    subject_kind: Option<String>,
    subject_id_text: Option<String>,
    payload_json: String,
) -> Result<domm_game::ApiEventView, ApiError> {
    let event_seq = battle_runtime::reserve_session_event_seq(session)?;
    Ok(domm_game::ApiEventView {
        session_id: session.id().to_string(),
        event_seq,
        event_key,
        audience_key,
        turn_number: session.current_turn,
        event_type,
        subject_kind,
        subject_id_text,
        payload: Some(payload_json),
        redacted: false,
    })
}

fn runtime_involved_battle_participant_ids(
    runtime: &BattleRuntime,
    battle_id: &str,
) -> BTreeSet<String> {
    let mut participant_ids = runtime
        .state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id)
        .filter_map(|stack| stack.owner_participant_id.clone())
        .collect::<BTreeSet<_>>();
    participant_ids.extend(runtime.participant_audience_keys.keys().cloned());
    participant_ids
}

fn involved_battle_participant_ids(
    state: &domm_game::BattleState,
    battle_id: &str,
) -> Result<BTreeSet<String>, ApiError> {
    let mut participant_ids = state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id)
        .filter_map(|stack| stack.owner_participant_id.clone())
        .collect::<BTreeSet<_>>();
    let battle = state.battle(battle_id).map_err(map_battle_error)?;
    if let Some(champion_id) = battle.attacker_champion_id.as_deref() {
        add_champion_owner_participant(&mut participant_ids, champion_id)?;
    }
    if let Some(champion_id) = battle.defender_champion_id.as_deref() {
        add_champion_owner_participant(&mut participant_ids, champion_id)?;
    }
    if let Some(town_id) = battle.defender_town_id.as_deref() {
        add_town_owner_participant(&mut participant_ids, town_id)?;
    }
    Ok(participant_ids)
}

fn add_champion_owner_participant(
    participant_ids: &mut BTreeSet<String>,
    champion_id: &str,
) -> Result<(), ApiError> {
    let champion_id = session_context::parse_id::<Champion>(champion_id, "champion_id")?;
    if let Some(champion) = champions_artifacts::load_champion(champion_id)? {
        participant_ids
            .insert(Id::<GameParticipant>::from_key(champion.participant_id).to_string());
    }
    Ok(())
}

fn add_town_owner_participant(
    participant_ids: &mut BTreeSet<String>,
    town_id: &str,
) -> Result<(), ApiError> {
    let town_id = session_context::parse_id::<Town>(town_id, "town_id")?;
    if let Some(town) = towns::load_town(town_id)?
        && let Some(owner_participant_id) = town.owner_participant_id
    {
        participant_ids.insert(Id::<GameParticipant>::from_key(owner_participant_id).to_string());
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

struct RoundAutoDefendPayload {
    stack_id: String,
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

fn parse_round_auto_defend_payload(payload: &str) -> Result<RoundAutoDefendPayload, ApiError> {
    Ok(RoundAutoDefendPayload {
        stack_id: json_field(payload, "stack_id").ok_or_else(|| {
            public_error(
                "invalid_battle_payload",
                "round auto-defend payload is missing stack_id",
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

fn partial_retry_at() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now()
            .as_millis()
            .saturating_add(SYSTEM_JOB_PARTIAL_RETRY_DELAY_MS),
    )
}
