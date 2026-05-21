use std::{cell::RefCell, collections::BTreeMap};

use domm_degens_schema::schema::{
    Battle, BattleObstacle, BattleOccupancy, BattleStack, Champion, ChampionArmyStack, GameCommand,
    GameParticipant, GameSession, NeutralArmy, NeutralArmyStack, Town, UnitDefinition,
};
use domm_game::{ApiError, MoveCoord};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::{battles, champions_artifacts, content, neutrals};

use super::{
    battle as battle_service, battle_runtime, command_response, session_context::public_error,
    town_runtime,
};

#[derive(Clone, Default)]
struct BattleStartupRows {
    stacks: Vec<BattleStack>,
    obstacles: Vec<BattleObstacle>,
    occupancy: Vec<BattleOccupancy>,
}

thread_local! {
    static BATTLE_STARTUP_ROWS: RefCell<BTreeMap<String, BattleStartupRows>> =
        const { RefCell::new(BTreeMap::new()) };
    static SEEDED_CHAMPION_ARMY_STACKS: RefCell<Vec<(String, Vec<ChampionArmyStack>)>> =
        const { RefCell::new(Vec::new()) };
    static SEEDED_NEUTRAL_ARMY_STACKS: RefCell<Vec<(String, Vec<NeutralArmyStack>)>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) fn remember_seeded_champion_army_stacks(
    champion_id: Id<Champion>,
    stacks: Vec<ChampionArmyStack>,
) {
    if stacks.is_empty() {
        return;
    }
    let key = champion_id.to_string();
    SEEDED_CHAMPION_ARMY_STACKS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
        cache.push((key, stacks));
    });
}

pub(crate) fn remember_seeded_neutral_army_stacks(
    neutral_army_id: Id<NeutralArmy>,
    stacks: Vec<NeutralArmyStack>,
) {
    if stacks.is_empty() {
        return;
    }
    let key = neutral_army_id.to_string();
    SEEDED_NEUTRAL_ARMY_STACKS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
        cache.push((key, stacks));
    });
}

fn take_seeded_champion_army_stacks(champion_id: Id<Champion>) -> Option<Vec<ChampionArmyStack>> {
    let key = champion_id.to_string();
    SEEDED_CHAMPION_ARMY_STACKS.with_borrow_mut(|cache| {
        cache
            .iter()
            .position(|(existing, _)| existing == &key)
            .map(|index| cache.remove(index).1)
    })
}

pub(crate) fn source_champion_army_stacks(
    champion_id: Id<Champion>,
) -> Result<Vec<ChampionArmyStack>, ApiError> {
    match take_seeded_champion_army_stacks(champion_id) {
        Some(stacks) => Ok(stacks),
        None => Ok(champions_artifacts::page_champion_army_stacks(
            champion_id,
            domm_game::MAX_LIST_LIMIT,
            None,
        )?
        .items),
    }
}

pub(crate) fn source_neutral_army_stacks(
    neutral_army_id: Id<NeutralArmy>,
) -> Result<Vec<NeutralArmyStack>, ApiError> {
    let key = neutral_army_id.to_string();
    if let Some(stacks) = SEEDED_NEUTRAL_ARMY_STACKS.with_borrow_mut(|cache| {
        cache
            .iter()
            .position(|(existing, _)| existing == &key)
            .map(|index| cache.remove(index).1)
    }) {
        return Ok(stacks);
    }
    Ok(neutrals::page_neutral_army_stacks(neutral_army_id, domm_game::MAX_LIST_LIMIT, None)?.items)
}

pub(crate) fn remember_startup_stacks(battle_id: Id<Battle>, stacks: Vec<BattleStack>) {
    if stacks.is_empty() {
        return;
    }
    BATTLE_STARTUP_ROWS.with_borrow_mut(|cache| {
        let rows = cache.entry(battle_id.to_string()).or_default();
        for stack in stacks {
            if let Some(existing) = rows
                .stacks
                .iter_mut()
                .find(|existing| existing.id() == stack.id())
            {
                *existing = stack;
            } else {
                rows.stacks.push(stack);
            }
        }
    });
}

