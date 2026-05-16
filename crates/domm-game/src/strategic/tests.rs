use super::backend::StrategicFixtureBackend;
use super::driver::{StrategicHeadlessDriver, run_first_playable_strategic_gate};
use super::types::StrategicCall;
use crate::fixtures::first_playable_fixture;

#[test]
fn gate_c_strategic_loop_runs_through_public_backend_surface() {
    let report = run_first_playable_strategic_gate().expect("strategic gate should pass");

    assert_eq!(report.session_id, "fixture-session-first-playable");
    assert_eq!(report.step_views.len(), 6);

    let started = step(&report, "started");
    assert_eq!(started.current_turn, 1);
    assert_eq!(started.champion_status, "active");
    assert!(started.visible_chunk_count > 0);
    assert!(started.visible_object_count > 0);

    let pickup = step(&report, "pickup");
    assert_eq!(pickup.champion_x, 9);
    assert_eq!(pickup.champion_y, 23);
    assert_eq!(pickup.resources.wood, 15);
    assert_eq!(pickup.object_command_count, 1);

    let income = step(&report, "income");
    assert_eq!(income.current_turn, 3);
    assert!(income.resources.gold > pickup.resources.gold);

    assert!(
        step(&report, "built")
            .built_buildings
            .contains(&"freehold-training-yard".to_string())
    );
    assert_eq!(step(&report, "recruited").town_garrison_quantity, 4);

    let battle = step(&report, "battle_trigger");
    assert_eq!(battle.champion_status, "in_battle");
    assert_eq!(battle.neutral_encounter_count, 1);
    assert!(
        battle
            .pending_battle_key
            .as_deref()
            .is_some_and(|key| key.contains("neutral:west-mine"))
    );

    assert_eq!(report.command_count, 22);
    assert_eq!(report.event_count, 36);
    assert_eq!(report.query_count, 15);
    assert_eq!(report.max_query_bytes, 5184);
    assert!(report.concerns.is_empty());
}

#[test]
fn strategic_driver_uses_public_commands_and_queries_only() {
    let fixture = first_playable_fixture();
    let backend = StrategicFixtureBackend::new(fixture.clone());
    let mut driver = StrategicHeadlessDriver::new(backend, fixture);

    let report = driver
        .run_first_playable_gate()
        .expect("strategic gate should pass");
    let backend = driver.into_backend();

    assert_eq!(report.final_view.neutral_encounter_count, 1);
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, StrategicCall::SubmitMoveIntent { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, StrategicCall::ApplyMovementObjects { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, StrategicCall::BuildTownStructure { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, StrategicCall::RecruitUnits { .. }))
    );
    assert!(
        backend
            .calls()
            .iter()
            .any(|call| matches!(call, StrategicCall::ApplyNeutralEncounters { .. }))
    );
    assert!(matches!(
        backend.calls().last(),
        Some(StrategicCall::InspectView { .. })
    ));
}

fn step<'a>(
    report: &'a super::types::StrategicGateReport,
    step_key: &str,
) -> &'a super::types::StrategicGameView {
    &report
        .step_views
        .iter()
        .find(|step| step.step_key == step_key)
        .unwrap_or_else(|| panic!("missing step {step_key}"))
        .view
}
