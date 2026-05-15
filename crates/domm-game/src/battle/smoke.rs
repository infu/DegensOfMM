use super::actions::{apply_stack_attack, legal_actions_for_stack};
use super::build::build_first_playable_battle_state;
use super::types::{BATTLE_SIDE_DEFENDER, BattleError, BattleSmokeView};

pub fn run_first_playable_battle_smoke() -> Result<BattleSmokeView, BattleError> {
    let mut state = build_first_playable_battle_state()?;
    let battle_id = state
        .battles
        .first()
        .expect("first playable battle fixture has one battle")
        .battle_id
        .clone();
    let active_stack_id = state
        .battle(&battle_id)?
        .active_stack_id
        .clone()
        .expect("first playable battle has an active stack");
    let target_stack_id = state
        .stacks
        .iter()
        .find(|stack| stack.battle_id == battle_id && stack.side == BATTLE_SIDE_DEFENDER)
        .expect("first playable battle has a defender")
        .battle_stack_id
        .clone();
    let legal_action_count =
        legal_actions_for_stack(&state, &battle_id, &active_stack_id)?.len() as u32;
    let first_damage = apply_stack_attack(
        &mut state,
        &battle_id,
        &active_stack_id,
        &target_stack_id,
        "command:fixture:battle:first-shot",
        0,
    )?;
    Ok(BattleSmokeView {
        battle_id,
        active_stack_id,
        stack_count: state.stacks.len() as u32,
        obstacle_count: state.obstacles.len() as u32,
        legal_action_count,
        first_damage,
    })
}
