use std::collections::BTreeMap;

use crate::champion::ChampionState;
use crate::economy::{EconomyState, ResourceApplyOutcome};
use crate::map::{FirstPlayableMapState, WorldObjectRecord};
use crate::movement::MovementSyncOutcome;

use super::types::{
    ChampionObjectVisitRecord, ObjectCommandEffectRecord, ObjectInteractionCommandRecord,
    ObjectInteractionOutcome, ObjectResourceOutcome, ObjectScoreRecord,
    ParticipantObjectVisitRecord, WorldObjectError, WorldObjectState, movement_object_command_id,
    object_command_id, object_payload_hash, visit_key_for,
};

pub fn interact_with_world_object(
    objects: &mut WorldObjectState,
    map: &mut FirstPlayableMapState,
    economy: &mut EconomyState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    client_nonce: u64,
    now_ms: u64,
) -> Result<ObjectInteractionOutcome, WorldObjectError> {
    let command_id = object_command_id(
        &objects.session_id,
        champion_id,
        object_id,
        current_turn,
        client_nonce,
    );
    let payload_hash = object_payload_hash(
        &objects.session_id,
        participant_id,
        champion_id,
        object_id,
        current_turn,
        client_nonce,
    );
    interact_with_world_object_command(
        objects,
        map,
        economy,
        champions,
        participant_id,
        champion_id,
        object_id,
        current_turn,
        client_nonce,
        now_ms,
        &command_id,
        &payload_hash,
    )
}

pub fn apply_movement_object_interactions(
    objects: &mut WorldObjectState,
    map: &mut FirstPlayableMapState,
    economy: &mut EconomyState,
    champions: &ChampionState,
    movement_outcome: &MovementSyncOutcome,
    current_turn: u32,
    now_ms: u64,
) -> Result<Vec<ObjectInteractionOutcome>, WorldObjectError> {
    let mut outcomes = Vec::new();
    for stop in &movement_outcome.object_stops {
        let champion = champions.champion(&stop.champion_id)?;
        let command_id = movement_object_command_id(
            &movement_outcome.command_id,
            &stop.champion_id,
            &stop.object_id,
        );
        let payload_hash = object_payload_hash(
            &objects.session_id,
            &champion.participant_id,
            &stop.champion_id,
            &stop.object_id,
            current_turn,
            0,
        );
        outcomes.push(interact_with_world_object_command(
            objects,
            map,
            economy,
            champions,
            &champion.participant_id,
            &stop.champion_id,
            &stop.object_id,
            current_turn,
            0,
            now_ms,
            &command_id,
            &payload_hash,
        )?);
    }
    Ok(outcomes)
}

fn interact_with_world_object_command(
    objects: &mut WorldObjectState,
    map: &mut FirstPlayableMapState,
    economy: &mut EconomyState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    client_nonce: u64,
    now_ms: u64,
    command_id: &str,
    payload_hash: &str,
) -> Result<ObjectInteractionOutcome, WorldObjectError> {
    let replay = begin_object_command(
        objects,
        command_id,
        participant_id,
        champion_id,
        object_id,
        client_nonce,
        payload_hash,
        now_ms,
    )?;
    if replay {
        return Ok(replay_outcome(objects, map, command_id, object_id));
    }

    let object = match validate_interaction(map, champions, participant_id, champion_id, object_id)
    {
        Ok(object) => object,
        Err(error) => {
            fail_object_command(objects, command_id, &error.to_string());
            return Err(error);
        }
    };
    let scoring_kind = normalized_scoring_kind(&object);
    let visit_key = visit_key_for(&scoring_kind, current_turn);
    ensure_effect(objects, command_id, "object.validate");

    let outcome_result = match scoring_kind.as_str() {
        "resource_pile" => apply_resource_pile(
            objects,
            map,
            economy,
            participant_id,
            champion_id,
            object_id,
            current_turn,
            command_id,
            &visit_key,
        ),
        "mine" => apply_capture(
            objects,
            map,
            economy,
            participant_id,
            champion_id,
            object_id,
            current_turn,
            command_id,
            &visit_key,
            "mine_capture",
            true,
        ),
        "central_objective" => apply_capture(
            objects,
            map,
            economy,
            participant_id,
            champion_id,
            object_id,
            current_turn,
            command_id,
            &visit_key,
            "central_objective_capture",
            false,
        ),
        other => {
            fail_object_command(objects, command_id, "unsupported_interaction");
            return Err(WorldObjectError::UnsupportedInteraction {
                object_id: object_id.to_string(),
                scoring_kind: other.to_string(),
            });
        }
    };
    let outcome = match outcome_result {
        Ok(outcome) => outcome,
        Err(error) => {
            fail_object_command(objects, command_id, &error.to_string());
            return Err(error);
        }
    };
    apply_object_command(objects, command_id, now_ms);
    Ok(outcome)
}

