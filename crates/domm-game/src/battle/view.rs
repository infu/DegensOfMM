use super::actions::{legal_actions_for_stack, v1_morale_luck_policy};
use super::initiative::initiative_order;
use super::types::{
    BattleError, BattleEventView, BattleGridView, BattleObstacleView, BattleStackView, BattleState,
    BattleView,
};

pub fn battle_view_for_participant(
    state: &BattleState,
    battle_id: &str,
    caller_participant_id: &str,
    now_ms: u64,
) -> Result<BattleView, BattleError> {
    let battle = state.battle(battle_id)?;
    let active_stack = battle
        .active_stack_id
        .as_deref()
        .and_then(|stack_id| state.stack(stack_id).ok());
    let active_participant_id = active_stack.and_then(|stack| stack.owner_participant_id.clone());
    let legal_actions_for_caller = match (battle.active_stack_id.as_deref(), active_stack) {
        (Some(stack_id), Some(stack))
            if stack.owner_participant_id.as_deref() == Some(caller_participant_id) =>
        {
            legal_actions_for_stack(state, battle_id, stack_id)?
        }
        _ => Vec::new(),
    };

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
        grid: BattleGridView {
            width: battle.grid_width,
            height: battle.grid_height,
        },
        obstacles: state
            .obstacles
            .iter()
            .filter(|obstacle| obstacle.battle_id == battle_id)
            .map(|obstacle| BattleObstacleView {
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
        stacks: state
            .stacks
            .iter()
            .filter(|stack| stack.battle_id == battle_id)
            .map(|stack| BattleStackView {
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
        initiative_order: initiative_order(state, battle_id)?
            .into_iter()
            .map(|entry| entry.battle_stack_id)
            .collect(),
        legal_actions_for_caller,
        events: Vec::<BattleEventView>::new(),
        next_event_seq: 0,
        morale_luck_policy: v1_morale_luck_policy(),
    })
}
