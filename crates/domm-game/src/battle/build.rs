use crate::champion::{ChampionArmyStackRecord, build_first_playable_champion_state};
use crate::content::{UnitContent, first_playable_content_manifest};
use crate::effects::validate_status_keys;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::neutral::{NeutralArmyStackRecord, build_first_playable_neutral_state};
use crate::rng::{RollKey, hash64};

use super::initiative::select_active_stack_id;
use super::occupancy::validate_battle_occupancy;
use super::types::{
    BATTLE_ACTION_DEADLINE_MS, BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_MAX_ROUNDS,
    BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER, BattleCoord, BattleError, BattleObstacleRecord,
    BattleOccupancyRecord, BattleRecord, BattleStackRecord, BattleState,
};

pub fn build_first_playable_battle_state() -> Result<BattleState, BattleError> {
    let fixture = first_playable_fixture();
    let manifest = first_playable_content_manifest();
    let champions = build_first_playable_champion_state();
    let west_champion = champions
        .champions
        .iter()
        .find(|champion| champion.champion_id == "champion:west")
        .expect("first playable west champion exists")
        .clone();
    let neutral = build_first_playable_neutral_state();
    let battle_id = format!(
        "battle:{}:8:champion:west:neutral:west-mine",
        fixture.ids.session_id
    );
    let turn_seed = hash64(&RollKey::new(
        fixture.scenario_seed.clone(),
        "battle_turn_seed",
        8,
        &battle_id,
        "champion:west",
        "neutral:west-mine",
        0,
    ));

    let battle = BattleRecord {
        battle_id: battle_id.clone(),
        session_id: fixture.ids.session_id.clone(),
        state: "active".to_string(),
        battle_type: "neutral".to_string(),
        attacker_champion_id: Some("champion:west".to_string()),
        defender_champion_id: None,
        defender_town_id: None,
        defender_neutral_army_id: Some("neutral:west-mine".to_string()),
        current_round: 1,
        active_side: BATTLE_SIDE_ATTACKER.to_string(),
        active_stack_id: None,
        grid_width: BATTLE_GRID_WIDTH,
        grid_height: BATTLE_GRID_HEIGHT,
        max_rounds: BATTLE_MAX_ROUNDS,
        turn_seed,
        winner_participant_id: None,
        created_turn: 8,
        action_deadline_at: Some((TURN_DURATION_MS * 8) + BATTLE_ACTION_DEADLINE_MS),
        resolved_at: None,
        cleanup_after_turn: 0,
        last_command_id: Some("command:fixture:start-neutral-battle".to_string()),
    };

    let mut stacks = Vec::new();
    for stack in champions
        .army_stacks
        .iter()
        .filter(|stack| stack.champion_id == "champion:west")
    {
        let deployment_y = match stack.slot_index {
            0 => 3,
            1 => 6,
            value => 2_u8.saturating_add(value),
        };
        stacks.push(stack_from_champion(
            &battle_id,
            &fixture.ids.participant_one_id,
            stack,
            west_champion.might,
            west_champion.guard,
            &manifest
                .unit(&stack.unit_slug)
                .ok_or_else(|| BattleError::UnitNotFound {
                    unit_slug: stack.unit_slug.clone(),
                })?
                .clone(),
            BattleCoord::new(1, deployment_y),
        )?);
    }
    for stack in neutral
        .stacks
        .iter()
        .filter(|stack| stack.neutral_army_id == "neutral:west-mine")
    {
        stacks.push(stack_from_neutral(
            &battle_id,
            stack,
            &manifest
                .unit(&stack.unit_slug)
                .ok_or_else(|| BattleError::UnitNotFound {
                    unit_slug: stack.unit_slug.clone(),
                })?
                .clone(),
            BattleCoord::new(10, 4_u8.saturating_add(stack.slot_index)),
        )?);
    }

    let obstacles = vec![
        obstacle(&battle_id, "rubble", 5, 4),
        obstacle(&battle_id, "broken-cart", 6, 5),
    ];
    let occupancy = stacks
        .iter()
        .map(|stack| BattleOccupancyRecord {
            battle_occupancy_id: format!("battle-occupancy:{}", stack.battle_stack_id),
            battle_id: battle_id.clone(),
            battle_stack_id: stack.battle_stack_id.clone(),
            battle_x: stack.battle_x,
            battle_y: stack.battle_y,
            last_command_id: Some("command:fixture:start-neutral-battle".to_string()),
        })
        .collect::<Vec<_>>();

    let mut state = BattleState {
        session_seed: fixture.scenario_seed,
        battles: vec![battle],
        stacks,
        obstacles,
        occupancy,
    };
    validate_battle_occupancy(&state, &battle_id)?;
    let active_stack_id =
        select_active_stack_id(&state, &battle_id)?.ok_or_else(|| BattleError::StackNotFound {
            battle_stack_id: "first active stack".to_string(),
        })?;
    let active_side = state.stack(&active_stack_id)?.side.clone();
    let battle = state.battle_mut(&battle_id)?;
    battle.active_stack_id = Some(active_stack_id);
    battle.active_side = active_side;
    Ok(state)
}