fn apply_resource_pile(
    objects: &mut WorldObjectState,
    map: &mut FirstPlayableMapState,
    economy: &mut EconomyState,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    command_id: &str,
    visit_key: &str,
) -> Result<ObjectInteractionOutcome, WorldObjectError> {
    ensure_not_visited_by_participant(objects, object_id, participant_id, visit_key, command_id)?;
    let participant_visit = record_participant_object_visit(
        objects,
        object_id,
        participant_id,
        visit_key,
        "resource_pickup",
        current_turn,
        command_id,
    )?;
    let champion_visit = record_champion_object_visit(
        objects,
        object_id,
        champion_id,
        visit_key,
        "resource_pickup",
        current_turn,
        command_id,
    )?;
    ensure_effect(objects, command_id, "resource.reward");
    let resource_outcome =
        economy.collect_resource_pile(participant_id, object_id, current_turn, command_id)?;
    mark_world_object_collected(map, object_id, participant_id, current_turn, command_id)?;

    Ok(ObjectInteractionOutcome {
        command_id: command_id.to_string(),
        object_id: object_id.to_string(),
        interaction_kind: "resource_pickup".to_string(),
        visit_key: visit_key.to_string(),
        duplicate_replay: false,
        participant_visit: Some(participant_visit),
        champion_visit: Some(champion_visit),
        resource_outcome: Some(resource_outcome.into()),
        captured_source_id: None,
        scores: world_object_scoreboard(map),
    })
}

fn apply_capture(
    objects: &mut WorldObjectState,
    map: &mut FirstPlayableMapState,
    economy: &mut EconomyState,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    current_turn: u32,
    command_id: &str,
    visit_key: &str,
    visit_kind: &str,
    captures_income: bool,
) -> Result<ObjectInteractionOutcome, WorldObjectError> {
    let participant_visit = record_participant_object_visit(
        objects,
        object_id,
        participant_id,
        visit_key,
        visit_kind,
        current_turn,
        command_id,
    )?;
    let champion_visit = record_champion_object_visit(
        objects,
        object_id,
        champion_id,
        visit_key,
        visit_kind,
        current_turn,
        command_id,
    )?;
    if captures_income {
        ensure_effect(objects, command_id, "income.capture");
        economy.capture_income_source(object_id, participant_id, current_turn, command_id)?;
    } else {
        ensure_effect(objects, command_id, "score.capture");
    }
    mark_world_object_captured(map, object_id, participant_id, current_turn, command_id)?;

    Ok(ObjectInteractionOutcome {
        command_id: command_id.to_string(),
        object_id: object_id.to_string(),
        interaction_kind: visit_kind.to_string(),
        visit_key: visit_key.to_string(),
        duplicate_replay: false,
        participant_visit: Some(participant_visit),
        champion_visit: Some(champion_visit),
        resource_outcome: None,
        captured_source_id: captures_income.then(|| object_id.to_string()),
        scores: world_object_scoreboard(map),
    })
}

