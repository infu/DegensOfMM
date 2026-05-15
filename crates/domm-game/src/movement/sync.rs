use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::champion::ChampionState;
use crate::content::FIRST_PLAYABLE_CHUNK_SIZE;
use crate::map::{FirstPlayableMapState, MapSubjectRecord};
use crate::rng::{RollKey, hash64};

use super::preview::{object_requires_stop, static_stop_reason};
use super::types::{
    BattleStartDraft, MoveCoord, MovementError, MovementPathStop, MovementResolutionCursor,
    MovementSnapshotRecord, MovementState, MovementSyncBudget, MovementSyncOutcome,
    MovementSystemCommandRecord, ObjectStopDraft, movement_resolution_command_id,
};

#[derive(Clone, Debug)]
struct MoveCandidate {
    intent_id: String,
    champion_id: String,
    participant_id: String,
    from: MoveCoord,
    to: MoveCoord,
    movement_cost: u16,
    remaining_before: u16,
    path_distance: u16,
    tie_break: u64,
}

#[derive(Default)]
struct MicrostepOutcome {
    resolved_intent_ids: Vec<String>,
    snapshots: Vec<MovementSnapshotRecord>,
    battle_starts: Vec<BattleStartDraft>,
    object_stops: Vec<ObjectStopDraft>,
    gameplay_rows_written: u32,
}

pub fn sync_session_turn(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    now_ms: u64,
    budget: MovementSyncBudget,
) -> Result<MovementSyncOutcome, MovementError> {
    sync_session_turn_internal(movement, map, champions, now_ms, budget, None)
}

#[cfg(test)]
pub(crate) fn sync_session_turn_with_trap_after_microsteps(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    now_ms: u64,
    budget: MovementSyncBudget,
    trap_after_microsteps: u32,
) -> Result<MovementSyncOutcome, MovementError> {
    sync_session_turn_internal(
        movement,
        map,
        champions,
        now_ms,
        budget,
        Some(trap_after_microsteps),
    )
}

fn sync_session_turn_internal(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    now_ms: u64,
    budget: MovementSyncBudget,
    trap_after_microsteps: Option<u32>,
) -> Result<MovementSyncOutcome, MovementError> {
    if movement.partial_cursor.is_none() && !movement.time_view(now_ms).sync_required {
        return Ok(MovementSyncOutcome::idle(movement, now_ms));
    }

    let (inspected, advanced) = recover_pending_movement_commands(movement, &budget);
    let from_turn = movement
        .partial_cursor
        .as_ref()
        .map_or(movement.current_turn, |cursor| cursor.turn_number);
    let command_id = movement.partial_cursor.as_ref().map_or_else(
        || movement_resolution_command_id(&movement.session_id, from_turn),
        |cursor| cursor.command_id.clone(),
    );
    ensure_system_command(movement, &command_id, from_turn, now_ms);

    let superseded_intent_ids = movement
        .intents
        .iter()
        .filter(|intent| intent.turn_number == from_turn && intent.status == "superseded")
        .map(|intent| intent.intent_id.clone())
        .collect::<Vec<_>>();
    let mut outcome = MovementSyncOutcome {
        session_id: movement.session_id.clone(),
        command_id: command_id.clone(),
        from_turn,
        current_turn: movement.current_turn,
        advanced_turn: false,
        resolved_intent_ids: Vec::new(),
        superseded_intent_ids,
        snapshots: Vec::new(),
        battle_starts: Vec::new(),
        object_stops: Vec::new(),
        budget_exhausted: false,
        recovery_checked: true,
        recovered_commands_inspected: inspected,
        recovered_commands_advanced: advanced,
        gameplay_rows_written: 0,
        cursor: movement.partial_cursor.clone(),
    };

    let mut next_step_index = movement
        .partial_cursor
        .as_ref()
        .map_or(0, |cursor| cursor.next_step_index);
    let mut microsteps_processed = 0_u32;

    loop {
        mark_intents_with_completed_paths(
            movement,
            from_turn,
            next_step_index,
            &command_id,
            &mut outcome,
        );

        let candidates = candidates_for_step(
            movement,
            map,
            champions,
            from_turn,
            next_step_index,
            &command_id,
        )?;
        if candidates.is_empty() {
            finalize_turn(movement, &command_id, from_turn, now_ms, &mut outcome);
            return Ok(outcome);
        }

        if microsteps_processed >= budget.max_microsteps
            || outcome.gameplay_rows_written >= budget.max_gameplay_rows
        {
            park_cursor(
                movement,
                &command_id,
                from_turn,
                next_step_index,
                outcome.gameplay_rows_written,
                &mut outcome,
            );
            outcome.budget_exhausted = true;
            return Ok(outcome);
        }

        let microstep = resolve_microstep(
            movement,
            map,
            champions,
            from_turn,
            next_step_index,
            &command_id,
            now_ms,
            candidates,
        )?;
        merge_microstep(&mut outcome, microstep);
        microsteps_processed += 1;
        next_step_index = next_step_index.saturating_add(1);

        if trap_after_microsteps == Some(microsteps_processed) {
            park_cursor(
                movement,
                &command_id,
                from_turn,
                next_step_index,
                outcome.gameplay_rows_written,
                &mut outcome,
            );
            if let Some(command) = movement
                .system_commands
                .iter_mut()
                .find(|command| command.command_id == command_id)
            {
                command.last_error = Some("simulated_trap_after_partial_apply".to_string());
            }
            return Err(MovementError::SimulatedTrapAfterPartialApply {
                command_id,
                next_step_index,
            });
        }
    }
}

