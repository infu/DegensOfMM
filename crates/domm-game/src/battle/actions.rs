use std::collections::{HashSet, VecDeque};

use crate::effects::{EffectDomain, EffectRequest, dispatch_effect, validate_status_keys};
use crate::rng::RollKey;

use super::occupancy::{
    adjacent_coords, is_obstacle_blocked, is_tile_open, occupant_at, validate_coord,
};
use super::types::{
    BattleCoord, BattleDamageOutcome, BattleError, BattleMoraleLuckPolicy, BattleStackRecord,
    BattleState, DamagePreview, LegalBattleAction,
};

pub fn legal_actions_for_stack(
    state: &BattleState,
    battle_id: &str,
    battle_stack_id: &str,
) -> Result<Vec<LegalBattleAction>, BattleError> {
    let battle = state.battle(battle_id)?;
    let stack = state.stack(battle_stack_id)?;
    let reachable = reachable_tiles(state, battle_id, battle_stack_id)?;
    let adjacent_enemies = adjacent_enemy_stack_ids(state, battle_id, stack);
    let enemies = enemy_stacks(state, battle_id, stack);

    let mut actions = Vec::new();
    actions.push(LegalBattleAction {
        action: "Move".to_string(),
        ability_key: None,
        enabled: !reachable.is_empty(),
        disabled_reason: if reachable.is_empty() {
            Some("no_reachable_tile".to_string())
        } else {
            None
        },
        targets: Vec::new(),
        path: reachable,
        damage_preview: None,
    });
    actions.push(LegalBattleAction {
        action: "MeleeAttack".to_string(),
        ability_key: None,
        enabled: !adjacent_enemies.is_empty(),
        disabled_reason: if adjacent_enemies.is_empty() {
            Some("no_adjacent_enemy".to_string())
        } else {
            None
        },
        targets: adjacent_enemies,
        path: Vec::new(),
        damage_preview: enemies
            .iter()
            .find(|target| stack.coord().manhattan(target.coord()) == 1)
            .map(|target| damage_preview(stack, target)),
    });

    let non_adjacent_targets = enemies
        .iter()
        .filter(|target| stack.coord().manhattan(target.coord()) > 1)
        .map(|target| target.battle_stack_id.clone())
        .collect::<Vec<_>>();
    let ranged_disabled = ranged_disabled_reason(state, battle_id, stack);
    actions.push(LegalBattleAction {
        action: "RangedAttack".to_string(),
        ability_key: None,
        enabled: ranged_disabled.is_none() && !non_adjacent_targets.is_empty(),
        disabled_reason: ranged_disabled.or_else(|| {
            if non_adjacent_targets.is_empty() {
                Some("no_non_adjacent_enemy".to_string())
            } else {
                None
            }
        }),
        targets: non_adjacent_targets,
        path: Vec::new(),
        damage_preview: enemies
            .iter()
            .find(|target| stack.coord().manhattan(target.coord()) > 1)
            .map(|target| damage_preview(stack, target)),
    });
    actions.push(LegalBattleAction {
        action: "Defend".to_string(),
        ability_key: None,
        enabled: stack.is_living() && stack.acted_round < battle.current_round,
        disabled_reason: None,
        targets: Vec::new(),
        path: Vec::new(),
        damage_preview: None,
    });
    actions.push(LegalBattleAction {
        action: "Wait".to_string(),
        ability_key: None,
        enabled: stack.is_living()
            && stack.acted_round < battle.current_round
            && stack.waited_round < battle.current_round,
        disabled_reason: None,
        targets: Vec::new(),
        path: Vec::new(),
        damage_preview: None,
    });
    actions.push(LegalBattleAction {
        action: "CastAbility".to_string(),
        ability_key: None,
        enabled: false,
        disabled_reason: Some("no_learned_battle_spell".to_string()),
        targets: Vec::new(),
        path: Vec::new(),
        damage_preview: None,
    });
    actions.push(LegalBattleAction {
        action: "Retreat".to_string(),
        ability_key: None,
        enabled: false,
        disabled_reason: Some("retreat_deferred_v1_no_rehire_flow".to_string()),
        targets: Vec::new(),
        path: Vec::new(),
        damage_preview: None,
    });
    actions.push(LegalBattleAction {
        action: "Surrender".to_string(),
        ability_key: None,
        enabled: false,
        disabled_reason: Some("surrender_deferred_v1_no_payment_terms".to_string()),
        targets: Vec::new(),
        path: Vec::new(),
        damage_preview: None,
    });
    Ok(actions)
}

pub fn reachable_tiles(
    state: &BattleState,
    battle_id: &str,
    battle_stack_id: &str,
) -> Result<Vec<BattleCoord>, BattleError> {
    let battle = state.battle(battle_id)?;
    let stack = state.stack(battle_stack_id)?;
    let start = stack.coord();
    let mut queue = VecDeque::from([(start, 0_u8)]);
    let mut visited = HashSet::from([start]);
    let mut reachable = Vec::new();

    while let Some((coord, distance)) = queue.pop_front() {
        if distance >= stack.speed {
            continue;
        }
        for next in adjacent_coords(battle.grid_width, battle.grid_height, coord) {
            if !visited.insert(next) {
                continue;
            }
            if !can_enter_tile(state, battle_id, next, stack.flying) {
                continue;
            }
            reachable.push(next);
            queue.push_back((next, distance + 1));
        }
    }
    reachable.sort_by(|left, right| left.y.cmp(&right.y).then_with(|| left.x.cmp(&right.x)));
    Ok(reachable)
}

