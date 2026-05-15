use crate::rng::{RollKey, hash64};

use super::types::{
    AI_MAX_ACTORS_PER_UPDATE, AI_MAX_CANDIDATES_PER_ACTOR, AI_MAX_EMITTED_COMMANDS_PER_UPDATE,
    AiCandidate, AiCommandDraft, AiDecisionInput, AiError, AiUpdateReport,
};

pub fn run_ai_update(
    inputs: &[AiDecisionInput],
    max_actors: u16,
    command_cap: u16,
) -> Result<AiUpdateReport, AiError> {
    let actor_limit = max_actors.min(AI_MAX_ACTORS_PER_UPDATE);
    let command_limit = command_cap.min(AI_MAX_EMITTED_COMMANDS_PER_UPDATE);
    if actor_limit == 0 || command_limit == 0 {
        return Ok(AiUpdateReport {
            actor_count: inputs.len().min(u16::MAX as usize) as u16,
            actors_processed: 0,
            candidates_considered: 0,
            emitted_commands: Vec::new(),
            budget_exhausted: true,
            cursor_json: Some("{\"reason\":\"ai_budget_exhausted_before_emit\"}".to_string()),
            no_available_reason: Some("ai_budget_exhausted_before_emit".to_string()),
        });
    }

    let mut emitted_commands = Vec::new();
    let mut candidates_considered = 0_u16;
    let mut actors_processed = 0_u16;
    let mut no_available_reason = None;

    for input in inputs.iter().take(actor_limit as usize) {
        if emitted_commands.len() as u16 >= command_limit {
            break;
        }
        actors_processed = actors_processed.saturating_add(1);
        let actor_report = decide_for_actor(input)?;
        candidates_considered =
            candidates_considered.saturating_add(actor_report.candidates_considered);
        if actor_report.emitted_commands.is_empty() {
            no_available_reason = actor_report.no_available_reason;
            continue;
        }
        emitted_commands.push(actor_report.emitted_commands[0].clone());
    }

    let actor_budget_exhausted = (inputs.len() as u16) > actor_limit;
    let command_budget_exhausted =
        (inputs.len() as u16) > actors_processed && emitted_commands.len() as u16 >= command_limit;
    let budget_exhausted = actor_budget_exhausted || command_budget_exhausted;
    Ok(AiUpdateReport {
        actor_count: inputs.len().min(u16::MAX as usize) as u16,
        actors_processed,
        candidates_considered,
        emitted_commands,
        budget_exhausted,
        cursor_json: budget_exhausted.then(|| {
            format!(
                "{{\"next_actor_index\":{},\"reason\":\"ai_update_budget_exhausted\"}}",
                actors_processed
            )
        }),
        no_available_reason,
    })
}

pub fn decide_for_actor(input: &AiDecisionInput) -> Result<AiUpdateReport, AiError> {
    validate_actor_kind(&input.actor.actor_kind)?;
    let mut candidates = Vec::new();
    add_battle_candidates(input, &mut candidates);
    add_strategic_candidates(input, &mut candidates);

    if candidates.is_empty() {
        candidates.push(AiCandidate {
            command_kind: "NoOp".to_string(),
            target_id_text: None,
            payload_summary: "{\"reason\":\"no_legal_ai_candidate\"}".to_string(),
            priority_score: 0,
            candidate_key: "noop".to_string(),
        });
    }

    candidates.truncate(AI_MAX_CANDIDATES_PER_ACTOR as usize);
    candidates.sort_by(|left, right| {
        right
            .priority_score
            .cmp(&left.priority_score)
            .then_with(|| candidate_tie_break(input, right).cmp(&candidate_tie_break(input, left)))
            .then_with(|| left.candidate_key.cmp(&right.candidate_key))
    });

    let selected = candidates
        .first()
        .expect("no-op candidate is inserted when no candidate exists");
    let command = command_from_candidate(input, selected);
    Ok(AiUpdateReport {
        actor_count: 1,
        actors_processed: 1,
        candidates_considered: candidates.len() as u16,
        emitted_commands: vec![command],
        budget_exhausted: false,
        cursor_json: None,
        no_available_reason: (selected.command_kind == "NoOp")
            .then(|| "no_legal_ai_candidate".to_string()),
    })
}

fn validate_actor_kind(actor_kind: &str) -> Result<(), AiError> {
    match actor_kind {
        "neutral_army" | "autopilot_participant" => Ok(()),
        other => Err(AiError::UnsupportedActorKind {
            actor_kind: other.to_string(),
        }),
    }
}

