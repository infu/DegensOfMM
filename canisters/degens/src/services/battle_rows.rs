use std::collections::{BTreeMap, BTreeSet};

use domm_degens_schema::schema::{Battle, BattleOccupancy, BattleStack, GameCommand, GameSession};
use domm_game::{
    ApiError, BattleCommandRecord, BattleObstacleRecord, BattleOccupancyRecord, BattleRecord,
    BattleStackRecord, BattleState, MAX_LIST_LIMIT,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::repos::battles;

use super::session_context::{self, public_error};

pub(crate) fn load_battle_row(session: &GameSession, battle_id: &str) -> Result<Battle, ApiError> {
    let id = session_context::parse_id::<Battle>(battle_id, "battle_id")?;
    let battle = battles::load_battle(id)?.ok_or_else(|| {
        public_error(
            "battle_not_found",
            format!("battle was not found: {battle_id}"),
            false,
        )
    })?;
    if battle.session_id != session.id().key() {
        return Err(public_error(
            "battle_not_found",
            "battle does not belong to this session",
            false,
        ));
    }
    Ok(battle)
}

pub(crate) fn load_battle_state(
    session: &GameSession,
    battle_id: &str,
) -> Result<BattleState, ApiError> {
    let battle = load_battle_row(session, battle_id)?;
    load_battle_state_from_row(session, battle)
}

pub(crate) fn load_battle_state_from_row(
    session: &GameSession,
    battle: Battle,
) -> Result<BattleState, ApiError> {
    let battle_id = battle.id();
    let stacks = battles::page_battle_stacks(battle_id, MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .map(stack_record)
        .collect::<Vec<_>>();
    let obstacles = battles::page_battle_obstacles(battle_id, MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .map(obstacle_record)
        .collect::<Vec<_>>();
    let occupancy = battles::page_battle_occupancy(battle_id, MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .map(occupancy_record)
        .collect::<Vec<_>>();

    Ok(BattleState {
        session_seed: session.seed.to_string(),
        battles: vec![battle_record(battle)],
        stacks,
        obstacles,
        occupancy,
        commands: Vec::new(),
        events: Vec::new(),
    })
}

pub(crate) fn persist_battle_state(
    state: &BattleState,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    let battle = state
        .battles
        .first()
        .ok_or_else(|| public_error("battle_not_found", "battle state is empty", true))?;
    let battle_id = session_context::parse_id::<Battle>(&battle.battle_id, "battle_id")?;
    let mut row = battles::load_battle(battle_id)?.ok_or_else(|| {
        public_error(
            "battle_not_found",
            format!("battle was not found: {}", battle.battle_id),
            true,
        )
    })?;

    row.state = battle.state.clone();
    row.current_round = battle.current_round;
    row.active_side = battle.active_side.clone();
    row.active_stack_id = battle
        .active_stack_id
        .as_deref()
        .map(|id| session_context::parse_id::<BattleStack>(id, "active_stack_id"))
        .transpose()?
        .map(|id| id.key());
    row.winner_participant_id = battle
        .winner_participant_id
        .as_deref()
        .map(|id| {
            session_context::parse_id::<domm_degens_schema::schema::GameParticipant>(
                id,
                "winner_participant_id",
            )
        })
        .transpose()?
        .map(|id| id.key());
    row.action_deadline_at = battle.action_deadline_at.map(timestamp_from_ms);
    row.resolved_at = battle
        .resolved_at
        .map(timestamp_from_ms)
        .or_else(|| (battle.state == "resolved" && row.resolved_at.is_none()).then(Timestamp::now));
    row.cleanup_after_turn = battle.cleanup_after_turn;
    row.last_command_id = Some(command_id.key());
    battles::update_battle(row)?;

    persist_stacks(state, battle_id, command_id)?;
    persist_occupancy(state, battle_id, command_id)?;
    Ok(())
}

pub(crate) fn battle_action_command(
    command: &GameCommand,
    battle_id: &str,
    participant_id: Option<String>,
    stack_id: String,
    action: String,
    target_stack_id: Option<String>,
    destination: Option<domm_game::BattleCoord>,
    system: bool,
) -> BattleCommandRecord {
    BattleCommandRecord {
        command_id: command.id().to_string(),
        battle_id: battle_id.to_string(),
        actor_participant_id: participant_id,
        battle_stack_id: Some(stack_id),
        client_nonce: command.client_nonce.to_string(),
        payload_hash: command.payload_hash.clone(),
        action,
        target_stack_id,
        destination,
        system,
        status: "applying".to_string(),
        created_at: command.created_at.as_millis().try_into().unwrap_or(0),
        applied_at: None,
        retryable_error: None,
    }
}

fn persist_stacks(
    state: &BattleState,
    battle_id: Id<Battle>,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    let existing = battles::page_battle_stacks(battle_id, MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .map(|row| (row.id().to_string(), row))
        .collect::<BTreeMap<_, _>>();

    for stack in state.stacks.iter().filter(|stack| {
        session_context::parse_id::<Battle>(&stack.battle_id, "battle_id")
            .map(|id| id == battle_id)
            .unwrap_or(false)
    }) {
        let Some(mut row) = existing.get(&stack.battle_stack_id).cloned() else {
            continue;
        };
        row.quantity = stack.quantity;
        row.front_hp = stack.front_hp;
        row.shots_remaining = stack.shots_remaining;
        row.battle_x = stack.battle_x;
        row.battle_y = stack.battle_y;
        row.readiness = stack.readiness;
        row.acted_round = stack.acted_round;
        row.retaliated_round = stack.retaliated_round;
        row.defended_round = stack.defended_round;
        row.waited_round = stack.waited_round;
        row.cast_round = stack.cast_round;
        row.status = stack.status.clone();
        row.status_keys = stack.status_keys.clone();
        row.last_command_id = Some(command_id.key());
        battles::update_battle_stack(row)?;
    }
    Ok(())
}

fn persist_occupancy(
    state: &BattleState,
    battle_id: Id<Battle>,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    let existing = battles::page_battle_occupancy(battle_id, MAX_LIST_LIMIT, None)?.items;
    let wanted = state
        .occupancy
        .iter()
        .filter(|occupancy| {
            session_context::parse_id::<Battle>(&occupancy.battle_id, "battle_id")
                .map(|id| id == battle_id)
                .unwrap_or(false)
        })
        .map(|occupancy| (occupancy.battle_occupancy_id.clone(), occupancy))
        .collect::<BTreeMap<_, _>>();
    let wanted_ids = wanted.keys().cloned().collect::<BTreeSet<_>>();

    for mut row in existing {
        let row_id = row.id().to_string();
        if !wanted_ids.contains(&row_id) {
            battles::delete_battle_occupancy(row.id())?;
            continue;
        }
        let wanted = wanted
            .get(&row_id)
            .expect("wanted id should be present after contains check");
        row.battle_x = wanted.battle_x;
        row.battle_y = wanted.battle_y;
        row.last_command_id = Some(command_id.key());
        battles::update_battle_occupancy(row)?;
    }
    Ok(())
}

fn battle_record(row: Battle) -> BattleRecord {
    BattleRecord {
        battle_id: row.id().to_string(),
        session_id: Id::<GameSession>::from_key(row.session_id).to_string(),
        state: row.state,
        battle_type: row.battle_type,
        attacker_champion_id: row
            .attacker_champion_id
            .map(|id| Id::<domm_degens_schema::schema::Champion>::from_key(id).to_string()),
        defender_champion_id: row
            .defender_champion_id
            .map(|id| Id::<domm_degens_schema::schema::Champion>::from_key(id).to_string()),
        defender_town_id: row
            .defender_town_id
            .map(|id| Id::<domm_degens_schema::schema::Town>::from_key(id).to_string()),
        defender_neutral_army_id: row
            .defender_neutral_army_id
            .map(|id| Id::<domm_degens_schema::schema::NeutralArmy>::from_key(id).to_string()),
        current_round: row.current_round,
        active_side: row.active_side,
        active_stack_id: row
            .active_stack_id
            .map(|id| Id::<BattleStack>::from_key(id).to_string()),
        grid_width: row.grid_width,
        grid_height: row.grid_height,
        max_rounds: row.max_rounds,
        turn_seed: row.turn_seed,
        winner_participant_id: row
            .winner_participant_id
            .map(|id| Id::<domm_degens_schema::schema::GameParticipant>::from_key(id).to_string()),
        created_turn: row.created_turn,
        action_deadline_at: row.action_deadline_at.and_then(ms_from_timestamp),
        resolved_at: row.resolved_at.and_then(ms_from_timestamp),
        cleanup_after_turn: row.cleanup_after_turn,
        last_command_id: row
            .last_command_id
            .map(|id| Id::<GameCommand>::from_key(id).to_string()),
    }
}

fn stack_record(row: BattleStack) -> BattleStackRecord {
    BattleStackRecord {
        battle_stack_id: row.id().to_string(),
        battle_id: Id::<Battle>::from_key(row.battle_id).to_string(),
        unit_id: Id::<domm_degens_schema::schema::UnitDefinition>::from_key(row.unit_id)
            .to_string(),
        owner_participant_id: row
            .owner_participant_id
            .map(|id| Id::<domm_degens_schema::schema::GameParticipant>::from_key(id).to_string()),
        side: row.side,
        slot_index: row.slot_index,
        origin_kind: row.origin_kind,
        origin_stack_id_text: row.origin_stack_id_text,
        origin_slot_index: row.origin_slot_index,
        champion_might: 0,
        champion_guard: 0,
        attack: row.attack,
        defense: row.defense,
        damage_min: row.damage_min,
        damage_max: row.damage_max,
        max_hp: row.max_hp,
        speed: row.speed,
        initiative: row.initiative,
        ranged: row.ranged,
        flying: row.flying,
        quantity: row.quantity,
        front_hp: row.front_hp,
        shots_remaining: row.shots_remaining,
        battle_x: row.battle_x,
        battle_y: row.battle_y,
        readiness: row.readiness,
        acted_round: row.acted_round,
        retaliated_round: row.retaliated_round,
        defended_round: row.defended_round,
        waited_round: row.waited_round,
        cast_round: row.cast_round,
        status: row.status,
        last_command_id: row
            .last_command_id
            .map(|id| Id::<GameCommand>::from_key(id).to_string()),
        status_keys: row.status_keys,
    }
}

fn obstacle_record(row: domm_degens_schema::schema::BattleObstacle) -> BattleObstacleRecord {
    BattleObstacleRecord {
        battle_obstacle_id: row.id().to_string(),
        battle_id: Id::<Battle>::from_key(row.battle_id).to_string(),
        obstacle_type: row.obstacle_type,
        battle_x: row.battle_x,
        battle_y: row.battle_y,
        width: row.width,
        height: row.height,
        hp: row.hp,
        state: row.state,
        last_command_id: row
            .last_command_id
            .map(|id| Id::<GameCommand>::from_key(id).to_string()),
    }
}

fn occupancy_record(row: BattleOccupancy) -> BattleOccupancyRecord {
    BattleOccupancyRecord {
        battle_occupancy_id: row.id().to_string(),
        battle_id: Id::<Battle>::from_key(row.battle_id).to_string(),
        battle_stack_id: Id::<BattleStack>::from_key(row.battle_stack_id).to_string(),
        battle_x: row.battle_x,
        battle_y: row.battle_y,
        last_command_id: row
            .last_command_id
            .map(|id| Id::<GameCommand>::from_key(id).to_string()),
    }
}

fn ms_from_timestamp(timestamp: Timestamp) -> Option<u64> {
    timestamp.as_millis().try_into().ok()
}

fn timestamp_from_ms(ms: u64) -> Timestamp {
    Timestamp::from_millis(i64::try_from(ms).unwrap_or(i64::MAX))
}