pub fn apply_stack_attack(
    state: &mut BattleState,
    battle_id: &str,
    attacker_stack_id: &str,
    target_stack_id: &str,
    command_id: &str,
    roll_index: u32,
) -> Result<BattleDamageOutcome, BattleError> {
    let battle = state.battle(battle_id)?.clone();
    let attacker = state.stack(attacker_stack_id)?.clone();
    let target = state.stack(target_stack_id)?.clone();
    validate_attack_target(state, battle_id, &attacker, &target)?;

    let adjacent_to_target = attacker.coord().manhattan(target.coord()) == 1;
    if attacker.ranged && !adjacent_to_target {
        if adjacent_enemy_stack_ids(state, battle_id, &attacker).is_empty() {
            if attacker.shots_remaining == 0 {
                return Err(BattleError::NoShotsRemaining {
                    battle_stack_id: attacker_stack_id.to_string(),
                });
            }
        } else {
            return Err(BattleError::RangedBlockedByAdjacentEnemy {
                battle_stack_id: attacker_stack_id.to_string(),
            });
        }
    } else if !adjacent_to_target {
        return Err(BattleError::TargetNotAdjacent {
            target_stack_id: target_stack_id.to_string(),
        });
    }

    let roll = RollKey::new(
        state.session_seed.clone(),
        "battle_damage",
        u32::from(battle.current_round),
        command_id,
        attacker_stack_id,
        target_stack_id,
        roll_index,
    )
    .roll_between_inclusive(
        u64::from(attacker.damage_min),
        u64::from(attacker.damage_max),
    )?;
    let rolled_damage_per_unit = roll.value as u16;
    let final_damage = damage_for_roll(&attacker, &target, rolled_damage_per_unit);
    let (killed, quantity_after, front_hp_after) =
        apply_damage_to_stack(state, target_stack_id, final_damage, command_id)?;

    let attacker_after = state.stack_mut(attacker_stack_id)?;
    if attacker_after.ranged && !adjacent_to_target {
        attacker_after.shots_remaining = attacker_after.shots_remaining.saturating_sub(1);
    }
    attacker_after.last_command_id = Some(command_id.to_string());

    Ok(BattleDamageOutcome {
        attacker_stack_id: attacker_stack_id.to_string(),
        target_stack_id: target_stack_id.to_string(),
        rolled_damage_per_unit,
        final_damage,
        killed,
        target_quantity_after: quantity_after,
        target_front_hp_after: front_hp_after,
        roll_audit: roll.audit(),
    })
}

pub fn apply_damage_to_stack(
    state: &mut BattleState,
    battle_stack_id: &str,
    damage: u32,
    command_id: &str,
) -> Result<(u32, u32, u16), BattleError> {
    let before = state.stack(battle_stack_id)?.clone();
    let total_hp_before = total_stack_hp(before.quantity, before.front_hp, before.max_hp);
    let total_hp_after = total_hp_before.saturating_sub(damage);
    let (quantity_after, front_hp_after) = stack_hp_from_total(total_hp_after, before.max_hp);
    let killed = before.quantity.saturating_sub(quantity_after);

    let stack = state.stack_mut(battle_stack_id)?;
    stack.quantity = quantity_after;
    stack.front_hp = front_hp_after;
    stack.last_command_id = Some(command_id.to_string());
    if quantity_after == 0 {
        stack.status = "defeated".to_string();
        state
            .occupancy
            .retain(|occupancy| occupancy.battle_stack_id != battle_stack_id);
    }

    Ok((killed, quantity_after, front_hp_after))
}

#[must_use]
pub fn damage_preview(attacker: &BattleStackRecord, target: &BattleStackRecord) -> DamagePreview {
    let min_damage = damage_for_roll(attacker, target, attacker.damage_min);
    let max_damage = damage_for_roll(attacker, target, attacker.damage_max);
    DamagePreview {
        target_stack_id: target.battle_stack_id.clone(),
        min_damage,
        max_damage,
        estimated_kills_min: estimate_kills(target, min_damage),
        estimated_kills_max: estimate_kills(target, max_damage),
    }
}

#[must_use]
pub fn v1_morale_luck_policy() -> BattleMoraleLuckPolicy {
    let morale = dispatch_effect(EffectRequest::new(EffectDomain::Morale, "morale"));
    let luck = dispatch_effect(EffectRequest::new(EffectDomain::Luck, "luck"));
    BattleMoraleLuckPolicy {
        morale_enabled: morale.supported,
        morale_disabled_reason: morale.disabled_reason,
        luck_enabled: luck.supported,
        luck_disabled_reason: luck.disabled_reason,
    }
}

