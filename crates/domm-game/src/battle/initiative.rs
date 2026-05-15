use crate::rng::{RollKey, hash64};

use super::types::{BattleError, BattleInitiativeEntry, BattleState};

pub fn initiative_order(
    state: &BattleState,
    battle_id: &str,
) -> Result<Vec<BattleInitiativeEntry>, BattleError> {
    let battle = state.battle(battle_id)?;
    let mut normal = Vec::new();
    let mut waited = Vec::new();

    for stack in state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.is_living())
    {
        if stack.acted_round >= battle.current_round {
            continue;
        }
        let entry = BattleInitiativeEntry {
            battle_stack_id: stack.battle_stack_id.clone(),
            side: stack.side.clone(),
            initiative: stack.initiative,
            speed: stack.speed,
            waited: stack.waited_round >= battle.current_round,
            tie_breaker: hash64(&RollKey::new(
                state.session_seed.clone(),
                "battle_initiative_tie",
                u32::from(battle.current_round),
                &battle.battle_id,
                &stack.battle_stack_id,
                &stack.side,
                0,
            )),
        };
        if entry.waited {
            waited.push(entry);
        } else {
            normal.push(entry);
        }
    }

    normal.sort_by(|left, right| {
        right
            .initiative
            .cmp(&left.initiative)
            .then_with(|| right.speed.cmp(&left.speed))
            .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
    });
    waited.sort_by(|left, right| {
        left.initiative
            .cmp(&right.initiative)
            .then_with(|| left.speed.cmp(&right.speed))
            .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
    });
    normal.extend(waited);
    Ok(normal)
}

pub fn select_active_stack_id(
    state: &BattleState,
    battle_id: &str,
) -> Result<Option<String>, BattleError> {
    Ok(initiative_order(state, battle_id)?
        .first()
        .map(|entry| entry.battle_stack_id.clone()))
}