pub fn record_participant_object_visit(
    objects: &mut WorldObjectState,
    object_id: &str,
    participant_id: &str,
    visit_key: &str,
    visit_kind: &str,
    visited_turn: u32,
    command_id: &str,
) -> Result<ParticipantObjectVisitRecord, WorldObjectError> {
    if let Some(existing) = objects.participant_visits.iter().find(|visit| {
        visit.object_id == object_id
            && visit.participant_id == participant_id
            && visit.visit_key == visit_key
    }) {
        if existing.command_id == command_id {
            return Ok(existing.clone());
        }
        return Err(WorldObjectError::ObjectAlreadyVisited {
            object_id: object_id.to_string(),
            visit_key: visit_key.to_string(),
        });
    }

    let visit = ParticipantObjectVisitRecord {
        visit_id: format!(
            "participant-visit:{}:{participant_id}:{visit_key}",
            object_id
        ),
        session_id: objects.session_id.clone(),
        object_id: object_id.to_string(),
        participant_id: participant_id.to_string(),
        visit_key: visit_key.to_string(),
        visit_kind: visit_kind.to_string(),
        visited_turn,
        command_id: command_id.to_string(),
    };
    objects.participant_visits.push(visit.clone());
    Ok(visit)
}

pub fn record_champion_object_visit(
    objects: &mut WorldObjectState,
    object_id: &str,
    champion_id: &str,
    visit_key: &str,
    visit_kind: &str,
    visited_turn: u32,
    command_id: &str,
) -> Result<ChampionObjectVisitRecord, WorldObjectError> {
    if let Some(existing) = objects.champion_visits.iter().find(|visit| {
        visit.object_id == object_id
            && visit.champion_id == champion_id
            && visit.visit_key == visit_key
    }) {
        if existing.command_id == command_id {
            return Ok(existing.clone());
        }
        return Err(WorldObjectError::ObjectAlreadyVisited {
            object_id: object_id.to_string(),
            visit_key: visit_key.to_string(),
        });
    }

    let visit = ChampionObjectVisitRecord {
        visit_id: format!("champion-visit:{}:{champion_id}:{visit_key}", object_id),
        session_id: objects.session_id.clone(),
        object_id: object_id.to_string(),
        champion_id: champion_id.to_string(),
        visit_key: visit_key.to_string(),
        visit_kind: visit_kind.to_string(),
        visited_turn,
        command_id: command_id.to_string(),
    };
    objects.champion_visits.push(visit.clone());
    Ok(visit)
}

pub fn world_object_scoreboard(map: &FirstPlayableMapState) -> Vec<ObjectScoreRecord> {
    let mut scores: BTreeMap<(String, Option<String>), u32> = BTreeMap::new();
    for object in &map.world_objects {
        let scoring_kind = normalized_scoring_kind(object);
        if matches!(scoring_kind.as_str(), "none" | "resource_pile") {
            continue;
        }
        *scores
            .entry((scoring_kind, object.owner_participant_id.clone()))
            .or_default() += 1;
    }
    scores
        .into_iter()
        .map(
            |((scoring_kind, owner_participant_id), object_count)| ObjectScoreRecord {
                scoring_kind,
                owner_participant_id,
                object_count,
            },
        )
        .collect()
}

fn begin_object_command(
    objects: &mut WorldObjectState,
    command_id: &str,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
    client_nonce: u64,
    payload_hash: &str,
    now_ms: u64,
) -> Result<bool, WorldObjectError> {
    if let Some(existing) = objects
        .commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
    {
        if existing.payload_hash != payload_hash {
            return Err(WorldObjectError::DuplicateNoncePayloadMismatch { client_nonce });
        }
        if existing.status == "applied" {
            return Ok(true);
        }
        existing.status = "applying".to_string();
        return Ok(false);
    }

    objects.commands.push(ObjectInteractionCommandRecord {
        command_id: command_id.to_string(),
        session_id: objects.session_id.clone(),
        participant_id: participant_id.to_string(),
        champion_id: champion_id.to_string(),
        object_id: object_id.to_string(),
        client_nonce,
        payload_hash: payload_hash.to_string(),
        status: "applying".to_string(),
        created_at_ms: now_ms,
        applied_at_ms: None,
        last_error: None,
    });
    Ok(false)
}

fn apply_object_command(objects: &mut WorldObjectState, command_id: &str, now_ms: u64) {
    if let Some(command) = objects
        .commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
    {
        command.status = "applied".to_string();
        command.applied_at_ms = Some(now_ms);
        command.last_error = None;
    }
}

