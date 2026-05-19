use std::collections::{HashMap, HashSet};

use super::types::{
    BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BattleCoord, BattleError, BattleOccupancyRecord,
    BattleState,
};

pub fn validate_battle_occupancy(state: &BattleState, battle_id: &str) -> Result<(), BattleError> {
    let battle = state.battle(battle_id)?;
    let mut stack_keys = HashSet::new();
    let mut tile_keys = HashSet::new();
    let stacks = state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id)
        .map(|stack| (stack.battle_stack_id.as_str(), stack))
        .collect::<HashMap<_, _>>();

    for occupancy in state
        .occupancy
        .iter()
        .filter(|occupancy| occupancy.battle_id == battle_id)
    {
        validate_coord(battle.grid_width, battle.grid_height, occupancy.coord())?;
        if !stack_keys.insert(occupancy.battle_stack_id.clone()) {
            return Err(BattleError::DuplicateStackOccupancy {
                battle_stack_id: occupancy.battle_stack_id.clone(),
            });
        }
        if !tile_keys.insert((occupancy.battle_x, occupancy.battle_y)) {
            return Err(BattleError::DuplicateTileOccupancy {
                battle_id: battle_id.to_string(),
                x: occupancy.battle_x,
                y: occupancy.battle_y,
            });
        }
        if is_obstacle_blocked(state, battle_id, occupancy.coord()) {
            return Err(BattleError::ObstacleBlocked {
                battle_id: battle_id.to_string(),
                x: occupancy.battle_x,
                y: occupancy.battle_y,
            });
        }
        let stack = stacks
            .get(occupancy.battle_stack_id.as_str())
            .ok_or_else(|| BattleError::StackNotFound {
                battle_stack_id: occupancy.battle_stack_id.clone(),
            })?;
        if stack.battle_x != occupancy.battle_x || stack.battle_y != occupancy.battle_y {
            return Err(BattleError::OccupancyCacheMismatch {
                battle_stack_id: stack.battle_stack_id.clone(),
                stack_x: stack.battle_x,
                stack_y: stack.battle_y,
                occupancy_x: occupancy.battle_x,
                occupancy_y: occupancy.battle_y,
            });
        }
    }

    for stack in state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.is_living())
    {
        if !stack_keys.contains(&stack.battle_stack_id) {
            return Err(BattleError::MissingStackOccupancy {
                battle_stack_id: stack.battle_stack_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn repair_stack_position_from_occupancy(
    state: &mut BattleState,
    battle_stack_id: &str,
) -> Result<(), BattleError> {
    let occupancy = state
        .occupancy
        .iter()
        .find(|occupancy| occupancy.battle_stack_id == battle_stack_id)
        .cloned()
        .ok_or_else(|| BattleError::MissingStackOccupancy {
            battle_stack_id: battle_stack_id.to_string(),
        })?;
    let stack = state.stack_mut(battle_stack_id)?;
    stack.battle_x = occupancy.battle_x;
    stack.battle_y = occupancy.battle_y;
    Ok(())
}

pub fn occupant_at<'a>(
    state: &'a BattleState,
    battle_id: &str,
    coord: BattleCoord,
) -> Option<&'a BattleOccupancyRecord> {
    state.occupancy.iter().find(|occupancy| {
        occupancy.battle_id == battle_id
            && occupancy.battle_x == coord.x
            && occupancy.battle_y == coord.y
    })
}

pub fn is_tile_open(state: &BattleState, battle_id: &str, coord: BattleCoord) -> bool {
    validate_coord(BATTLE_GRID_WIDTH, BATTLE_GRID_HEIGHT, coord).is_ok()
        && occupant_at(state, battle_id, coord).is_none()
        && !is_obstacle_blocked(state, battle_id, coord)
}

pub fn is_obstacle_blocked(state: &BattleState, battle_id: &str, coord: BattleCoord) -> bool {
    state.obstacles.iter().any(|obstacle| {
        obstacle.battle_id == battle_id
            && obstacle.state == "active"
            && coord.x >= obstacle.battle_x
            && coord.y >= obstacle.battle_y
            && coord.x < obstacle.battle_x.saturating_add(obstacle.width)
            && coord.y < obstacle.battle_y.saturating_add(obstacle.height)
    })
}

pub fn validate_coord(width: u8, height: u8, coord: BattleCoord) -> Result<(), BattleError> {
    if coord.x >= width || coord.y >= height {
        return Err(BattleError::OutOfBounds {
            x: coord.x,
            y: coord.y,
        });
    }
    Ok(())
}

pub fn adjacent_coords(width: u8, height: u8, coord: BattleCoord) -> Vec<BattleCoord> {
    let mut coords = Vec::with_capacity(4);
    if coord.x > 0 {
        coords.push(BattleCoord::new(coord.x - 1, coord.y));
    }
    if coord.x + 1 < width {
        coords.push(BattleCoord::new(coord.x + 1, coord.y));
    }
    if coord.y > 0 {
        coords.push(BattleCoord::new(coord.x, coord.y - 1));
    }
    if coord.y + 1 < height {
        coords.push(BattleCoord::new(coord.x, coord.y + 1));
    }
    coords
}

impl BattleOccupancyRecord {
    #[must_use]
    pub fn coord(&self) -> BattleCoord {
        BattleCoord::new(self.battle_x, self.battle_y)
    }
}
