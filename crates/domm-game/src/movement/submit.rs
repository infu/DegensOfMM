use crate::champion::ChampionState;
use crate::limits::MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN;
use crate::map::FirstPlayableMapState;

use super::preview::validate_move_path;
use super::types::{
    MoveCoord, MovementError, MovementIntentRecord, MovementIntentSubmitOutcome, MovementState,
    movement_intent_command_id, movement_intent_id, movement_payload_hash,
};

pub fn submit_move_intent(
    movement: &mut MovementState,
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    client_nonce: u64,
    submitted_at_ms: u64,
) -> Result<MovementIntentSubmitOutcome, MovementError> {
    let preview = validate_move_path(
        movement,
        map,
        champions,
        participant_id,
        champion_id,
        path.clone(),
        submitted_at_ms,
    )?;
    let payload_hash = movement_payload_hash(
        &movement.session_id,
        participant_id,
        champion_id,
        movement.current_turn,
        client_nonce,
        &path,
    );
    let command_id = movement_intent_command_id(
        &movement.session_id,
        champion_id,
        movement.current_turn,
        client_nonce,
    );

    if let Some(existing) = movement.intents.iter().find(|intent| {
        intent.turn_number == movement.current_turn
            && intent.champion_id == champion_id
            && intent.client_nonce == client_nonce
    }) {
        if existing.payload_hash != payload_hash {
            return Err(MovementError::DuplicateNoncePayloadMismatch {
                champion_id: champion_id.to_string(),
                client_nonce,
            });
        }
        return Ok(MovementIntentSubmitOutcome {
            intent: existing.clone(),
            preview,
            replaced_intent_ids: Vec::new(),
            command_id,
        });
    }

    let unresolved_intents = movement
        .intents
        .iter()
        .filter(|intent| intent.turn_number == movement.current_turn && intent.status == "pending")
        .count();
    let replaces_pending_intent = movement.intents.iter().any(|intent| {
        intent.turn_number == movement.current_turn
            && intent.champion_id == champion_id
            && intent.status == "pending"
    });
    if unresolved_intents >= MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN as usize
        && !replaces_pending_intent
    {
        return Err(MovementError::UnresolvedIntentLimitExceeded {
            turn_number: movement.current_turn,
            max_intents: MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN,
        });
    }

    let intent_id = movement_intent_id(
        &movement.session_id,
        champion_id,
        movement.current_turn,
        client_nonce,
    );
    let mut replaced_intent_ids = Vec::new();
    for intent in movement.intents.iter_mut().filter(|intent| {
        intent.turn_number == movement.current_turn
            && intent.champion_id == champion_id
            && intent.status == "pending"
    }) {
        intent.status = "superseded".to_string();
        intent.superseded_by_intent_id = Some(intent_id.clone());
        replaced_intent_ids.push(intent.intent_id.clone());
    }

    let intent = MovementIntentRecord {
        intent_id,
        session_id: movement.session_id.clone(),
        participant_id: participant_id.to_string(),
        champion_id: champion_id.to_string(),
        turn_number: movement.current_turn,
        client_nonce,
        payload_hash,
        path: preview.path.clone(),
        status: "pending".to_string(),
        submitted_at_ms,
        superseded_by_intent_id: None,
        resolved_command_id: None,
        last_command_id: command_id.clone(),
    };
    movement.intents.push(intent.clone());

    Ok(MovementIntentSubmitOutcome {
        intent,
        preview,
        replaced_intent_ids,
        command_id,
    })
}
