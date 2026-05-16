use super::backend::PlayableFixtureBackend;
use super::driver::run_first_playable_backend_gate;
use super::types::PlayableCall;
use crate::fixtures::first_playable_fixture;
use crate::strategic::{StrategicFixtureBackend, StrategicHeadlessDriver};

#[test]
fn gate_d_backend_fixture_reaches_victory_from_public_calls() {
    let report = run_first_playable_backend_gate().expect("backend gate should pass");
    let fixture = first_playable_fixture();

    assert_eq!(report.session_id, fixture.ids.session_id);
    assert_eq!(report.final_view.final_session_state, "finished");
    assert_eq!(
        report.final_view.winner_participant_id.as_deref(),
        Some(fixture.ids.participant_one_id.as_str())
    );
    assert_eq!(report.final_view.defeated_neutral_state, "defeated");
    assert_eq!(
        report.final_view.captured_town_owner,
        fixture.ids.participant_one_id
    );
    assert_eq!(report.final_view.defeated_champion_status, "defeated");
    assert_eq!(report.final_view.match_summary_count, 2);
    assert_eq!(report.final_view.match_history_count, 2);

    assert_eq!(report.strategic.final_view.neutral_encounter_count, 1);
    assert!(report.recovery_retry_count >= 1);
    assert!(report.event_page.events_returned > 0);
    assert_eq!(
        report.event_page.total_event_count,
        report.final_view.event_count
    );
    assert_eq!(report.command_count, 32);
    assert_eq!(report.event_count, 42);
    assert_eq!(report.query_count, 19);
    assert_eq!(report.storage_row_count, 193);
    assert_eq!(report.max_query_bytes, 5144);
    assert!(report.concerns.is_empty());
}

#[test]
fn gate_d_driver_records_only_public_backend_calls() {
    let fixture = first_playable_fixture();
    let strategic_backend = StrategicFixtureBackend::new(fixture.clone());
    let mut strategic_driver = StrategicHeadlessDriver::new(strategic_backend, fixture.clone());
    let strategic = strategic_driver
        .run_first_playable_gate()
        .expect("strategic gate should pass");
    let strategic_backend = strategic_driver.into_backend();
    let mut backend = PlayableFixtureBackend::from_strategic(fixture.clone(), strategic_backend);
    let caller = fixture.principals.player_one;

    let prepared = backend
        .prepare_battle_public(caller, &strategic.session_id)
        .expect("battle should prepare");
    let battle_id = prepared.battle_id.expect("prepared battle id");
    let battle = backend
        .inspect_battle_public(caller, &battle_id, 481_000)
        .expect("battle should inspect");
    let active_stack_id = battle.active_stack_id.expect("active stack id");
    backend
        .submit_battle_action_public(
            caller,
            &battle_id,
            &active_stack_id,
            "Defend",
            None,
            None,
            "gate-d-public-call-retry",
            480_000,
        )
        .expect("first action should apply");
    let retry = backend
        .submit_battle_action_public(
            caller,
            &battle_id,
            &active_stack_id,
            "Defend",
            None,
            None,
            "gate-d-public-call-retry",
            480_001,
        )
        .expect("retry should replay");
    assert!(retry.replayed);

    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, PlayableCall::Strategic(_)))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, PlayableCall::PrepareBattle { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, PlayableCall::InspectBattle { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, PlayableCall::SubmitBattleAction { .. }))
    );
}