fn stack_from_champion(
    battle_id: &str,
    owner_participant_id: &str,
    source: &ChampionArmyStackRecord,
    champion_might: i16,
    champion_guard: i16,
    unit: &UnitContent,
    coord: BattleCoord,
) -> Result<BattleStackRecord, BattleError> {
    stack_from_unit(
        battle_id,
        unit,
        Some(owner_participant_id.to_string()),
        BATTLE_SIDE_ATTACKER,
        source.slot_index,
        "champion_army",
        Some(source.stack_id.clone()),
        source.slot_index,
        champion_might,
        champion_guard,
        source.quantity,
        source.front_hp,
        coord,
        &[],
    )
}

fn stack_from_neutral(
    battle_id: &str,
    source: &NeutralArmyStackRecord,
    unit: &UnitContent,
    coord: BattleCoord,
) -> Result<BattleStackRecord, BattleError> {
    stack_from_unit(
        battle_id,
        unit,
        None,
        BATTLE_SIDE_DEFENDER,
        source.slot_index,
        "neutral_army",
        Some(source.stack_id.clone()),
        source.slot_index,
        0,
        0,
        source.quantity,
        source.front_hp,
        coord,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn stack_from_unit(
    battle_id: &str,
    unit: &UnitContent,
    owner_participant_id: Option<String>,
    side: &str,
    slot_index: u8,
    origin_kind: &str,
    origin_stack_id_text: Option<String>,
    origin_slot_index: u8,
    champion_might: i16,
    champion_guard: i16,
    quantity: u32,
    front_hp: u16,
    coord: BattleCoord,
    status_keys: &[String],
) -> Result<BattleStackRecord, BattleError> {
    validate_status_keys(status_keys)?;
    Ok(BattleStackRecord {
        battle_stack_id: format!("battle-stack:{battle_id}:{side}:{slot_index}"),
        battle_id: battle_id.to_string(),
        unit_id: unit.id.clone(),
        owner_participant_id,
        side: side.to_string(),
        slot_index,
        origin_kind: origin_kind.to_string(),
        origin_stack_id_text,
        origin_slot_index,
        champion_might,
        champion_guard,
        attack: unit.attack,
        defense: unit.defense,
        damage_min: unit.damage_min,
        damage_max: unit.damage_max,
        max_hp: unit.max_hp,
        speed: unit.speed,
        initiative: unit.initiative,
        ranged: unit.ranged,
        flying: unit.flying,
        quantity,
        front_hp,
        shots_remaining: unit.shots,
        battle_x: coord.x,
        battle_y: coord.y,
        readiness: 0,
        acted_round: 0,
        retaliated_round: 0,
        defended_round: 0,
        waited_round: 0,
        cast_round: 0,
        status: "active".to_string(),
        last_command_id: Some("command:fixture:start-neutral-battle".to_string()),
        status_keys: status_keys.to_vec(),
    })
}

fn obstacle(battle_id: &str, obstacle_type: &str, x: u8, y: u8) -> BattleObstacleRecord {
    BattleObstacleRecord {
        battle_obstacle_id: format!("battle-obstacle:{battle_id}:{x}:{y}"),
        battle_id: battle_id.to_string(),
        obstacle_type: obstacle_type.to_string(),
        battle_x: x,
        battle_y: y,
        width: 1,
        height: 1,
        hp: 0,
        state: "active".to_string(),
        last_command_id: Some("command:fixture:start-neutral-battle".to_string()),
    }
}
