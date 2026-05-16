use crate::fixtures::first_playable_fixture;
use crate::playable::{PlayableGateReport, run_first_playable_backend_gate};

use super::audit::part_two_spec_audit;
use super::movement_conflict::run_e2e_movement_conflict_probe;
use super::types::{
    EndToEndCoverage, EndToEndError, EndToEndFirstPlayableReport, EndToEndMeasurements,
    ManualSmokeCommand, MovementConflictReport,
};

pub fn run_first_playable_e2e_fixture() -> Result<EndToEndFirstPlayableReport, EndToEndError> {
    let backend_gate = run_first_playable_backend_gate()?;
    let movement_conflict = run_e2e_movement_conflict_probe()?;
    let fixture = first_playable_fixture();

    Ok(EndToEndFirstPlayableReport {
        session_id: backend_gate.session_id.clone(),
        coverage: coverage(
            &backend_gate,
            &movement_conflict,
            &fixture.ids.participant_one_id,
        ),
        measurements: measurements(&backend_gate),
        movement_conflict,
        backend_gate,
        spec_audit: part_two_spec_audit(),
        manual_smoke_commands: manual_smoke_commands(),
    })
}

fn coverage(
    backend_gate: &PlayableGateReport,
    movement_conflict: &MovementConflictReport,
    winning_participant_id: &str,
) -> EndToEndCoverage {
    let step_keys = backend_gate
        .strategic
        .step_views
        .iter()
        .map(|step| step.step_key.as_str())
        .collect::<Vec<_>>();
    EndToEndCoverage {
        exploration: step_keys.iter().any(|step| *step == "started"),
        pickup: step_keys.iter().any(|step| *step == "pickup"),
        building: step_keys.iter().any(|step| *step == "built"),
        recruitment: step_keys.iter().any(|step| *step == "recruited"),
        movement_conflict: movement_conflict.stopped_tile_conflict,
        battle: backend_gate.strategic.final_view.neutral_encounter_count > 0
            && backend_gate.event_page.events_returned > 0,
        town_capture: backend_gate.final_view.captured_town_owner == winning_participant_id,
        victory: backend_gate.final_view.final_session_state == "finished"
            && backend_gate.final_view.winner_participant_id.as_deref()
                == Some(winning_participant_id),
    }
}

fn measurements(backend_gate: &PlayableGateReport) -> EndToEndMeasurements {
    EndToEndMeasurements {
        command_count: backend_gate.command_count,
        event_count: backend_gate.event_count,
        query_count: backend_gate.query_count,
        storage_row_count: backend_gate.storage_row_count,
        max_query_bytes: backend_gate.max_query_bytes,
        estimated_response_bytes: backend_gate.max_query_bytes,
        recovery_retry_count: backend_gate.recovery_retry_count,
    }
}

fn manual_smoke_commands() -> Vec<ManualSmokeCommand> {
    vec![
        ManualSmokeCommand {
            label: "End-to-end fixture".to_string(),
            command: "make smoke-e2e".to_string(),
            expected: "checkpoint_19_e2e_fixture_covers_first_playable_scope passes and prints fixture metrics".to_string(),
        },
        ManualSmokeCommand {
            label: "Playable web client".to_string(),
            command: "cargo test -p domm-client-probe gate_e -- --nocapture".to_string(),
            expected: "Gate E client walkthrough completes the first playable web route".to_string(),
        },
        ManualSmokeCommand {
            label: "Backend victory gate".to_string(),
            command: "cargo test -p domm-game gate_d_backend_fixture_reaches_victory_from_public_calls -- --nocapture".to_string(),
            expected: "Gate D backend route reaches victory with stable command, event, query, and row counts".to_string(),
        },
        ManualSmokeCommand {
            label: "Workspace regression".to_string(),
            command: "make regression".to_string(),
            expected: "all workspace tests pass from the DoMM repo root".to_string(),
        },
    ]
}