fn recover_pending_movement_commands(
    movement: &mut MovementState,
    budget: &MovementSyncBudget,
) -> (u32, u32) {
    movement.recovery_checks = movement.recovery_checks.saturating_add(1);
    let mut inspected = 0_u32;
    let mut advanced = 0_u32;
    for command in movement
        .system_commands
        .iter_mut()
        .filter(|command| matches!(command.status.as_str(), "pending" | "applying"))
    {
        if inspected >= budget.max_commands_inspected {
            break;
        }
        inspected += 1;
        if command.status == "pending" && advanced < budget.max_commands_advanced {
            command.status = "applying".to_string();
            advanced += 1;
        }
    }
    (inspected, advanced)
}

fn ensure_system_command(
    movement: &mut MovementState,
    command_id: &str,
    turn_number: u32,
    now_ms: u64,
) {
    if let Some(command) = movement
        .system_commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
    {
        if command.status == "pending" {
            command.status = "applying".to_string();
        }
        return;
    }

    movement.system_commands.push(MovementSystemCommandRecord {
        command_id: command_id.to_string(),
        session_id: movement.session_id.clone(),
        turn_number,
        idempotency_key: command_id.to_string(),
        status: "applying".to_string(),
        created_at_ms: now_ms,
        applied_at_ms: None,
        last_error: None,
    });
}

fn candidates_for_step(
    movement: &MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
) -> Result<Vec<MoveCandidate>, MovementError> {
    let mut candidates = Vec::new();
    for intent in movement
        .intents
        .iter()
        .filter(|intent| intent.turn_number == turn_number && intent.status == "pending")
    {
        let Some(to) = intent.path.get(usize::from(step_index)).copied() else {
            continue;
        };
        let champion = champions.champion(&intent.champion_id)?;
        if champion.status != "active" {
            continue;
        }
        let movement_cost = map
            .movement_cost_at(to.x, to.y)
            .ok_or(MovementError::OutOfBounds { x: to.x, y: to.y })?;
        let remaining_before = champions.effective_movement(&intent.champion_id, turn_number)?;
        candidates.push(MoveCandidate {
            intent_id: intent.intent_id.clone(),
            champion_id: intent.champion_id.clone(),
            participant_id: intent.participant_id.clone(),
            from: MoveCoord::new(champion.x, champion.y),
            to,
            movement_cost: u16::from(movement_cost),
            remaining_before,
            path_distance: step_index.saturating_add(1),
            tie_break: tile_conflict_tie_break(
                &movement.session_seed,
                turn_number,
                command_id,
                &intent.champion_id,
                to,
            ),
        });
    }
    candidates.sort_by(|left, right| left.champion_id.cmp(&right.champion_id));
    Ok(candidates)
}