pub(crate) fn remember_startup_obstacles(battle_id: Id<Battle>, obstacles: Vec<BattleObstacle>) {
    if obstacles.is_empty() {
        return;
    }
    BATTLE_STARTUP_ROWS.with_borrow_mut(|cache| {
        let rows = cache.entry(battle_id.to_string()).or_default();
        for obstacle in obstacles {
            if let Some(existing) = rows
                .obstacles
                .iter_mut()
                .find(|existing| existing.id() == obstacle.id())
            {
                *existing = obstacle;
            } else {
                rows.obstacles.push(obstacle);
            }
        }
    });
}

pub(crate) fn remember_startup_occupancy(battle_id: Id<Battle>, occupancy: Vec<BattleOccupancy>) {
    if occupancy.is_empty() {
        return;
    }
    BATTLE_STARTUP_ROWS.with_borrow_mut(|cache| {
        let rows = cache.entry(battle_id.to_string()).or_default();
        for cell in occupancy {
            if let Some(existing) = rows
                .occupancy
                .iter_mut()
                .find(|existing| existing.id() == cell.id())
            {
                *existing = cell;
            } else {
                rows.occupancy.push(cell);
            }
        }
    });
}

pub(crate) fn has_startup_rows(battle_id: Id<Battle>) -> bool {
    BATTLE_STARTUP_ROWS.with_borrow(|cache| cache.contains_key(&battle_id.to_string()))
}

pub(crate) fn has_startup_stacks_for_side(battle_id: Id<Battle>, side: &str) -> bool {
    BATTLE_STARTUP_ROWS.with_borrow(|cache| {
        cache
            .get(&battle_id.to_string())
            .is_some_and(|rows| rows.stacks.iter().any(|stack| stack.side == side))
    })
}

pub(crate) fn has_startup_obstacles(battle_id: Id<Battle>) -> bool {
    BATTLE_STARTUP_ROWS.with_borrow(|cache| {
        cache
            .get(&battle_id.to_string())
            .is_some_and(|rows| !rows.obstacles.is_empty())
    })
}

pub(crate) fn take_complete_startup_rows(
    battle_id: Id<Battle>,
) -> Option<(Vec<BattleStack>, Vec<BattleObstacle>, Vec<BattleOccupancy>)> {
    BATTLE_STARTUP_ROWS.with_borrow_mut(|cache| {
        let key = battle_id.to_string();
        let complete = cache.get(&key).is_some_and(|rows| {
            rows.stacks.iter().any(|stack| stack.side == "attacker")
                && rows.stacks.iter().any(|stack| stack.side == "defender")
                && !rows.obstacles.is_empty()
                && rows.occupancy.len() >= rows.stacks.len()
        });
        complete.then(|| {
            let rows = cache
                .remove(&key)
                .expect("startup rows existed after complete check");
            (rows.stacks, rows.obstacles, rows.occupancy)
        })
    })
}