fn add_battle_candidates(input: &AiDecisionInput, candidates: &mut Vec<AiCandidate>) {
    let Some(view) = &input.battle_view else {
        return;
    };
    if view.battle_state != "active" || view.active_stack_id.is_none() {
        return;
    }
    let stack_id = view.active_stack_id.clone().unwrap_or_default();
    candidates.push(AiCandidate {
        command_kind: "BattleDefend".to_string(),
        target_id_text: Some(stack_id.clone()),
        payload_summary: format!(
            "{{\"battle_id\":\"{}\",\"action\":\"Defend\",\"stack_id\":\"{}\"}}",
            escape_json(&view.battle_id),
            escape_json(&stack_id)
        ),
        priority_score: if view.legal_action_count > 0 { 120 } else { 10 },
        candidate_key: format!("battle:defend:{}", view.battle_id),
    });
}

fn add_strategic_candidates(input: &AiDecisionInput, candidates: &mut Vec<AiCandidate>) {
    let Some(view) = &input.strategic_view else {
        return;
    };
    if view.sync_required {
        candidates.push(AiCandidate {
            command_kind: "SyncTurn".to_string(),
            target_id_text: Some(view.session_id.clone()),
            payload_summary: format!("{{\"session_id\":\"{}\"}}", escape_json(&view.session_id)),
            priority_score: 110,
            candidate_key: "strategic:sync_turn".to_string(),
        });
    }
    if view.recruit_pool_available >= 4 {
        candidates.push(AiCandidate {
            command_kind: "RecruitUnits".to_string(),
            target_id_text: Some("town:west".to_string()),
            payload_summary:
                "{\"town_id\":\"town:west\",\"unit_slug\":\"mudhook-levy\",\"quantity\":4}"
                    .to_string(),
            priority_score: 90,
            candidate_key: "town:recruit:mudhook-levy".to_string(),
        });
    }
    if !view
        .built_buildings
        .iter()
        .any(|building| building == "freehold-training-yard")
        && view.resources.gold >= 1_000
    {
        candidates.push(AiCandidate {
            command_kind: "BuildTownStructure".to_string(),
            target_id_text: Some("town:west".to_string()),
            payload_summary:
                "{\"town_id\":\"town:west\",\"building_slug\":\"freehold-training-yard\"}"
                    .to_string(),
            priority_score: 80,
            candidate_key: "town:build:freehold-training-yard".to_string(),
        });
    }
    if view.pending_battle_key.is_some() {
        candidates.push(AiCandidate {
            command_kind: "SyncBattle".to_string(),
            target_id_text: view.pending_battle_key.clone(),
            payload_summary: "{\"reason\":\"pending_battle_key\"}".to_string(),
            priority_score: 70,
            candidate_key: "battle:sync_pending".to_string(),
        });
    }
    if view.champion_status == "active" {
        candidates.push(AiCandidate {
            command_kind: "SubmitMoveIntent".to_string(),
            target_id_text: Some(view.champion_id.clone()),
            payload_summary: format!(
                "{{\"champion_id\":\"{}\",\"profile\":\"safe_short_move\"}}",
                escape_json(&view.champion_id)
            ),
            priority_score: 40,
            candidate_key: "movement:safe_short_move".to_string(),
        });
    }
}

fn command_from_candidate(input: &AiDecisionInput, candidate: &AiCandidate) -> AiCommandDraft {
    AiCommandDraft {
        actor_kind: "ai".to_string(),
        actor_id_text: input.actor.actor_id_text.clone(),
        command_kind: candidate.command_kind.clone(),
        target_id_text: candidate.target_id_text.clone(),
        payload_summary: candidate.payload_summary.clone(),
        client_nonce: deterministic_nonce(input, candidate),
        priority_score: candidate.priority_score,
    }
}

fn deterministic_nonce(input: &AiDecisionInput, candidate: &AiCandidate) -> String {
    format!(
        "ai:{}:{:016x}",
        input.turn_number,
        hash64(&roll_key(
            input,
            "ai_client_nonce",
            &candidate.candidate_key,
            0
        ))
    )
}

fn candidate_tie_break(input: &AiDecisionInput, candidate: &AiCandidate) -> u64 {
    hash64(&roll_key(
        input,
        "ai_candidate_tie_break",
        &candidate.candidate_key,
        0,
    ))
}

fn roll_key(
    input: &AiDecisionInput,
    domain_key: &str,
    target_id_text: &str,
    roll_index: u32,
) -> RollKey {
    RollKey::new(
        input.session_seed.clone(),
        domain_key,
        input.turn_number,
        "system:ai",
        input.actor.actor_id_text.clone(),
        target_id_text,
        roll_index,
    )
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