fn resolve_microstep(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    candidates: Vec<MoveCandidate>,
) -> Result<MicrostepOutcome, MovementError> {
    let mut outcome = MicrostepOutcome::default();
    let mut active = candidates
        .into_iter()
        .map(|candidate| (candidate.champion_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();

    resolve_tile_conflicts(
        movement,
        &mut active,
        step_index,
        command_id,
        now_ms,
        &mut outcome,
    );
    resolve_crossing_conflicts(
        movement,
        champions,
        &mut active,
        turn_number,
        step_index,
        command_id,
        now_ms,
        &mut outcome,
    )?;
    resolve_blockers_and_interactions(
        movement,
        map,
        champions,
        &mut active,
        turn_number,
        step_index,
        command_id,
        now_ms,
        &mut outcome,
    )?;
    commit_remaining_moves(
        movement,
        map,
        champions,
        active,
        turn_number,
        step_index,
        command_id,
        now_ms,
        &mut outcome,
    )?;
    Ok(outcome)
}

fn resolve_tile_conflicts(
    movement: &mut MovementState,
    active: &mut BTreeMap<String, MoveCandidate>,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome: &mut MicrostepOutcome,
) {
    let mut by_tile: BTreeMap<MoveCoord, Vec<MoveCandidate>> = BTreeMap::new();
    for candidate in active.values() {
        by_tile
            .entry(candidate.to)
            .or_default()
            .push(candidate.clone());
    }

    for group in by_tile.values().filter(|group| group.len() > 1) {
        let winner = tile_conflict_winner(group);
        for candidate in group
            .iter()
            .filter(|candidate| candidate.champion_id != winner.champion_id)
        {
            active.remove(&candidate.champion_id);
            stop_candidate(
                movement,
                candidate,
                step_index,
                command_id,
                now_ms,
                "stopped_tile_conflict",
                None,
                outcome,
            );
        }
    }
}

fn resolve_crossing_conflicts(
    movement: &mut MovementState,
    champions: &mut ChampionState,
    active: &mut BTreeMap<String, MoveCandidate>,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    let ids = active.keys().cloned().collect::<Vec<_>>();
    let mut stopped = BTreeSet::new();
    for left_index in 0..ids.len() {
        for right_index in left_index + 1..ids.len() {
            let left_id = &ids[left_index];
            let right_id = &ids[right_index];
            if stopped.contains(left_id) || stopped.contains(right_id) {
                continue;
            }
            let Some(left) = active.get(left_id).cloned() else {
                continue;
            };
            let Some(right) = active.get(right_id).cloned() else {
                continue;
            };
            if left.to == right.from && right.to == left.from {
                let is_enemy = left.participant_id != right.participant_id;
                if is_enemy {
                    start_champion_battle(
                        movement,
                        champions,
                        &left,
                        &right.champion_id,
                        left.to,
                        command_id,
                        turn_number,
                        outcome,
                    )?;
                }
                active.remove(left_id);
                active.remove(right_id);
                stopped.insert(left_id.clone());
                stopped.insert(right_id.clone());
                stop_candidate(
                    movement,
                    &left,
                    step_index,
                    command_id,
                    now_ms,
                    if is_enemy {
                        "started_crossing_battle"
                    } else {
                        "stopped_crossing_conflict"
                    },
                    Some(MovementPathStop {
                        reason: "crossing_conflict".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: right.champion_id.clone(),
                        x: left.to.x,
                        y: left.to.y,
                    }),
                    outcome,
                );
                stop_candidate(
                    movement,
                    &right,
                    step_index,
                    command_id,
                    now_ms,
                    if is_enemy {
                        "started_crossing_battle"
                    } else {
                        "stopped_crossing_conflict"
                    },
                    Some(MovementPathStop {
                        reason: "crossing_conflict".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: left.champion_id.clone(),
                        x: right.to.x,
                        y: right.to.y,
                    }),
                    outcome,
                );
            }
        }
    }
    Ok(())
}

fn resolve_blockers_and_interactions(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    active: &mut BTreeMap<String, MoveCandidate>,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    let ids = active.keys().cloned().collect::<Vec<_>>();
    for champion_id in ids {
        let Some(candidate) = active.get(&champion_id).cloned() else {
            continue;
        };
        if candidate.movement_cost > candidate.remaining_before {
            active.remove(&champion_id);
            stop_candidate(
                movement,
                &candidate,
                step_index,
                command_id,
                now_ms,
                "stopped_budget_exhausted",
                None,
                outcome,
            );
            continue;
        }

        if let Some(blocker) = champion_blocker_at(champions, candidate.to, &candidate.champion_id)
        {
            active.remove(&champion_id);
            let blocker_candidate = active.remove(&blocker.champion_id);
            if blocker.participant_id != candidate.participant_id {
                start_champion_battle(
                    movement,
                    champions,
                    &candidate,
                    &blocker.champion_id,
                    candidate.to,
                    command_id,
                    turn_number,
                    outcome,
                )?;
                stop_candidate(
                    movement,
                    &candidate,
                    step_index,
                    command_id,
                    now_ms,
                    "started_champion_battle",
                    Some(MovementPathStop {
                        reason: "enemy_champion_blocker".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: blocker.champion_id.clone(),
                        x: candidate.to.x,
                        y: candidate.to.y,
                    }),
                    outcome,
                );
                if let Some(blocker_candidate) = blocker_candidate {
                    stop_candidate(
                        movement,
                        &blocker_candidate,
                        step_index,
                        command_id,
                        now_ms,
                        "started_champion_battle",
                        Some(MovementPathStop {
                            reason: "enemy_champion_blocker".to_string(),
                            subject_kind: "champion".to_string(),
                            subject_id_text: candidate.champion_id.clone(),
                            x: blocker_candidate.to.x,
                            y: blocker_candidate.to.y,
                        }),
                        outcome,
                    );
                }
            } else {
                stop_candidate(
                    movement,
                    &candidate,
                    step_index,
                    command_id,
                    now_ms,
                    "stopped_champion_blocker",
                    Some(MovementPathStop {
                        reason: "friendly_champion_blocker".to_string(),
                        subject_kind: "champion".to_string(),
                        subject_id_text: blocker.champion_id,
                        x: candidate.to.x,
                        y: candidate.to.y,
                    }),
                    outcome,
                );
            }
            continue;
        }

        if let Some(stop) = static_interaction_at(map, &candidate, candidate.to) {
            active.remove(&champion_id);
            match stop.subject_kind.as_str() {
                "neutral_army" => {
                    start_neutral_battle(
                        movement,
                        champions,
                        &candidate,
                        &stop.subject_id_text,
                        command_id,
                        turn_number,
                        outcome,
                    )?;
                    stop_candidate(
                        movement,
                        &candidate,
                        step_index,
                        command_id,
                        now_ms,
                        "started_neutral_battle",
                        Some(stop),
                        outcome,
                    );
                }
                "world_object" if object_stop_moves_onto_tile(map, &stop.subject_id_text) => {
                    apply_champion_move(
                        movement,
                        map,
                        champions,
                        &candidate,
                        turn_number,
                        step_index,
                        command_id,
                        now_ms,
                        "stopped_object_interaction",
                        Some(stop.clone()),
                        outcome,
                    )?;
                    mark_intent_resolved(
                        movement,
                        &candidate.intent_id,
                        command_id,
                        &mut outcome.resolved_intent_ids,
                        &mut outcome.gameplay_rows_written,
                    );
                    outcome.object_stops.push(ObjectStopDraft {
                        champion_id: candidate.champion_id.clone(),
                        object_id: stop.subject_id_text,
                        interaction_key: stop.reason,
                        x: candidate.to.x,
                        y: candidate.to.y,
                    });
                    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(1);
                }
                _ => {
                    stop_candidate(
                        movement,
                        &candidate,
                        step_index,
                        command_id,
                        now_ms,
                        "stopped_static_interaction",
                        Some(stop),
                        outcome,
                    );
                }
            }
        }
    }
    Ok(())
}

fn commit_remaining_moves(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    active: BTreeMap<String, MoveCandidate>,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    for candidate in active.values() {
        apply_champion_move(
            movement,
            map,
            champions,
            candidate,
            turn_number,
            step_index,
            command_id,
            now_ms,
            "moved",
            None,
            outcome,
        )?;
        if intent_path_is_finished(movement, &candidate.intent_id, step_index) {
            mark_intent_resolved(
                movement,
                &candidate.intent_id,
                command_id,
                &mut outcome.resolved_intent_ids,
                &mut outcome.gameplay_rows_written,
            );
        }
    }
    Ok(())
}

fn apply_champion_move(
    movement: &mut MovementState,
    map: &mut FirstPlayableMapState,
    champions: &mut ChampionState,
    candidate: &MoveCandidate,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome_text: &str,
    stop: Option<MovementPathStop>,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    let remaining_after = champions.spend_movement(
        &candidate.champion_id,
        turn_number,
        candidate.movement_cost,
        command_id,
    )?;
    {
        let champion = champions.champion_mut(&candidate.champion_id)?;
        champion.x = candidate.to.x;
        champion.y = candidate.to.y;
        champion.last_command_id = Some(command_id.to_string());
    }
    update_champion_map_position(map, &candidate.champion_id, candidate.to, command_id)?;
    let snapshot = push_snapshot(
        movement,
        candidate,
        step_index,
        command_id,
        now_ms,
        candidate.to,
        candidate.movement_cost,
        remaining_after,
        outcome_text,
        stop,
    );
    outcome.snapshots.push(snapshot);
    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(3);
    Ok(())
}

fn stop_candidate(
    movement: &mut MovementState,
    candidate: &MoveCandidate,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    outcome_text: &str,
    stop: Option<MovementPathStop>,
    outcome: &mut MicrostepOutcome,
) {
    let snapshot = push_snapshot(
        movement,
        candidate,
        step_index,
        command_id,
        now_ms,
        candidate.from,
        0,
        candidate.remaining_before,
        outcome_text,
        stop,
    );
    outcome.snapshots.push(snapshot);
    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(1);
    mark_intent_resolved(
        movement,
        &candidate.intent_id,
        command_id,
        &mut outcome.resolved_intent_ids,
        &mut outcome.gameplay_rows_written,
    );
}

fn push_snapshot(
    movement: &mut MovementState,
    candidate: &MoveCandidate,
    step_index: u16,
    command_id: &str,
    now_ms: u64,
    to: MoveCoord,
    movement_cost: u16,
    remaining_after: u16,
    outcome: &str,
    stop: Option<MovementPathStop>,
) -> MovementSnapshotRecord {
    let snapshot = MovementSnapshotRecord {
        snapshot_id: format!(
            "move-snapshot:{command_id}:{}:{step_index}:{}",
            candidate.champion_id,
            movement.snapshots.len()
        ),
        session_id: movement.session_id.clone(),
        command_id: command_id.to_string(),
        turn_number: candidate_turn(candidate, movement),
        champion_id: candidate.champion_id.clone(),
        participant_id: candidate.participant_id.clone(),
        intent_id: candidate.intent_id.clone(),
        step_index,
        from_x: candidate.from.x,
        from_y: candidate.from.y,
        to_x: to.x,
        to_y: to.y,
        movement_cost,
        remaining_after,
        outcome: outcome.to_string(),
        interaction_kind: stop.as_ref().map(|stop| stop.subject_kind.clone()),
        interaction_id_text: stop.as_ref().map(|stop| stop.subject_id_text.clone()),
        created_at_ms: now_ms,
    };
    movement.snapshots.push(snapshot.clone());
    snapshot
}

fn mark_intents_with_completed_paths(
    movement: &mut MovementState,
    turn_number: u32,
    step_index: u16,
    command_id: &str,
    outcome: &mut MovementSyncOutcome,
) {
    let intent_ids = movement
        .intents
        .iter()
        .filter(|intent| {
            intent.turn_number == turn_number
                && intent.status == "pending"
                && intent.path.len() <= usize::from(step_index)
        })
        .map(|intent| intent.intent_id.clone())
        .collect::<Vec<_>>();
    for intent_id in intent_ids {
        mark_intent_resolved(
            movement,
            &intent_id,
            command_id,
            &mut outcome.resolved_intent_ids,
            &mut outcome.gameplay_rows_written,
        );
    }
}

fn mark_intent_resolved(
    movement: &mut MovementState,
    intent_id: &str,
    command_id: &str,
    resolved_intent_ids: &mut Vec<String>,
    gameplay_rows_written: &mut u32,
) {
    if let Some(intent) = movement
        .intents
        .iter_mut()
        .find(|intent| intent.intent_id == intent_id && intent.status == "pending")
    {
        intent.status = "resolved".to_string();
        intent.resolved_command_id = Some(command_id.to_string());
        resolved_intent_ids.push(intent.intent_id.clone());
        *gameplay_rows_written = gameplay_rows_written.saturating_add(1);
    }
}

fn finalize_turn(
    movement: &mut MovementState,
    command_id: &str,
    from_turn: u32,
    now_ms: u64,
    outcome: &mut MovementSyncOutcome,
) {
    movement.partial_cursor = None;
    if let Some(command) = movement
        .system_commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
    {
        command.status = "applied".to_string();
        command.applied_at_ms = Some(now_ms);
        command.last_error = None;
    }
    if movement.current_turn == from_turn {
        movement.current_turn = movement.current_turn.saturating_add(1);
        movement.turn_started_at_ms = movement
            .turn_started_at_ms
            .saturating_add(movement.turn_duration_ms);
        outcome.advanced_turn = true;
    }
    outcome.current_turn = movement.current_turn;
    outcome.cursor = None;
    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(1);
}

fn park_cursor(
    movement: &mut MovementState,
    command_id: &str,
    turn_number: u32,
    next_step_index: u16,
    gameplay_rows_written: u32,
    outcome: &mut MovementSyncOutcome,
) {
    let cursor = MovementResolutionCursor {
        session_id: movement.session_id.clone(),
        turn_number,
        command_id: command_id.to_string(),
        next_step_index,
        gameplay_rows_written,
    };
    movement.partial_cursor = Some(cursor.clone());
    outcome.cursor = Some(cursor);
    outcome.current_turn = movement.current_turn;
}

fn merge_microstep(outcome: &mut MovementSyncOutcome, microstep: MicrostepOutcome) {
    outcome
        .resolved_intent_ids
        .extend(microstep.resolved_intent_ids);
    outcome.snapshots.extend(microstep.snapshots);
    outcome.battle_starts.extend(microstep.battle_starts);
    outcome.object_stops.extend(microstep.object_stops);
    outcome.gameplay_rows_written = outcome
        .gameplay_rows_written
        .saturating_add(microstep.gameplay_rows_written);
}

fn tile_conflict_winner(group: &[MoveCandidate]) -> MoveCandidate {
    let mut sorted = group.to_vec();
    sorted.sort_by(|left, right| compare_tile_conflict_candidates(left, right));
    sorted
        .into_iter()
        .next()
        .expect("tile conflict group should not be empty")
}

fn compare_tile_conflict_candidates(left: &MoveCandidate, right: &MoveCandidate) -> Ordering {
    right
        .remaining_before
        .cmp(&left.remaining_before)
        .then_with(|| left.path_distance.cmp(&right.path_distance))
        .then_with(|| right.tie_break.cmp(&left.tie_break))
        .then_with(|| left.champion_id.cmp(&right.champion_id))
}

fn champion_blocker_at(
    champions: &ChampionState,
    coord: MoveCoord,
    moving_champion_id: &str,
) -> Option<ChampionBlocker> {
    champions
        .champions
        .iter()
        .find(|champion| {
            champion.champion_id != moving_champion_id
                && champion.status != "defeated"
                && champion.x == coord.x
                && champion.y == coord.y
        })
        .map(|champion| ChampionBlocker {
            champion_id: champion.champion_id.clone(),
            participant_id: champion.participant_id.clone(),
        })
}

#[derive(Clone)]
struct ChampionBlocker {
    champion_id: String,
    participant_id: String,
}

fn static_interaction_at(
    map: &FirstPlayableMapState,
    candidate: &MoveCandidate,
    coord: MoveCoord,
) -> Option<MovementPathStop> {
    let subject = subject_at(map, coord)?;
    let reason = static_stop_reason(subject, &candidate.participant_id)?;
    Some(MovementPathStop {
        reason,
        subject_kind: subject.subject_kind.clone(),
        subject_id_text: subject.subject_id_text.clone(),
        x: coord.x,
        y: coord.y,
    })
}

fn subject_at(map: &FirstPlayableMapState, coord: MoveCoord) -> Option<&MapSubjectRecord> {
    map.subjects
        .iter()
        .filter(|subject| subject.x == coord.x && subject.y == coord.y)
        .find(|subject| matches!(subject.subject_kind.as_str(), "neutral_army" | "town"))
        .or_else(|| {
            map.subjects
                .iter()
                .filter(|subject| subject.x == coord.x && subject.y == coord.y)
                .find(|subject| {
                    subject.subject_kind == "world_object" && object_requires_stop(subject)
                })
        })
}

fn object_stop_moves_onto_tile(map: &FirstPlayableMapState, object_id: &str) -> bool {
    map.subjects
        .iter()
        .find(|subject| {
            subject.subject_kind == "world_object" && subject.subject_id_text == object_id
        })
        .is_some_and(object_requires_stop)
}

fn start_champion_battle(
    movement: &MovementState,
    champions: &mut ChampionState,
    attacker: &MoveCandidate,
    defender_champion_id: &str,
    coord: MoveCoord,
    command_id: &str,
    turn_number: u32,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    champions.set_champion_status(&attacker.champion_id, "in_battle", turn_number, command_id)?;
    champions.set_champion_status(defender_champion_id, "in_battle", turn_number, command_id)?;
    outcome.battle_starts.push(BattleStartDraft {
        battle_key: format!(
            "battle:{}:{turn_number}:{}:{defender_champion_id}",
            movement.session_id, attacker.champion_id
        ),
        battle_type: "champion".to_string(),
        attacker_champion_id: attacker.champion_id.clone(),
        defender_kind: "champion".to_string(),
        defender_id_text: defender_champion_id.to_string(),
        x: coord.x,
        y: coord.y,
    });
    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(3);
    Ok(())
}

fn start_neutral_battle(
    movement: &MovementState,
    champions: &mut ChampionState,
    attacker: &MoveCandidate,
    neutral_army_id: &str,
    command_id: &str,
    turn_number: u32,
    outcome: &mut MicrostepOutcome,
) -> Result<(), MovementError> {
    champions.set_champion_status(&attacker.champion_id, "in_battle", turn_number, command_id)?;
    outcome.battle_starts.push(BattleStartDraft {
        battle_key: format!(
            "battle:{}:{turn_number}:{}:{neutral_army_id}",
            movement.session_id, attacker.champion_id
        ),
        battle_type: "neutral".to_string(),
        attacker_champion_id: attacker.champion_id.clone(),
        defender_kind: "neutral_army".to_string(),
        defender_id_text: neutral_army_id.to_string(),
        x: attacker.to.x,
        y: attacker.to.y,
    });
    outcome.gameplay_rows_written = outcome.gameplay_rows_written.saturating_add(2);
    Ok(())
}

fn update_champion_map_position(
    map: &mut FirstPlayableMapState,
    champion_id: &str,
    coord: MoveCoord,
    command_id: &str,
) -> Result<(), MovementError> {
    map.cleanup_occupancy_by_subject("champion", champion_id);
    map.insert_occupancy_footprint(
        coord.x,
        coord.y,
        1,
        1,
        "champion",
        "champion",
        champion_id,
        true,
        Some(command_id.to_string()),
    )?;
    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.x = coord.x;
        subject.y = coord.y;
        subject.chunk_x = coord.x / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
        subject.chunk_y = coord.y / u16::from(FIRST_PLAYABLE_CHUNK_SIZE);
    }
    Ok(())
}

fn intent_path_is_finished(
    movement: &MovementState,
    intent_id: &str,
    completed_step_index: u16,
) -> bool {
    movement
        .intents
        .iter()
        .find(|intent| intent.intent_id == intent_id)
        .is_some_and(|intent| intent.path.len() <= usize::from(completed_step_index) + 1)
}

fn tile_conflict_tie_break(
    session_seed: &str,
    turn_number: u32,
    command_id: &str,
    champion_id: &str,
    coord: MoveCoord,
) -> u64 {
    hash64(&RollKey::new(
        session_seed,
        "movement.tile_conflict",
        turn_number,
        command_id,
        champion_id,
        format!("tile:{}:{}", coord.x, coord.y),
        0,
    ))
}

fn candidate_turn(candidate: &MoveCandidate, movement: &MovementState) -> u32 {
    movement
        .intents
        .iter()
        .find(|intent| intent.intent_id == candidate.intent_id)
        .map_or(movement.current_turn, |intent| intent.turn_number)
}
