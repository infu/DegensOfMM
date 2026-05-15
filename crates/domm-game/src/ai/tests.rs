use super::decision::{decide_for_actor, run_ai_update};
use super::types::{AiActorStateRecord, AiDecisionInput, AiError};
use crate::fixtures::first_playable_fixture;
use crate::playable::PlayableBattleView;
use crate::strategic::run_first_playable_strategic_gate;

#[test]
fn same_visible_state_produces_same_ai_command() {
    let input = strategic_input("ai:autopilot:one");

    let first = decide_for_actor(&input).expect("AI should decide");
    let second = decide_for_actor(&input).expect("AI should decide deterministically");

    assert_eq!(first, second);
    assert_eq!(first.emitted_commands.len(), 1);
    assert_eq!(first.emitted_commands[0].command_kind, "BuildTownStructure");
    assert!(first.emitted_commands[0].client_nonce.starts_with("ai:1:"));
}

#[test]
fn battle_ai_prefers_defend_from_legal_battle_view() {
    let mut input = strategic_input("ai:neutral:west-mine");
    input.actor.actor_kind = "neutral_army".to_string();
    input.battle_view = Some(PlayableBattleView {
        battle_id: "battle:fixture:west-neutral".to_string(),
        battle_state: "active".to_string(),
        active_stack_id: Some("battle-stack:fixture:west-neutral:defender:0".to_string()),
        legal_action_count: 3,
        event_count: 0,
    });

    let report = decide_for_actor(&input).expect("battle AI should decide");

    assert_eq!(report.emitted_commands[0].command_kind, "BattleDefend");
    assert_eq!(
        report.emitted_commands[0].target_id_text.as_deref(),
        Some("battle-stack:fixture:west-neutral:defender:0")
    );
}

#[test]
fn ai_update_enforces_actor_and_command_caps() {
    let inputs = vec![
        strategic_input("ai:autopilot:one"),
        strategic_input("ai:autopilot:two"),
        strategic_input("ai:autopilot:three"),
    ];

    let report = run_ai_update(&inputs, 3, 3).expect("AI update should run");

    assert_eq!(report.actor_count, 3);
    assert_eq!(report.actors_processed, 2);
    assert_eq!(report.emitted_commands.len(), 2);
    assert!(report.budget_exhausted);
    assert_eq!(
        report.cursor_json.as_deref(),
        Some("{\"next_actor_index\":2,\"reason\":\"ai_update_budget_exhausted\"}")
    );
}

#[test]
fn no_available_action_emits_legal_noop() {
    let fixture = first_playable_fixture();
    let input = AiDecisionInput {
        session_id: fixture.ids.session_id,
        session_seed: fixture.scenario_seed,
        turn_number: 4,
        actor: actor("ai:autopilot:idle", "autopilot_participant"),
        strategic_view: None,
        battle_view: None,
    };

    let report = decide_for_actor(&input).expect("AI should no-op");

    assert_eq!(report.emitted_commands[0].command_kind, "NoOp");
    assert_eq!(
        report.no_available_reason.as_deref(),
        Some("no_legal_ai_candidate")
    );
}

#[test]
fn zero_budget_fails_closed_without_emitting_commands() {
    let input = strategic_input("ai:autopilot:one");

    let report = run_ai_update(&[input], 1, 0).expect("AI should fail closed");

    assert!(report.budget_exhausted);
    assert!(report.emitted_commands.is_empty());
    assert_eq!(report.actors_processed, 0);
}

#[test]
fn unsupported_actor_kind_is_rejected() {
    let mut input = strategic_input("ai:unsupported");
    input.actor.actor_kind = "full_bot_participant".to_string();

    let err = decide_for_actor(&input).expect_err("unsupported actor should fail");

    assert!(matches!(
        err,
        AiError::UnsupportedActorKind { actor_kind } if actor_kind == "full_bot_participant"
    ));
}

fn strategic_input(actor_id_text: &str) -> AiDecisionInput {
    let fixture = first_playable_fixture();
    let strategic = run_first_playable_strategic_gate().expect("strategic gate should run");
    let view = strategic
        .step_views
        .iter()
        .find(|step| step.step_key == "started")
        .expect("started view exists")
        .view
        .clone();
    AiDecisionInput {
        session_id: fixture.ids.session_id,
        session_seed: fixture.scenario_seed,
        turn_number: view.current_turn,
        actor: actor(actor_id_text, "autopilot_participant"),
        strategic_view: Some(view),
        battle_view: None,
    }
}

fn actor(actor_id_text: &str, actor_kind: &str) -> AiActorStateRecord {
    let fixture = first_playable_fixture();
    AiActorStateRecord {
        session_id: fixture.ids.session_id,
        actor_id_text: actor_id_text.to_string(),
        actor_kind: actor_kind.to_string(),
        participant_id: fixture.ids.participant_two_id,
        profile_key: "first_playable_simple".to_string(),
        cursor_json: None,
        last_turn_processed: 0,
        last_command_nonce: None,
    }
}
