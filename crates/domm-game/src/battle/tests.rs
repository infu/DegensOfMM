use super::actions::{
    apply_damage_to_stack, apply_stack_attack, legal_actions_for_stack, v1_morale_luck_policy,
    validate_battle_stack_status_keys,
};
use super::build::build_first_playable_battle_state;
use super::commands::{battle_action_payload_hash, submit_battle_action, sync_battle};
use super::initiative::initiative_order;
use super::occupancy::{repair_stack_position_from_occupancy, validate_battle_occupancy};
use super::smoke::run_first_playable_battle_smoke;
use super::types::{
    BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER,
    BattleCommandBudget, BattleCommandRecord, BattleCoord, BattleError,
};
use super::view::battle_view_for_participant;
use crate::fixtures::first_playable_fixture;

#[test]
fn first_playable_battle_fixture_creates_rows_view_and_active_stack() {
    let state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle = state.battles.first().expect("fixture has one battle");

    assert_eq!(battle.grid_width, BATTLE_GRID_WIDTH);
    assert_eq!(battle.grid_height, BATTLE_GRID_HEIGHT);
    assert_eq!(battle.battle_type, "neutral");
    assert_eq!(state.stacks.len(), 3);
    assert_eq!(state.obstacles.len(), 2);
    assert!(state.stacks.iter().all(|stack| stack.readiness == 0));
    assert!(
        battle
            .active_stack_id
            .as_deref()
            .is_some_and(|id| id.ends_with(":attacker:1"))
    );
    validate_battle_occupancy(&state, &battle.battle_id).expect("occupancy is valid");

    let fixture = first_playable_fixture();
    let view = battle_view_for_participant(
        &state,
        &battle.battle_id,
        &fixture.ids.participant_one_id,
        480_000,
    )
    .expect("view should build");
    assert_eq!(view.stacks.len(), 3);
    assert_eq!(view.obstacles.len(), 2);
    assert_eq!(view.initiative_order.len(), 3);
    assert!(
        view.legal_actions_for_caller
            .iter()
            .any(|action| action.action == "RangedAttack" && action.enabled)
    );
    assert!(!view.morale_luck_policy.morale_enabled);
}

#[test]
fn initiative_order_uses_speed_and_seeded_ties() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();
    for stack in state
        .stacks
        .iter_mut()
        .filter(|stack| stack.side == BATTLE_SIDE_ATTACKER)
    {
        stack.initiative = 8;
        stack.speed = 4;
    }

    let first = initiative_order(&state, &battle_id).expect("initiative should sort");
    let second = initiative_order(&state, &battle_id).expect("initiative should be stable");
    assert_eq!(first, second);

    let attacker_entries = first
        .iter()
        .filter(|entry| entry.side == BATTLE_SIDE_ATTACKER)
        .collect::<Vec<_>>();
    assert_eq!(attacker_entries.len(), 2);
    assert!(attacker_entries[0].tie_breaker <= attacker_entries[1].tie_breaker);
}

#[test]
fn occupancy_rejects_duplicates_and_repairs_cached_stack_coords() {
    let mut duplicate = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = duplicate.battles[0].battle_id.clone();
    let first_coord = BattleCoord::new(
        duplicate.occupancy[0].battle_x,
        duplicate.occupancy[0].battle_y,
    );
    duplicate.occupancy[1].battle_x = first_coord.x;
    duplicate.occupancy[1].battle_y = first_coord.y;
    let duplicate_error =
        validate_battle_occupancy(&duplicate, &battle_id).expect_err("duplicate tile should fail");
    assert!(matches!(
        duplicate_error,
        BattleError::DuplicateTileOccupancy { .. }
    ));

    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();
    let stack_id = state.stacks[0].battle_stack_id.clone();
    state.stack_mut(&stack_id).unwrap().battle_x = 9;
    assert!(matches!(
        validate_battle_occupancy(&state, &battle_id),
        Err(BattleError::OccupancyCacheMismatch { .. })
    ));
    repair_stack_position_from_occupancy(&mut state, &stack_id)
        .expect("repair should use occupancy");
    validate_battle_occupancy(&state, &battle_id).expect("repaired occupancy is valid");
}