fn fail_object_command(objects: &mut WorldObjectState, command_id: &str, error: &str) {
    if let Some(command) = objects
        .commands
        .iter_mut()
        .find(|command| command.command_id == command_id)
    {
        command.status = "failed".to_string();
        command.last_error = Some(error.to_string());
    }
}

fn ensure_effect(objects: &mut WorldObjectState, command_id: &str, effect_key: &str) {
    let effect_id = format!("object-effect:{command_id}:{effect_key}");
    if objects
        .effects
        .iter()
        .any(|effect| effect.effect_id == effect_id)
    {
        return;
    }
    objects.effects.push(ObjectCommandEffectRecord {
        effect_id,
        command_id: command_id.to_string(),
        effect_key: effect_key.to_string(),
        status: "applied".to_string(),
    });
}

fn validate_interaction(
    map: &FirstPlayableMapState,
    champions: &ChampionState,
    participant_id: &str,
    champion_id: &str,
    object_id: &str,
) -> Result<WorldObjectRecord, WorldObjectError> {
    let champion = champions.champion(champion_id)?;
    if champion.participant_id != participant_id {
        return Err(WorldObjectError::ChampionNotOwned {
            participant_id: participant_id.to_string(),
            champion_id: champion_id.to_string(),
        });
    }
    let object = map
        .world_objects
        .iter()
        .find(|object| object.object_id == object_id)
        .ok_or_else(|| WorldObjectError::ObjectNotFound {
            object_id: object_id.to_string(),
        })?;
    if !map.is_visible_at(participant_id, object.x, object.y) {
        return Err(WorldObjectError::ObjectNotVisible {
            participant_id: participant_id.to_string(),
            object_id: object_id.to_string(),
        });
    }
    if champion.x != object.x || champion.y != object.y {
        return Err(WorldObjectError::ChampionNotOnObject {
            champion_id: champion_id.to_string(),
            object_id: object_id.to_string(),
        });
    }
    if let Some(guard_id) = object.guarded_neutral_army_id.as_deref() {
        if map.subjects.iter().any(|subject| {
            subject.subject_kind == "neutral_army"
                && subject.subject_id_text == guard_id
                && subject.state != "defeated"
        }) {
            return Err(WorldObjectError::ObjectGuarded {
                object_id: object_id.to_string(),
                guard_id: guard_id.to_string(),
            });
        }
    }
    Ok(object.clone())
}

fn ensure_not_visited_by_participant(
    objects: &WorldObjectState,
    object_id: &str,
    participant_id: &str,
    visit_key: &str,
    command_id: &str,
) -> Result<(), WorldObjectError> {
    if let Some(existing) = objects.participant_visits.iter().find(|visit| {
        visit.object_id == object_id
            && visit.participant_id == participant_id
            && visit.visit_key == visit_key
    }) {
        if existing.command_id == command_id {
            return Ok(());
        }
        return Err(WorldObjectError::ObjectAlreadyVisited {
            object_id: object_id.to_string(),
            visit_key: visit_key.to_string(),
        });
    }
    Ok(())
}

fn mark_world_object_collected(
    map: &mut FirstPlayableMapState,
    object_id: &str,
    participant_id: &str,
    current_turn: u32,
    command_id: &str,
) -> Result<(), WorldObjectError> {
    update_world_object(
        map,
        object_id,
        Some(participant_id),
        "collected",
        current_turn,
        command_id,
    )?;
    map.cleanup_occupancy_by_subject("world_object", object_id);
    Ok(())
}

fn mark_world_object_captured(
    map: &mut FirstPlayableMapState,
    object_id: &str,
    participant_id: &str,
    current_turn: u32,
    command_id: &str,
) -> Result<(), WorldObjectError> {
    update_world_object(
        map,
        object_id,
        Some(participant_id),
        "captured",
        current_turn,
        command_id,
    )
}

