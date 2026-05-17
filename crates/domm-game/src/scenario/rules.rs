use crate::economy::week_for_turn;
use crate::rng::{RollKey, hash64};

use super::types::{
    MAX_ADVANCED_VICTORY_CHECKS_PER_UPDATE, OPENING_QUEST_KEY, OPENING_QUEST_OBJECTIVE_KEY,
    OPENING_QUEST_REWARD_GOLD, OPENING_QUEST_TITLE, QuestMutation, QuestProgressView,
    ScenarioRuleView, WorldEventView,
};

#[must_use]
pub fn quest_accept_transition(status: &str) -> QuestMutation {
    match status {
        "available" => QuestMutation {
            allowed: true,
            next_status: "accepted".to_string(),
            reward_gold_delta: 0,
            disabled_reason: None,
        },
        "accepted" | "completed" => QuestMutation {
            allowed: false,
            next_status: status.to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_already_accepted".to_string()),
        },
        "claimed" => QuestMutation {
            allowed: false,
            next_status: "claimed".to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_already_claimed".to_string()),
        },
        _ => QuestMutation {
            allowed: false,
            next_status: status.to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_unavailable".to_string()),
        },
    }
}

#[must_use]
pub fn quest_claim_transition(
    status: &str,
    progress_value: u32,
    required_value: u32,
    reward_gold: u32,
) -> QuestMutation {
    if status == "claimed" {
        return QuestMutation {
            allowed: false,
            next_status: "claimed".to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_reward_already_claimed".to_string()),
        };
    }
    if !matches!(status, "accepted" | "completed") {
        return QuestMutation {
            allowed: false,
            next_status: status.to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_not_accepted".to_string()),
        };
    }
    if progress_value < required_value {
        return QuestMutation {
            allowed: false,
            next_status: status.to_string(),
            reward_gold_delta: 0,
            disabled_reason: Some("quest_incomplete".to_string()),
        };
    }
    QuestMutation {
        allowed: true,
        next_status: "claimed".to_string(),
        reward_gold_delta: reward_gold,
        disabled_reason: None,
    }
}

#[must_use]
pub fn opening_quest_view(
    participant_id: &str,
    status: &str,
    progress_value: u32,
) -> QuestProgressView {
    QuestProgressView {
        quest_key: OPENING_QUEST_KEY.to_string(),
        title: OPENING_QUEST_TITLE.to_string(),
        participant_id: participant_id.to_string(),
        objective_key: OPENING_QUEST_OBJECTIVE_KEY.to_string(),
        status: status.to_string(),
        progress_value,
        required_value: 1,
        reward_gold: Some(OPENING_QUEST_REWARD_GOLD),
        reward_claimed: status == "claimed",
        accepted_turn: 0,
        claimed_turn: 0,
        redacted: false,
    }
}

#[must_use]
pub fn redact_quest_for_viewer(
    mut quest: QuestProgressView,
    viewer_participant_id: &str,
) -> QuestProgressView {
    if quest.participant_id != viewer_participant_id {
        quest.progress_value = 0;
        quest.reward_gold = None;
        quest.redacted = true;
    }
    quest
}

#[must_use]
pub fn objective_status(progress_value: u32, required_value: u32) -> &'static str {
    if progress_value >= required_value {
        "complete"
    } else {
        "active"
    }
}

#[must_use]
pub fn world_event_window_for_turn(turn_number: u32) -> (u32, String, u32, u32) {
    let week = week_for_turn(turn_number);
    let starts_turn = week.saturating_sub(1).saturating_mul(7).saturating_add(1);
    let ends_turn = starts_turn.saturating_add(6);
    (week, format!("week:{week}"), starts_turn, ends_turn)
}

#[must_use]
pub fn deterministic_world_event_key(session_seed: u64, turn_number: u32) -> String {
    let (week, _, _, _) = world_event_window_for_turn(turn_number);
    let roll_key = RollKey::new(
        &session_seed.to_string(),
        "world_event",
        week,
        format!("week:{week}"),
        "first_playable",
        "scenario_event",
        0,
    );
    format!("world:event:week:{week}:{:016x}", hash64(&roll_key))
}

#[must_use]
pub fn deterministic_world_event(session_seed: u64, turn_number: u32) -> WorldEventView {
    let (_, event_window, starts_turn, ends_turn) = world_event_window_for_turn(turn_number);
    WorldEventView {
        event_key: deterministic_world_event_key(session_seed, turn_number),
        event_type: "weekly_omen".to_string(),
        event_window,
        starts_turn,
        ends_turn,
        status: "active".to_string(),
        payload: Some("{\"omen\":\"ledger_smoke\",\"visibility\":\"public\"}".to_string()),
        redacted: false,
    }
}

#[must_use]
pub fn max_turn_rule_state(current_turn: u32, max_turns: u32) -> &'static str {
    if current_turn >= max_turns {
        "max_turn_reached"
    } else {
        "active"
    }
}

#[must_use]
pub fn bounded_victory_checks(rules: &[ScenarioRuleView]) -> Vec<ScenarioRuleView> {
    rules
        .iter()
        .take(MAX_ADVANCED_VICTORY_CHECKS_PER_UPDATE as usize)
        .cloned()
        .collect()
}