#[test]
fn legal_moves_and_attacks_respect_obstacles_occupancy_and_range() {
    let state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();
    let active_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();
    let actions = legal_actions_for_stack(&state, &battle_id, &active_stack_id)
        .expect("legal actions should build");

    let move_action = actions
        .iter()
        .find(|action| action.action == "Move")
        .expect("move action exists");
    assert!(move_action.enabled);
    assert!(!move_action.path.contains(&BattleCoord::new(5, 4)));
    assert!(!move_action.path.contains(&BattleCoord::new(10, 4)));

    let melee = actions
        .iter()
        .find(|action| action.action == "MeleeAttack")
        .expect("melee action exists");
    assert!(!melee.enabled);
    assert_eq!(melee.disabled_reason.as_deref(), Some("no_adjacent_enemy"));

    let ranged = actions
        .iter()
        .find(|action| action.action == "RangedAttack")
        .expect("ranged action exists");
    assert!(ranged.enabled);
    assert_eq!(ranged.targets.len(), 1);
    assert!(ranged.damage_preview.is_some());
}

#[test]
fn damage_rolls_are_deterministic_and_apply_stack_health_rules() {
    let mut first = build_first_playable_battle_state().expect("battle fixture should build");
    let mut second = first.clone();
    let battle_id = first.battles[0].battle_id.clone();
    let attacker_id = first
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();
    let target_id = first
        .stacks
        .iter()
        .find(|stack| stack.side == BATTLE_SIDE_DEFENDER)
        .unwrap()
        .battle_stack_id
        .clone();

    let first_outcome = apply_stack_attack(
        &mut first,
        &battle_id,
        &attacker_id,
        &target_id,
        "command:test:shot",
        0,
    )
    .expect("shot should resolve");
    let second_outcome = apply_stack_attack(
        &mut second,
        &battle_id,
        &attacker_id,
        &target_id,
        "command:test:shot",
        0,
    )
    .expect("shot should be deterministic");

    assert_eq!(first_outcome, second_outcome);
    assert_eq!(first_outcome.roll_audit.domain_key, "battle_damage");
    assert!(first_outcome.final_damage > 0);
    assert!(first_outcome.killed >= 3);
    assert!(first.stack(&target_id).unwrap().quantity < 12);
    assert_eq!(first.stack(&attacker_id).unwrap().shots_remaining, 7);
}

#[test]
fn lethal_damage_defeats_stack_and_removes_occupancy() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();
    let target_id = state
        .stacks
        .iter()
        .find(|stack| stack.side == BATTLE_SIDE_DEFENDER)
        .unwrap()
        .battle_stack_id
        .clone();

    let (killed, quantity_after, front_hp_after) =
        apply_damage_to_stack(&mut state, &target_id, 10_000, "command:test:lethal")
            .expect("damage should apply");
    assert_eq!(killed, 12);
    assert_eq!(quantity_after, 0);
    assert_eq!(front_hp_after, 0);
    assert_eq!(state.stack(&target_id).unwrap().status, "defeated");
    assert!(
        state
            .occupancy
            .iter()
            .all(|occupancy| occupancy.battle_stack_id != target_id)
    );
    validate_battle_occupancy(&state, &battle_id).expect("dead stacks do not require occupancy");
}

#[test]
fn morale_luck_policy_is_explicitly_disabled_and_status_keys_are_capped() {
    let policy = v1_morale_luck_policy();
    assert!(!policy.morale_enabled);
    assert_eq!(
        policy.morale_disabled_reason.as_deref(),
        Some("morale_disabled_v1")
    );
    assert!(!policy.luck_enabled);
    assert_eq!(
        policy.luck_disabled_reason.as_deref(),
        Some("luck_disabled_v1")
    );

    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    state.stacks[0].status_keys = (0..9).map(|index| format!("status:{index}")).collect();
    assert!(matches!(
        validate_battle_stack_status_keys(&state.stacks[0]),
        Err(BattleError::Effect(_))
    ));
}

#[test]
fn first_playable_battle_smoke_resolves_first_damage() {
    let smoke = run_first_playable_battle_smoke().expect("battle smoke should pass");
    assert_eq!(smoke.stack_count, 3);
    assert_eq!(smoke.obstacle_count, 2);
    assert!(smoke.legal_action_count >= 5);
    assert!(smoke.first_damage.final_damage > 0);
}

#[test]
fn submit_battle_action_dedupes_and_rejects_payload_mismatch() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let fixture = first_playable_fixture();
    let battle_id = state.battles[0].battle_id.clone();
    let active_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();

    let first = submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "nonce:defend:1",
        480_000,
    )
    .expect("defend should apply");
    let duplicate = submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "nonce:defend:1",
        480_001,
    )
    .expect("duplicate should replay");
    assert_eq!(first.command_id, duplicate.command_id);
    assert_eq!(first.event_seq, Some(1));
    assert_eq!(state.events.len(), 1);

    let mismatch = submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &active_stack_id,
        "Wait",
        None,
        None,
        "nonce:defend:1",
        480_002,
    )
    .expect_err("same nonce with different payload should fail");
    assert!(matches!(
        mismatch,
        BattleError::DuplicateCommandPayloadMismatch { .. }
    ));
}