fn update_world_object(
    map: &mut FirstPlayableMapState,
    object_id: &str,
    owner_participant_id: Option<&str>,
    state: &str,
    _current_turn: u32,
    _command_id: &str,
) -> Result<(), WorldObjectError> {
    let object = map
        .world_objects
        .iter_mut()
        .find(|object| object.object_id == object_id)
        .ok_or_else(|| WorldObjectError::ObjectNotFound {
            object_id: object_id.to_string(),
        })?;
    object.owner_participant_id = owner_participant_id.map(ToString::to_string);
    object.state = state.to_string();
    object.public_json = world_object_public_json(object);
    object.redacted_json = world_object_redacted_json(object);

    if let Some(subject) = map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "world_object" && subject.subject_id_text == object_id
    }) {
        subject.owner_participant_id = owner_participant_id.map(ToString::to_string);
        subject.state = state.to_string();
        subject.public_json = object.public_json.clone();
        subject.redacted_json = object.redacted_json.clone();
        subject.scoring_kind = object.scoring_kind.clone();
    }
    Ok(())
}

fn replay_outcome(
    objects: &WorldObjectState,
    map: &FirstPlayableMapState,
    command_id: &str,
    object_id: &str,
) -> ObjectInteractionOutcome {
    let command = objects
        .commands
        .iter()
        .find(|command| command.command_id == command_id);
    let participant_visit = command.and_then(|command| {
        objects
            .participant_visits
            .iter()
            .find(|visit| {
                visit.command_id == command_id && visit.participant_id == command.participant_id
            })
            .cloned()
    });
    let champion_visit = command.and_then(|command| {
        objects
            .champion_visits
            .iter()
            .find(|visit| {
                visit.command_id == command_id && visit.champion_id == command.champion_id
            })
            .cloned()
    });
    ObjectInteractionOutcome {
        command_id: command_id.to_string(),
        object_id: object_id.to_string(),
        interaction_kind: participant_visit
            .as_ref()
            .map_or_else(|| "replay".to_string(), |visit| visit.visit_kind.clone()),
        visit_key: participant_visit
            .as_ref()
            .map_or_else(|| "once".to_string(), |visit| visit.visit_key.clone()),
        duplicate_replay: true,
        participant_visit,
        champion_visit,
        resource_outcome: None,
        captured_source_id: None,
        scores: world_object_scoreboard(map),
    }
}

fn normalized_scoring_kind(object: &WorldObjectRecord) -> String {
    match object.scoring_kind.as_deref() {
        Some("objective") => "central_objective".to_string(),
        Some(value) => value.to_string(),
        None => "none".to_string(),
    }
}

fn world_object_public_json(object: &WorldObjectRecord) -> String {
    format!(
        "{{\"type\":\"world_object\",\"object_id\":\"{}\",\"object_type\":\"{}\",\"scoring_kind\":\"{}\",\"interaction_key\":\"{}\",\"state\":\"{}\"}}",
        escape_json(&object.object_id),
        escape_json(&object.object_type),
        escape_json(object.scoring_kind.as_deref().unwrap_or("none")),
        interaction_key(object),
        escape_json(&object.state)
    )
}

fn world_object_redacted_json(object: &WorldObjectRecord) -> String {
    format!(
        "{{\"type\":\"world_object\",\"object_id\":\"{}\",\"object_type\":\"{}\",\"state\":\"last_known\"}}",
        escape_json(&object.object_id),
        escape_json(&object.object_type)
    )
}

fn interaction_key(object: &WorldObjectRecord) -> &'static str {
    match object.object_type.as_str() {
        "mine" if object.object_slug == "crystal-mine" => "capture_crystal_income",
        "mine" => "capture_gold_income",
        "central_objective" => "score_central_objective",
        "resource_pile" => "grant_resource_reward",
        _ => "inspect",
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

impl From<ResourceApplyOutcome> for ObjectResourceOutcome {
    fn from(value: ResourceApplyOutcome) -> Self {
        Self {
            ledger_rows_touched: value.ledger_rows_touched as u32,
            balance_updates: value.balance_updates as u32,
            skipped_applied_rows: value.skipped_applied_rows as u32,
            budget_exhausted: value.budget_exhausted,
        }
    }
}