pub(crate) fn discard_startup_rows(battle_id: Id<Battle>) {
    BATTLE_STARTUP_ROWS.with_borrow_mut(|cache| {
        cache.remove(&battle_id.to_string());
    });
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn persist_battle_header_for_handoff(
    battle: &Battle,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    let mut row = battle.clone();
    row.last_command_id = Some(command_id.key());
    battles::update_battle(row)?;
    Ok(())
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn persist_loaded_startup_rows_for_handoff(
    stacks: &[BattleStack],
    obstacles: &[BattleObstacle],
    occupancy: &[BattleOccupancy],
) -> Result<usize, ApiError> {
    let mut persisted = 0_usize;
    for stack in stacks {
        if battles::load_battle_stack(stack.id())?.is_some() {
            battles::update_battle_stack(stack.clone())?;
        } else {
            battles::insert_battle_stack(stack.clone())?;
        }
        persisted = persisted.saturating_add(1);
    }
    for obstacle in obstacles {
        if battles::load_battle_obstacle(obstacle.id())?.is_some() {
            battles::update_battle_obstacle(obstacle.clone())?;
        } else {
            battles::insert_battle_obstacle(obstacle.clone())?;
        }
        persisted = persisted.saturating_add(1);
    }
    for row in occupancy {
        if battles::load_battle_occupancy(row.id())?.is_some() {
            battles::update_battle_occupancy(row.clone())?;
        } else {
            battles::insert_battle_occupancy(row.clone())?;
        }
        persisted = persisted.saturating_add(1);
    }
    Ok(persisted)
}

pub(crate) fn start_champion_battle(
    session: &GameSession,
    command_id: Id<GameCommand>,
    attacker: &Champion,
    _attacker_participant_id: Id<GameParticipant>,
    defender: &Champion,
    coord: MoveCoord,
) -> Result<Option<Battle>, ApiError> {
    if let Some(existing) =
        battles::find_champion_battle_by_attacker_defender(attacker.id(), defender.id())?
    {
        if existing.state == "active" || existing.state.starts_with("starting") {
            if existing.state.starts_with("starting") {
                return continue_champion_battle_start(
                    session, command_id, existing, attacker, defender,
                );
            }
            battle_runtime::adopt_active_battle_from_rows(session, existing.clone())?;
            return Ok(Some(existing));
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
    let stacks = create_champion_side_stacks(
        command_id,
        battle.id(),
        attacker.id(),
        Some(Id::<GameParticipant>::from_key(attacker.participant_id)),
        "attacker",
        1,
    )?;
    remember_startup_stacks(battle.id(), stacks);
    battle.state = "starting_attacker".to_string();
    battles::update_battle(battle)?;
    Ok(None)
}

fn continue_champion_battle_start(
    session: &GameSession,
    command_id: Id<GameCommand>,
    mut battle: Battle,
    attacker: &Champion,
    defender: &Champion,
) -> Result<Option<Battle>, ApiError> {
    match battle.state.as_str() {
        "starting" => {
            let attacker_stacks = battles::page_battle_stacks_by_side(
                battle.id(),
                "attacker",
                domm_game::MAX_LIST_LIMIT,
                None,
            )?;
            if attacker_stacks.items.is_empty() {
                let stacks = create_champion_side_stacks(
                    command_id,
                    battle.id(),
                    attacker.id(),
                    Some(Id::<GameParticipant>::from_key(attacker.participant_id)),
                    "attacker",
                    1,
                )?;
                remember_startup_stacks(battle.id(), stacks);
            } else {
                remember_startup_stacks(battle.id(), attacker_stacks.items);
            }
            battle.state = "starting_attacker".to_string();
            battles::update_battle(battle)?;
            Ok(None)
        }
        "starting_attacker" => {
            let defender_stacks = battles::page_battle_stacks_by_side(
                battle.id(),
                "defender",
                domm_game::MAX_LIST_LIMIT,
                None,
            )?;
            if defender_stacks.items.is_empty() {
                let stacks = create_champion_side_stacks(
                    command_id,
                    battle.id(),
                    defender.id(),
                    Some(Id::<GameParticipant>::from_key(defender.participant_id)),
                    "defender",
                    domm_game::BATTLE_GRID_WIDTH - 2,
                )?;
                remember_startup_stacks(battle.id(), stacks);
            } else {
                remember_startup_stacks(battle.id(), defender_stacks.items);
            }
            battle.state = "starting_defender".to_string();
            battles::update_battle(battle)?;
            Ok(None)
        }
        "starting_defender" => {
            create_default_obstacles(command_id, battle.id())?;
            battle.state = "starting_obstacles".to_string();
            battles::update_battle(battle)?;
            Ok(None)
        }
        "starting_obstacles" => {
            let cached_rows = take_complete_startup_rows(battle.id());
            let mut stacks = cached_rows
                .as_ref()
                .map(|rows| rows.0.clone())
                .map_or_else(
                    || battles::list_battle_stacks(battle.id(), domm_game::MAX_LIST_LIMIT),
                    Ok,
                )?;
            battle.state = "active".to_string();
            battle.action_deadline_at = Some(fresh_action_deadline_at());
            battle = set_initial_active_stack(session, &mut battle, &mut stacks)?;
            battle_service::schedule_new_battle_timeout_job(session.id(), &battle)?;
            #[cfg(not(feature = "benchmark"))]
            persist_battle_header_for_handoff(&battle, command_id)?;
            if let Some((_, obstacles, occupancy)) = cached_rows {
                #[cfg(not(feature = "benchmark"))]
                persist_loaded_startup_rows_for_handoff(&stacks, &obstacles, &occupancy)?;
                battle_runtime::adopt_active_battle_from_loaded_rows(
                    session,
                    battle.clone(),
                    stacks,
                    obstacles,
                    occupancy,
                )?;
            } else {
                battle_runtime::adopt_active_battle_from_rows_with_stacks(
                    session,
                    battle.clone(),
                    stacks,
                )?;
                discard_startup_rows(battle.id());
            }
            Ok(Some(battle))
        }
        _ => Ok(None),
    }
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
            battle_runtime::adopt_active_battle_from_rows(session, existing.clone())?;
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
    stacks.extend(create_town_defender_stacks(command_id, battle.id(), town)?);
    create_default_obstacles(command_id, battle.id())?;
    if stacks.iter().all(|stack| stack.side != "defender") {
        battle.state = "resolved".to_string();
        battle.winner_participant_id = Some(attacker_participant_id.key());
        battle.active_stack_id = None;
        battle.action_deadline_at = None;
        battle.resolved_at = Some(Timestamp::now());
        discard_startup_rows(battle.id());
        battles::update_battle(battle)
    } else {
        let battle = set_initial_active_stack(session, &mut battle, &mut stacks)?;
        battle_service::schedule_new_battle_timeout_job(session.id(), &battle)?;
        #[cfg(not(feature = "benchmark"))]
        persist_battle_header_for_handoff(&battle, command_id)?;
        if let Some((_, obstacles, occupancy)) = take_complete_startup_rows(battle.id()) {
            #[cfg(not(feature = "benchmark"))]
            persist_loaded_startup_rows_for_handoff(&stacks, &obstacles, &occupancy)?;
            battle_runtime::adopt_active_battle_from_loaded_rows(
                session,
                battle.clone(),
                stacks,
                obstacles,
                occupancy,
            )?;
        } else {
            battle_runtime::adopt_active_battle_from_rows_with_stacks(
                session,
                battle.clone(),
                stacks,
            )?;
            discard_startup_rows(battle.id());
        }
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
        Some(fresh_action_deadline_at()),
        command_id,
    )
}

pub(crate) fn fresh_action_deadline_at() -> Timestamp {
    Timestamp::from_millis(
        Timestamp::now().as_millis().saturating_add(
            i64::try_from(domm_game::BATTLE_ACTION_DEADLINE_MS).unwrap_or(i64::MAX),
        ),
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
    let battle_spell_status_keys = battle_spell_status_keys_for_champion(champion_id)?;
    for stack in source_champion_army_stacks(champion_id)?
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
        let mut battle_stack = create_stack_from_unit(
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
        if !battle_spell_status_keys.is_empty() {
            battle_stack.status_keys = battle_spell_status_keys.clone();
            battle_stack.last_command_id = Some(command_id.key());
            battle_stack = battles::update_battle_stack(battle_stack)?;
        }
        let occupancy = battles::create_battle_occupancy(
            battle_id,
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        remember_startup_occupancy(battle_id, vec![occupancy]);
        rows.push(battle_stack);
    }
    Ok(rows)
}

fn battle_spell_status_keys_for_champion(
    champion_id: Id<Champion>,
) -> Result<Vec<String>, ApiError> {
    let mut status_keys = champions_artifacts::page_champion_spells(
        champion_id,
        domm_game::CHAMPION_SPELLBOOK_CAP as u32,
        None,
    )?
    .items
    .into_iter()
    .filter_map(|spell| spell.spell_slug)
    .map(|slug| format!("battle_spell:{slug}"))
    .collect::<Vec<_>>();
    status_keys.sort();
    status_keys.dedup();
    Ok(status_keys)
}

fn create_town_defender_stacks(
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    town: &Town,
) -> Result<Vec<BattleStack>, ApiError> {
    let mut rows = Vec::new();
    for stack in town_runtime::projection_for_town(town)?
        .garrison_stacks
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
        let occupancy = battles::create_battle_occupancy(
            battle_id,
            battle_stack.id(),
            battle_stack.battle_x,
            battle_stack.battle_y,
            command_id,
        )?;
        remember_startup_occupancy(battle_id, vec![occupancy]);
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
    let obstacles = vec![
        battles::create_battle_obstacle(battle_id, "rubble".to_string(), 5, 4, command_id)?,
        battles::create_battle_obstacle(battle_id, "broken-cart".to_string(), 6, 5, command_id)?,
    ];
    remember_startup_obstacles(battle_id, obstacles);
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
