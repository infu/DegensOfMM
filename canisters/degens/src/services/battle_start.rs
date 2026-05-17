use domm_degens_schema::schema::{
    Battle, BattleStack, Champion, GameCommand, GameParticipant, GameSession, Town, UnitDefinition,
};
use domm_game::{ApiError, MoveCoord};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::{battles, champions_artifacts, content, towns};

use super::{battle as battle_service, command_response, session_context::public_error};

pub(crate) fn start_champion_battle(
    session: &GameSession,
    command_id: Id<GameCommand>,
    attacker: &Champion,
    attacker_participant_id: Id<GameParticipant>,
    defender: &Champion,
    coord: MoveCoord,
) -> Result<Battle, ApiError> {
    if let Some(existing) = battles::find_battle_by_attacker(attacker.id())? {
        if existing.state == "active" && existing.defender_champion_id == Some(defender.id().key())
        {
            return Ok(existing);
        }
    }
    let mut battle = create_battle(
        session,
        command_id,
        "champion",
        Some(attacker.id()),
        Some(defender.id()),
        None,
        None,
        battle_seed(session, attacker, &defender.id().to_string(), coord),
    )?;
    let mut stacks = Vec::new();
    stacks.extend(create_champion_side_stacks(
        command_id,
        battle.id(),
        attacker.id(),
        Some(attacker_participant_id),
        "attacker",
        1,
    )?);
    stacks.extend(create_champion_side_stacks(
        command_id,
        battle.id(),
        defender.id(),
        Some(Id::<GameParticipant>::from_key(defender.participant_id)),
        "defender",
        domm_game::BATTLE_GRID_WIDTH - 2,
    )?);
    create_default_obstacles(command_id, battle.id())?;
    let battle = set_initial_active_stack(session, &mut battle, &mut stacks)?;
    battle_service::schedule_battle_timeout_job(session.id(), &battle)?;
    Ok(battle)
}

pub(crate) fn start_town_battle(
    session: &GameSession,
    command_id: Id<GameCommand>,
    attacker: &Champion,
    attacker_participant_id: Id<GameParticipant>,
    town: &Town,
    coord: MoveCoord,
) -> Result<Battle, ApiError> {
    if let Some(existing) = battles::find_battle_by_attacker(attacker.id())? {
        if existing.state == "active" && existing.defender_town_id == Some(town.id().key()) {
            return Ok(existing);
        }
    }
    let mut battle = create_battle(
        session,
        command_id,
        "town",
        Some(attacker.id()),
        None,
        Some(town.id()),
        None,
        battle_seed(session, attacker, &town.id().to_string(), coord),
    )?;
    let mut stacks = create_champion_side_stacks(
        command_id,
        battle.id(),
        attacker.id(),
        Some(attacker_participant_id),
        "attacker",
        1,
    )?;
    stacks.extend(create_town_defender_stacks(
        command_id,
        battle.id(),
        town.id(),
    )?);
    create_default_obstacles(command_id, battle.id())?;
    if stacks.iter().all(|stack| stack.side != "defender") {
        battle.state = "resolved".to_string();
        battle.winner_participant_id = Some(attacker_participant_id.key());
        battle.active_stack_id = None;
        battle.action_deadline_at = None;
        battle.resolved_at = Some(Timestamp::now());
        battles::update_battle(battle)
    } else {
        let battle = set_initial_active_stack(session, &mut battle, &mut stacks)?;
        battle_service::schedule_battle_timeout_job(session.id(), &battle)?;
        Ok(battle)
    }
}

#[allow(clippy::too_many_arguments)]
fn create_battle(
    session: &GameSession,
    command_id: Id<GameCommand>,
    battle_type: &str,
    attacker_champion_id: Option<Id<Champion>>,
    defender_champion_id: Option<Id<Champion>>,
    defender_town_id: Option<Id<Town>>,
    defender_neutral_army_id: Option<Id<domm_degens_schema::schema::NeutralArmy>>,
    seed: u64,
) -> Result<Battle, ApiError> {
    let action_deadline_at =
        Timestamp::from_millis(Timestamp::now().as_millis().saturating_add(
            i64::try_from(domm_game::BATTLE_ACTION_DEADLINE_MS).unwrap_or(i64::MAX),
        ));
    battles::create_battle(
        session.id(),
        "active".to_string(),
        battle_type.to_string(),
        attacker_champion_id,
        defender_champion_id,
        defender_town_id,
        defender_neutral_army_id,
        "attacker".to_string(),
        domm_game::BATTLE_GRID_WIDTH,
        domm_game::BATTLE_GRID_HEIGHT,
        domm_game::BATTLE_MAX_ROUNDS,
        seed,
        session.current_turn,
        Some(action_deadline_at),
        command_id,
    )
}

