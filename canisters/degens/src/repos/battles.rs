//! Repository boundary for battle state, stacks, occupancy, obstacles, and tactical events.

use domm_degens_schema::schema::{
    Battle, BattleObstacle, BattleOccupancy, BattleStack, Champion, GameCommand, GameParticipant,
    GameSession, NeutralArmy, Town, UnitDefinition,
};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const BATTLES_BY_SESSION_STATE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.by_session_state",
    entity: "Battle",
    indexed_fields: &["session_id", "state"],
    bounded_limit: Some(domm_game::MAX_ACTIVE_BATTLES_PER_SESSION),
};

pub(crate) const BATTLE_STACKS_BY_SIDE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.stacks_by_side",
    entity: "BattleStack",
    indexed_fields: &["battle_id", "side"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const BATTLE_STACKS_BY_BATTLE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.stacks_by_battle",
    entity: "BattleStack",
    indexed_fields: &["battle_id", "side", "slot_index"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const BATTLE_OBSTACLES_BY_BATTLE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.obstacles_by_battle",
    entity: "BattleObstacle",
    indexed_fields: &["battle_id", "battle_x", "battle_y"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const BATTLE_OCCUPANCY_BY_BATTLE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.occupancy_by_battle",
    entity: "BattleOccupancy",
    indexed_fields: &["battle_id", "battle_x", "battle_y"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const BATTLE_OCCUPANCY_CELL_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battles.occupancy_by_cell",
    entity: "BattleOccupancy",
    indexed_fields: &["battle_id", "battle_x", "battle_y"],
    bounded_limit: Some(1),
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_battle(
    session_id: Id<GameSession>,
    state: String,
    battle_type: String,
    attacker_champion_id: Option<Id<Champion>>,
    defender_champion_id: Option<Id<Champion>>,
    defender_town_id: Option<Id<Town>>,
    defender_neutral_army_id: Option<Id<NeutralArmy>>,
    active_side: String,
    grid_width: u8,
    grid_height: u8,
    max_rounds: u16,
    turn_seed: u64,
    created_turn: u32,
    action_deadline_at: Option<Timestamp>,
    command_id: Id<GameCommand>,
) -> RepoResult<Battle> {
    let input: Create<Battle> = Create::<Battle> {
        session_id: Some(session_id.key()),
        state: Some(state),
        battle_type: Some(battle_type),
        attacker_champion_id: Some(attacker_champion_id.map(|id| id.key())),
        defender_champion_id: Some(defender_champion_id.map(|id| id.key())),
        defender_town_id: Some(defender_town_id.map(|id| id.key())),
        defender_neutral_army_id: Some(defender_neutral_army_id.map(|id| id.key())),
        current_round: Some(1),
        active_side: Some(active_side),
        active_stack_id: Some(None),
        grid_width: Some(grid_width),
        grid_height: Some(grid_height),
        max_rounds: Some(max_rounds),
        turn_seed: Some(turn_seed),
        winner_participant_id: Some(None),
        created_turn: Some(created_turn),
        action_deadline_at: Some(action_deadline_at),
        resolved_at: Some(None),
        cleanup_after_turn: Some(0),
        last_command_id: Some(Some(command_id.key())),
    };

    foundation::create("battles.create_battle", input)
}

pub(crate) fn update_battle(battle: Battle) -> RepoResult<Battle> {
    foundation::update("battles.update_battle", battle)
}

pub(crate) fn load_battle(id: Id<Battle>) -> RepoResult<Option<Battle>> {
    foundation::load_by_id("battles.load_battle", id)
}

pub(crate) fn page_battles_by_session_state(
    session_id: Id<GameSession>,
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Battle>> {
    let limit = foundation::validate_limit(
        "limit",
        limit,
        domm_game::MAX_ACTIVE_BATTLES_PER_SESSION,
        "active_battle_limit_exceeded",
    )?;
    foundation::execute_page(
        BATTLES_BY_SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("created_turn")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_battle_by_attacker(champion_id: Id<Champion>) -> RepoResult<Option<Battle>> {
    foundation::storage_operation("battles.by_attacker", || {
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("attacker_champion_id").eq(champion_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn find_champion_battle_by_attacker_defender(
    attacker_champion_id: Id<Champion>,
    defender_champion_id: Id<Champion>,
) -> RepoResult<Option<Battle>> {
    foundation::storage_operation("battles.by_attacker_defender_champion", || {
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("attacker_champion_id").eq(attacker_champion_id.key()))
            .filter(FieldRef::new("defender_champion_id").eq(defender_champion_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn page_battle_stacks(
    battle_id: Id<Battle>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_STACKS_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleStack>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("side")
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_battle_stacks(
    battle_id: Id<Battle>,
    limit: u32,
) -> RepoResult<Vec<BattleStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_operation(BATTLE_STACKS_BY_BATTLE_LOOKUP.name, || {
        crate::db()
            .load::<BattleStack>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("side")
            .order_asc("slot_index")
            .order_asc("id")
            .limit(limit)
            .entities()
    })
}

pub(crate) fn update_battle_stack(stack: BattleStack) -> RepoResult<BattleStack> {
    foundation::update("battles.update_battle_stack", stack)
}

pub(crate) fn load_battle_stack(id: Id<BattleStack>) -> RepoResult<Option<BattleStack>> {
    foundation::load_by_id("battles.load_battle_stack", id)
}

pub(crate) fn insert_battle_stack(stack: BattleStack) -> RepoResult<BattleStack> {
    foundation::insert("battles.insert_battle_stack", stack)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_battle_stack(
    battle_id: Id<Battle>,
    unit_id: Id<UnitDefinition>,
    owner_participant_id: Option<Id<GameParticipant>>,
    side: String,
    slot_index: u8,
    origin_kind: String,
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
    command_id: Id<GameCommand>,
) -> RepoResult<BattleStack> {
    let input: Create<BattleStack> = Create::<BattleStack> {
        battle_id: Some(battle_id.key()),
        unit_id: Some(unit_id.key()),
        owner_participant_id: Some(owner_participant_id.map(|id| id.key())),
        side: Some(side),
        slot_index: Some(slot_index),
        origin_kind: Some(origin_kind),
        origin_stack_id_text: Some(origin_stack_id_text),
        origin_slot_index: Some(origin_slot_index),
        attack: Some(attack),
        defense: Some(defense),
        damage_min: Some(damage_min),
        damage_max: Some(damage_max),
        max_hp: Some(max_hp),
        speed: Some(speed),
        initiative: Some(initiative),
        ranged: Some(ranged),
        flying: Some(flying),
        quantity: Some(quantity),
        front_hp: Some(front_hp),
        shots_remaining: Some(shots_remaining),
        battle_x: Some(battle_x),
        battle_y: Some(battle_y),
        readiness: Some(0),
        acted_round: Some(0),
        retaliated_round: Some(0),
        defended_round: Some(0),
        waited_round: Some(0),
        cast_round: Some(0),
        status: Some("active".to_string()),
        last_command_id: Some(Some(command_id.key())),
        status_keys: Some(Vec::new()),
    };

    foundation::create("battles.create_battle_stack", input)
}

pub(crate) fn create_battle_occupancy(
    battle_id: Id<Battle>,
    battle_stack_id: Id<BattleStack>,
    battle_x: u8,
    battle_y: u8,
    command_id: Id<GameCommand>,
) -> RepoResult<BattleOccupancy> {
    let input: Create<BattleOccupancy> = Create::<BattleOccupancy> {
        battle_id: Some(battle_id.key()),
        battle_stack_id: Some(battle_stack_id.key()),
        battle_x: Some(battle_x),
        battle_y: Some(battle_y),
        last_command_id: Some(Some(command_id.key())),
    };

    foundation::create("battles.create_battle_occupancy", input)
}

pub(crate) fn update_battle_occupancy(occupancy: BattleOccupancy) -> RepoResult<BattleOccupancy> {
    foundation::update("battles.update_battle_occupancy", occupancy)
}

pub(crate) fn load_battle_occupancy(
    id: Id<BattleOccupancy>,
) -> RepoResult<Option<BattleOccupancy>> {
    foundation::load_by_id("battles.load_battle_occupancy", id)
}

pub(crate) fn insert_battle_occupancy(occupancy: BattleOccupancy) -> RepoResult<BattleOccupancy> {
    foundation::insert("battles.insert_battle_occupancy", occupancy)
}

pub(crate) fn delete_battle_occupancy(occupancy_id: Id<BattleOccupancy>) -> RepoResult<u32> {
    foundation::delete_by_id("battles.delete_battle_occupancy", occupancy_id)
}

pub(crate) fn create_battle_obstacle(
    battle_id: Id<Battle>,
    obstacle_type: String,
    battle_x: u8,
    battle_y: u8,
    command_id: Id<GameCommand>,
) -> RepoResult<BattleObstacle> {
    let input: Create<BattleObstacle> = Create::<BattleObstacle> {
        battle_id: Some(battle_id.key()),
        obstacle_type: Some(obstacle_type),
        battle_x: Some(battle_x),
        battle_y: Some(battle_y),
        width: Some(1),
        height: Some(1),
        hp: Some(0),
        state: Some("active".to_string()),
        last_command_id: Some(Some(command_id.key())),
    };

    foundation::create("battles.create_battle_obstacle", input)
}

pub(crate) fn load_battle_obstacle(id: Id<BattleObstacle>) -> RepoResult<Option<BattleObstacle>> {
    foundation::load_by_id("battles.load_battle_obstacle", id)
}

pub(crate) fn insert_battle_obstacle(obstacle: BattleObstacle) -> RepoResult<BattleObstacle> {
    foundation::insert("battles.insert_battle_obstacle", obstacle)
}

pub(crate) fn update_battle_obstacle(obstacle: BattleObstacle) -> RepoResult<BattleObstacle> {
    foundation::update("battles.update_battle_obstacle", obstacle)
}

pub(crate) fn page_battle_obstacles(
    battle_id: Id<Battle>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleObstacle>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_OBSTACLES_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleObstacle>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_battle_obstacles(
    battle_id: Id<Battle>,
    limit: u32,
) -> RepoResult<Vec<BattleObstacle>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_operation(BATTLE_OBSTACLES_BY_BATTLE_LOOKUP.name, || {
        crate::db()
            .load::<BattleObstacle>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id")
            .limit(limit)
            .entities()
    })
}

pub(crate) fn page_battle_occupancy(
    battle_id: Id<Battle>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleOccupancy>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_OCCUPANCY_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleOccupancy>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn list_battle_occupancy(
    battle_id: Id<Battle>,
    limit: u32,
) -> RepoResult<Vec<BattleOccupancy>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_operation(BATTLE_OCCUPANCY_BY_BATTLE_LOOKUP.name, || {
        crate::db()
            .load::<BattleOccupancy>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id")
            .limit(limit)
            .entities()
    })
}

pub(crate) fn page_battle_stacks_by_side(
    battle_id: Id<Battle>,
    side: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_STACKS_BY_SIDE_LOOKUP.name,
        crate::db()
            .load::<BattleStack>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("side").eq(side))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_battle_occupancy_cell(
    battle_id: Id<Battle>,
    battle_x: u8,
    battle_y: u8,
) -> RepoResult<Option<BattleOccupancy>> {
    foundation::storage_operation(BATTLE_OCCUPANCY_CELL_LOOKUP.name, || {
        crate::db()
            .load::<BattleOccupancy>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("battle_x").eq(battle_x))
            .filter(FieldRef::new("battle_y").eq(battle_y))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

#[cfg(test)]
pub(crate) fn active_battles_plan_text(
    session_id: Id<GameSession>,
    state: &str,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        BATTLES_BY_SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<Battle>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("created_turn")
            .order_asc("id")
            .limit(limit),
    )
}

#[cfg(test)]
#[cfg(test)]
pub(crate) fn battle_stacks_plan_text(battle_id: Id<Battle>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        BATTLE_STACKS_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleStack>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("side")
            .order_asc("slot_index")
            .order_asc("id")
            .limit(limit),
    )
}

#[cfg(test)]
#[cfg(test)]
pub(crate) fn battle_obstacles_plan_text(battle_id: Id<Battle>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        BATTLE_OBSTACLES_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleObstacle>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id")
            .limit(limit),
    )
}

#[cfg(test)]
#[cfg(test)]
pub(crate) fn battle_occupancy_plan_text(battle_id: Id<Battle>, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        BATTLE_OCCUPANCY_BY_BATTLE_LOOKUP.name,
        crate::db()
            .load::<BattleOccupancy>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .order_asc("battle_x")
            .order_asc("battle_y")
            .order_asc("id")
            .limit(limit),
    )
}