#[test]
fn timeout_auto_defend_wins_after_deadline_and_is_idempotent() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let fixture = first_playable_fixture();
    let battle_id = state.battles[0].battle_id.clone();
    let expired_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();

    let sync = sync_battle(
        &mut state,
        &battle_id,
        510_000,
        BattleCommandBudget::default(),
    )
    .expect("timeout sync should apply");
    assert_eq!(sync.timeout_actions_applied, 1);
    assert_eq!(state.events[0].event_type, "battle_timeout_auto_defend");
    assert_eq!(state.stack(&expired_stack_id).unwrap().acted_round, 1);

    let replay = sync_battle(
        &mut state,
        &battle_id,
        510_001,
        BattleCommandBudget::default(),
    )
    .expect("timeout replay should not duplicate");
    assert_eq!(replay.timeout_actions_applied, 0);
    assert_eq!(state.events.len(), 1);

    let late_action = submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &expired_stack_id,
        "Defend",
        None,
        None,
        "nonce:late",
        510_001,
    )
    .expect_err("old active stack lost its deadline race");
    assert!(matches!(late_action, BattleError::StackNotActive { .. }));
}

#[test]
fn player_action_before_deadline_prevents_timeout_for_that_stack() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let fixture = first_playable_fixture();
    let battle_id = state.battles[0].battle_id.clone();
    let active_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();

    submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "nonce:deadline-race",
        509_999,
    )
    .expect("player action before deadline should apply");
    let sync = sync_battle(
        &mut state,
        &battle_id,
        520_000,
        BattleCommandBudget::default(),
    )
    .expect("sync after player action should not timeout next stack yet");
    assert_eq!(sync.timeout_actions_applied, 0);
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].event_type, "battle_action_applied");
}

#[test]
fn sync_battle_respects_timeout_budget() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();

    let sync = sync_battle(
        &mut state,
        &battle_id,
        700_000,
        BattleCommandBudget {
            max_recoveries: 8,
            max_timeout_actions: 1,
        },
    )
    .expect("bounded timeout sync should return partial progress");
    assert_eq!(sync.timeout_actions_applied, 1);
    assert!(sync.battle_sync_incomplete);
    assert_eq!(state.events.len(), 1);
}

#[test]
fn recovery_finishes_applying_command_before_validating_new_action() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let fixture = first_playable_fixture();
    let battle_id = state.battles[0].battle_id.clone();
    let active_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();
    let pending_command_id = "command:test:recover:defend".to_string();
    state.commands.push(BattleCommandRecord {
        command_id: pending_command_id.clone(),
        battle_id: battle_id.clone(),
        actor_participant_id: Some(fixture.ids.participant_one_id.clone()),
        battle_stack_id: Some(active_stack_id.clone()),
        client_nonce: "nonce:recover".to_string(),
        payload_hash: battle_action_payload_hash("Defend", None, None),
        action: "Defend".to_string(),
        target_stack_id: None,
        destination: None,
        system: false,
        status: "applying".to_string(),
        created_at: 480_000,
        applied_at: None,
        retryable_error: None,
    });

    let rejected = submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "nonce:new",
        480_001,
    )
    .expect_err("new action should be validated after recovery changes active stack");
    assert!(matches!(rejected, BattleError::StackNotActive { .. }));
    let recovered = state
        .commands
        .iter()
        .find(|command| command.command_id == pending_command_id)
        .expect("pending command remains recorded");
    assert_eq!(recovered.status, "applied");
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].command_id, pending_command_id);
}

#[test]
fn battle_events_are_ordered_across_player_actions() {
    let mut state = build_first_playable_battle_state().expect("battle fixture should build");
    let fixture = first_playable_fixture();
    let battle_id = state.battles[0].battle_id.clone();
    let first_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();
    submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &first_stack_id,
        "Defend",
        None,
        None,
        "nonce:event:1",
        480_000,
    )
    .expect("first action should apply");
    let second_stack_id = state
        .battle(&battle_id)
        .unwrap()
        .active_stack_id
        .clone()
        .unwrap();
    submit_battle_action(
        &mut state,
        &battle_id,
        &fixture.ids.participant_one_id,
        &second_stack_id,
        "Defend",
        None,
        None,
        "nonce:event:2",
        481_000,
    )
    .expect("second action should apply");

    assert_eq!(state.events.len(), 2);
    assert_eq!(state.events[0].event_seq, 1);
    assert_eq!(state.events[1].event_seq, 2);
    assert_ne!(state.events[0].event_key, state.events[1].event_key);
}
