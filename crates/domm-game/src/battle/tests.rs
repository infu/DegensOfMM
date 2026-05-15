use super::actions::{
    apply_damage_to_stack, apply_stack_attack, legal_actions_for_stack, v1_morale_luck_policy,
    validate_battle_stack_status_keys,
};
use super::build::build_first_playable_battle_state;
use super::initiative::initiative_order;
use super::occupancy::{repair_stack_position_from_occupancy, validate_battle_occupancy};
use super::smoke::run_first_playable_battle_smoke;
use super::types::{
    BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER, BattleCoord,
    BattleError,
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
