use super::{
    OPENING_QUEST_REWARD_GOLD, ScenarioRuleView, bounded_victory_checks,
    deterministic_world_event_key, max_turn_rule_state, objective_status, opening_quest_view,
    quest_accept_transition, quest_claim_transition, redact_quest_for_viewer,
    world_event_window_for_turn,
};

#[test]
fn objective_status_is_bounded_and_progress_based() {
    assert_eq!(objective_status(0, 1), "active");
    assert_eq!(objective_status(1, 1), "complete");
    assert_eq!(objective_status(u32::MAX, 1), "complete");
}

#[test]
fn quest_accept_and_reward_claim_are_idempotent_by_status() {
    let accepted = quest_accept_transition("available");
    assert!(accepted.allowed);
    assert_eq!(accepted.next_status, "accepted");

    let repeated_accept = quest_accept_transition("accepted");
    assert!(!repeated_accept.allowed);
    assert_eq!(
        repeated_accept.disabled_reason.as_deref(),
        Some("quest_already_accepted")
    );

    let accept_claimed = quest_accept_transition("claimed");
    assert!(!accept_claimed.allowed);
    assert_eq!(accept_claimed.next_status, "claimed");
    assert_eq!(
        accept_claimed.disabled_reason.as_deref(),
        Some("quest_already_claimed")
    );

    let premature = quest_claim_transition("accepted", 0, 1, OPENING_QUEST_REWARD_GOLD);
    assert!(!premature.allowed);
    assert_eq!(
        premature.disabled_reason.as_deref(),
        Some("quest_incomplete")
    );

    let claimed = quest_claim_transition("accepted", 1, 1, OPENING_QUEST_REWARD_GOLD);
    assert!(claimed.allowed);
    assert_eq!(claimed.next_status, "claimed");
    assert_eq!(claimed.reward_gold_delta, OPENING_QUEST_REWARD_GOLD);

    let repeated_claim = quest_claim_transition("claimed", 1, 1, OPENING_QUEST_REWARD_GOLD);
    assert!(!repeated_claim.allowed);
    assert_eq!(repeated_claim.reward_gold_delta, 0);
    assert_eq!(
        repeated_claim.disabled_reason.as_deref(),
        Some("quest_reward_already_claimed")
    );
}

#[test]
fn quest_view_redacts_progress_and_reward_from_other_participants() {
    let own = opening_quest_view("participant:one", "accepted", 1);
    let redacted = redact_quest_for_viewer(own, "participant:two");

    assert!(redacted.redacted);
    assert_eq!(redacted.progress_value, 0);
    assert_eq!(redacted.reward_gold, None);
}

#[test]
fn world_event_keys_are_deterministic_by_seed_and_week_window() {
    let key = deterministic_world_event_key(42, 1);
    assert_eq!(key, deterministic_world_event_key(42, 7));
    assert_ne!(key, deterministic_world_event_key(42, 8));
    assert_ne!(key, deterministic_world_event_key(43, 1));
    assert_eq!(
        world_event_window_for_turn(8),
        (2, "week:2".to_string(), 8, 14)
    );
}

#[test]
fn victory_checks_are_capped_and_max_turn_state_is_explicit() {
    let rules = (0..16)
        .map(|index| ScenarioRuleView {
            rule_key: format!("rule:{index}"),
            rule_type: "test".to_string(),
            status: "active".to_string(),
            victory_state: "active".to_string(),
            required_value: 1,
            current_value: 0,
            owner_participant_id: None,
            winner_participant_id: None,
            disabled_reason: None,
            last_checked_turn: 1,
        })
        .collect::<Vec<_>>();

    assert_eq!(bounded_victory_checks(&rules).len(), 8);
    assert_eq!(max_turn_rule_state(29, 30), "active");
    assert_eq!(max_turn_rule_state(30, 30), "max_turn_reached");
}