pub fn validate_battle_stack_status_keys(stack: &BattleStackRecord) -> Result<(), BattleError> {
    validate_status_keys(&stack.status_keys)?;
    Ok(())
}

fn can_enter_tile(state: &BattleState, battle_id: &str, coord: BattleCoord, flying: bool) -> bool {
    if occupant_at(state, battle_id, coord).is_some() {
        return false;
    }
    if flying {
        validate_coord(
            super::types::BATTLE_GRID_WIDTH,
            super::types::BATTLE_GRID_HEIGHT,
            coord,
        )
        .is_ok()
    } else {
        is_tile_open(state, battle_id, coord)
    }
}

fn validate_attack_target(
    state: &BattleState,
    battle_id: &str,
    attacker: &BattleStackRecord,
    target: &BattleStackRecord,
) -> Result<(), BattleError> {
    if attacker.side == target.side {
        return Err(BattleError::TargetNotEnemy {
            target_stack_id: target.battle_stack_id.clone(),
        });
    }
    if target.battle_id != battle_id || !target.is_living() {
        return Err(BattleError::StackNotFound {
            battle_stack_id: target.battle_stack_id.clone(),
        });
    }
    if is_obstacle_blocked(state, battle_id, target.coord()) {
        return Err(BattleError::ObstacleBlocked {
            battle_id: battle_id.to_string(),
            x: target.battle_x,
            y: target.battle_y,
        });
    }
    Ok(())
}

fn ranged_disabled_reason(
    state: &BattleState,
    battle_id: &str,
    stack: &BattleStackRecord,
) -> Option<String> {
    if !stack.ranged {
        return Some("stack_not_ranged".to_string());
    }
    if stack.shots_remaining == 0 {
        return Some("no_shots_remaining".to_string());
    }
    if !adjacent_enemy_stack_ids(state, battle_id, stack).is_empty() {
        return Some("adjacent_enemy_blocks_ranged".to_string());
    }
    None
}

fn adjacent_enemy_stack_ids(
    state: &BattleState,
    battle_id: &str,
    stack: &BattleStackRecord,
) -> Vec<String> {
    enemy_stacks(state, battle_id, stack)
        .into_iter()
        .filter(|target| stack.coord().manhattan(target.coord()) == 1)
        .map(|target| target.battle_stack_id.clone())
        .collect()
}

fn enemy_stacks<'a>(
    state: &'a BattleState,
    battle_id: &str,
    stack: &BattleStackRecord,
) -> Vec<&'a BattleStackRecord> {
    state
        .stacks
        .iter()
        .filter(|target| {
            target.battle_id == battle_id
                && target.side != stack.side
                && target.is_living()
                && target.battle_stack_id != stack.battle_stack_id
        })
        .collect()
}

fn damage_for_roll(
    attacker: &BattleStackRecord,
    target: &BattleStackRecord,
    damage_per_unit: u16,
) -> u32 {
    let base_damage = u32::from(damage_per_unit).saturating_mul(attacker.quantity);
    let attack_bonus_bp = i32::from((attacker.attack - target.defense).max(0)).saturating_mul(500);
    let defense_penalty_bp =
        i32::from((target.defense - attacker.attack).max(0)).saturating_mul(400);
    let attack_bp = 10_000_i32.saturating_add(attack_bonus_bp);
    let defense_bp = 10_000_i32.saturating_sub(defense_penalty_bp).max(0);
    let champion_bp = 10_000_i32
        .saturating_add(i32::from(attacker.champion_might).saturating_mul(300))
        .saturating_sub(i32::from(target.champion_guard).saturating_mul(250))
        .max(0);
    let combined_bp = attack_bp
        .saturating_mul(defense_bp)
        .saturating_div(10_000)
        .saturating_mul(champion_bp)
        .saturating_div(10_000);
    let final_bp = combined_bp.clamp(2_500, 40_000) as u32;
    base_damage.saturating_mul(final_bp) / 10_000
}

fn estimate_kills(target: &BattleStackRecord, damage: u32) -> u32 {
    let before = target.quantity;
    let (after, _) = stack_hp_from_total(
        total_stack_hp(target.quantity, target.front_hp, target.max_hp).saturating_sub(damage),
        target.max_hp,
    );
    before.saturating_sub(after)
}

fn total_stack_hp(quantity: u32, front_hp: u16, max_hp: u16) -> u32 {
    if quantity == 0 {
        0
    } else {
        quantity
            .saturating_sub(1)
            .saturating_mul(u32::from(max_hp))
            .saturating_add(u32::from(front_hp))
    }
}

fn stack_hp_from_total(total_hp: u32, max_hp: u16) -> (u32, u16) {
    if total_hp == 0 {
        return (0, 0);
    }
    let max_hp = u32::from(max_hp);
    let quantity = total_hp.div_ceil(max_hp);
    let remainder = total_hp % max_hp;
    let front_hp = if remainder == 0 { max_hp } else { remainder };
    (quantity, front_hp as u16)
}
