use std::collections::BTreeSet;

use crate::champion::ChampionState;
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::map::{FirstPlayableMapState, MAP_FLAG_BLOCKING_TERRAIN, MapSubjectRecord};

use super::types::{
    MAX_MOVE_CHUNKS_TOUCHED, MAX_MOVE_PATH_STEPS, MoveCoord, MovementError, MovementPathStop,
    MovementPreview, MovementState,
};

pub fn preview_move_path(
    movement: &MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    now_ms: u64,
) -> Result<MovementPreview, MovementError> {
    validate_move_path(
        movement,
        map,
        champions,
        participant_id,
        champion_id,
        path,
        now_ms,
    )
}

pub(crate) fn validate_move_path(
    movement: &MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    now_ms: u64,
) -> Result<MovementPreview, MovementError> {
    if movement.session_status != "active" {
        return Err(MovementError::SessionNotActive);
    }
    if !movement.accepts_intents_at(now_ms) {
        return Err(MovementError::SubmissionWindowClosed {
            turn_number: movement.current_turn,
            now_ms,
        });
    }
    if path.is_empty() {
        return Err(MovementError::PathEmpty);
    }
    if path.len() > MAX_MOVE_PATH_STEPS {
        return Err(MovementError::PathTooLong {
            path_len: path.len(),
            max_len: MAX_MOVE_PATH_STEPS,
        });
    }

    let champion = champions.champion(champion_id)?;
    if champion.participant_id != participant_id {
        return Err(MovementError::ChampionNotOwned {
            participant_id: participant_id.to_string(),
            champion_id: champion_id.to_string(),
        });
    }
    if champion.status != "active" {
        return Err(MovementError::ChampionNotActive {
            champion_id: champion_id.to_string(),
            status: champion.status.clone(),
        });
    }

    let mut previous = MoveCoord::new(champion.x, champion.y);
    let mut total_cost: u16 = 0;
    let mut chunks_touched = BTreeSet::new();
    let mut normalized_path = Vec::with_capacity(path.len());
    let mut stop = None;

    for (step_index, coord) in path.iter().copied().enumerate() {
        if !coord.is_adjacent_to(previous) {
            return Err(MovementError::PathStepNotAdjacent {
                step_index,
                from_x: previous.x,
                from_y: previous.y,
                to_x: coord.x,
                to_y: coord.y,
            });
        }

        let Some(flags) = map.flags_at(coord.x, coord.y) else {
            return Err(MovementError::OutOfBounds {
                x: coord.x,
                y: coord.y,
            });
        };
        if flags & MAP_FLAG_BLOCKING_TERRAIN != 0 {
            return Err(MovementError::ImpassableTerrain {
                x: coord.x,
                y: coord.y,
            });
        }
        if !map.is_visible_at(participant_id, coord.x, coord.y)
            && !map.is_discovered_at(participant_id, coord.x, coord.y)
        {
            return Err(MovementError::HiddenTile {
                participant_id: participant_id.to_string(),
                x: coord.x,
                y: coord.y,
            });
        }

        let cost = map
            .movement_cost_at(coord.x, coord.y)
            .ok_or(MovementError::OutOfBounds {
                x: coord.x,
                y: coord.y,
            })?;
        total_cost = total_cost.saturating_add(u16::from(cost));
        chunks_touched.insert(chunk_key(coord));
        if chunks_touched.len() > MAX_MOVE_CHUNKS_TOUCHED {
            return Err(MovementError::TooManyChunks {
                chunk_count: chunks_touched.len(),
                max_chunks: MAX_MOVE_CHUNKS_TOUCHED,
            });
        }

        normalized_path.push(coord);
        if let Some(path_stop) =
            visible_dynamic_stop(map, champions, participant_id, coord, champion_id)
        {
            stop = Some(path_stop);
            break;
        }
        if let Some(path_stop) = visible_static_stop(map, participant_id, coord, champion_id) {
            stop = Some(path_stop);
            break;
        }
        previous = coord;
    }

    let available = champions.effective_movement(champion_id, movement.current_turn)?;
    if total_cost > available {
        return Err(MovementError::MovementTooExpensive {
            cost: total_cost,
            available,
        });
    }

    Ok(MovementPreview {
        champion_id: champion_id.to_string(),
        participant_id: participant_id.to_string(),
        turn_number: movement.current_turn,
        path: normalized_path,
        total_cost,
        available_movement: available,
        chunks_touched: chunks_touched.len() as u32,
        stop,
    })
}

pub(crate) fn visible_static_stop(
    map: &FirstPlayableMapState,
    participant_id: &str,
    coord: MoveCoord,
    moving_champion_id: &str,
) -> Option<MovementPathStop> {
    let subject = map
        .subjects
        .iter()
        .filter(|subject| subject.x == coord.x && subject.y == coord.y)
        .find(|subject| {
            subject.subject_kind != "champion" || subject.subject_id_text != moving_champion_id
        })?;

    if !map.is_visible_at(participant_id, subject.x, subject.y) {
        return None;
    }

    static_stop_reason(subject, participant_id).map(|reason| MovementPathStop {
        reason,
        subject_kind: subject.subject_kind.clone(),
        subject_id_text: subject.subject_id_text.clone(),
        x: subject.x,
        y: subject.y,
    })
}

fn visible_dynamic_stop(
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    coord: MoveCoord,
    moving_champion_id: &str,
) -> Option<MovementPathStop> {
    if !map.is_visible_at(participant_id, coord.x, coord.y) {
        return None;
    }
    let champion = champions.champions.iter().find(|champion| {
        champion.champion_id != moving_champion_id
            && champion.status != "defeated"
            && champion.participant_id != participant_id
            && champion.x == coord.x
            && champion.y == coord.y
    })?;
    Some(MovementPathStop {
        reason: "enemy_champion_interaction".to_string(),
        subject_kind: "champion".to_string(),
        subject_id_text: champion.champion_id.clone(),
        x: coord.x,
        y: coord.y,
    })
}

pub(crate) fn static_stop_reason(
    subject: &MapSubjectRecord,
    participant_id: &str,
) -> Option<String> {
    match subject.subject_kind.as_str() {
        "neutral_army" => Some("neutral_army_encounter".to_string()),
        "world_object" => object_requires_stop(subject).then(|| "object_interaction".to_string()),
        "town" if subject.owner_participant_id.as_deref() != Some(participant_id) => {
            Some("town_interaction".to_string())
        }
        _ => None,
    }
}

pub(crate) fn object_requires_stop(subject: &MapSubjectRecord) -> bool {
    subject.subject_kind == "world_object"
        && matches!(
            subject.scoring_kind.as_deref(),
            Some("resource_pile" | "mine" | "objective")
        )
}

fn chunk_key(coord: MoveCoord) -> (u16, u16) {
    let chunk_size = u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    (coord.x / chunk_size, coord.y / chunk_size)
}