fn create_champion_side_stacks(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    champion_id: Id<Champion>,
    owner_participant_id: Option<Id<GameParticipant>>,
    side: &str,
    x: u8,
) -> Result<Vec<BattleStack>, ApiError> {
    let mut rows = Vec::new();
    for stack in champions_artifacts::page_champion_army_stacks(
        champion_id,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    .into_iter()
    .filter(|stack| stack.status == "active" && stack.quantity > 0)
    {
        let unit_id = Id::<UnitDefinition>::from_key(stack.unit_id);
        let unit = content::load_unit(unit_id)?
            .ok_or_else(|| public_error("unit_not_found", "battle unit not found", true))?;
        let y = match stack.slot_index {
            0 => 3,
            1 => 6,
            value => 2_u8
                .saturating_add(value)
                .min(domm_game::BATTLE_GRID_HEIGHT - 1),
        };
        let battle_stack = create_stack_from_unit(
            command_id,
            battle_id,
            unit.id(),
            owner_participant_id,
            side,
            stack.slot_index,
            "champion_army",
            Some(stack.id().to_string()),
            stack.slot_index,
            unit.attack,
            unit.defense,
            unit.damage_min,
            unit.damage_max,
            unit.max_hp,
            unit.speed,
            unit.initiative,
            unit.ranged,
            unit.flying,
            stack.quantity,
            stack.front_hp,
            unit.shots,
            x,
            y,
        )?;
        battles::create_battle_occupancy(
            battle_id,
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        rows.push(battle_stack);
    }
    Ok(rows)
}

fn create_town_defender_stacks(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    town_id: Id<Town>,
) -> Result<Vec<BattleStack>, ApiError> {
    let mut rows = Vec::new();
    for stack in towns::page_town_garrison(town_id, domm_game::MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .filter(|stack| stack.quantity > 0)
    {
        let unit_id = Id::<UnitDefinition>::from_key(stack.unit_id);
        let unit = content::load_unit(unit_id)?
            .ok_or_else(|| public_error("unit_not_found", "battle unit not found", true))?;
        let y = 4_u8
            .saturating_add(stack.slot_index)
            .min(domm_game::BATTLE_GRID_HEIGHT - 1);
        let battle_stack = create_stack_from_unit(
            command_id,
            battle_id,
            unit.id(),
            None,
            "defender",
            stack.slot_index,
            "town_garrison",
            Some(stack.id().to_string()),
            stack.slot_index,
            unit.attack,
            unit.defense,
            unit.damage_min,
            unit.damage_max,
            unit.max_hp,
            unit.speed,
            unit.initiative,
            unit.ranged,
            unit.flying,
            stack.quantity,
            stack.front_hp,
            unit.shots,
            domm_game::BATTLE_GRID_WIDTH - 2,
            y,
        )?;
        battles::create_battle_occupancy(
            battle_id,
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        rows.push(battle_stack);
    }
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn create_stack_from_unit(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    unit_id: Id<UnitDefinition>,
    owner_participant_id: Option<Id<GameParticipant>>,
    side: &str,
    slot_index: u8,
    origin_kind: &str,
    origin_stack_id_text: Option<String>,
    origin_slot_index: u8,
    attack: i16,
    defense: i16,
    damage_min: u16,
    damage_max: u16,
    max_hp: u16,
    speed: u8,
    initiative: u8,
    ranged: bool,
    flying: bool,
    quantity: u32,
    front_hp: u16,
    shots_remaining: u16,
    battle_x: u8,
    battle_y: u8,
) -> Result<BattleStack, ApiError> {
    battles::create_battle_stack(
        battle_id,
        unit_id,
        owner_participant_id,
        side.to_string(),
        slot_index,
        origin_kind.to_string(),
        origin_stack_id_text,
        origin_slot_index,
        attack,
        defense,
        damage_min,
        damage_max,
        max_hp,
        speed,
        initiative,
        ranged,
        flying,
        quantity,
        front_hp,
        shots_remaining,
        battle_x,
        battle_y,
        command_id,
    )
}

fn create_default_obstacles(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
) -> Result<(), ApiError> {
    battles::create_battle_obstacle(battle_id, "rubble".to_string(), 5, 4, command_id)?;
    battles::create_battle_obstacle(battle_id, "broken-cart".to_string(), 6, 5, command_id)?;
    Ok(())
}

fn set_initial_active_stack(
    session: &GameSession,
    battle: &mut Battle,
    stacks: &mut [BattleStack],
) -> Result<Battle, ApiError> {
    stacks.sort_by(|left, right| {
        right
            .initiative
            .cmp(&left.initiative)
            .then_with(|| right.speed.cmp(&left.speed))
            .then_with(|| {
                battle_stack_tie_break(session, battle, left)
                    .cmp(&battle_stack_tie_break(session, battle, right))
            })
            .then_with(|| left.id().to_string().cmp(&right.id().to_string()))
    });
    if let Some(active_stack) = stacks.first() {
        battle.active_stack_id = Some(active_stack.id().key());
        battle.active_side = active_stack.side.clone();
    }
    battles::update_battle(battle.clone())
}

fn battle_seed(
    session: &GameSession,
    champion: &Champion,
    target_id: &str,
    coord: MoveCoord,
) -> u64 {
    let hash = command_response::payload_hash(
        "battle_turn_seed",
        &session.seed.to_string(),
        &session.current_turn.to_string(),
        &format!(
            "{}:{}:{}:{}:{}",
            champion.id(),
            target_id,
            coord.x,
            coord.y,
            session.id()
        ),
    );
    u64::from_str_radix(hash.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}

fn battle_stack_tie_break(session: &GameSession, battle: &Battle, stack: &BattleStack) -> u64 {
    let hash = command_response::payload_hash(
        "battle_initiative_tie",
        &session.seed.to_string(),
        &battle.current_round.to_string(),
        &format!("{}:{}:{}", battle.id(), stack.id(), stack.side),
    );
    u64::from_str_radix(hash.get(..16).unwrap_or("0"), 16).unwrap_or(0)
}
