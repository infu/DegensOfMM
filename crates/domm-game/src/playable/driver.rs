use crate::battle::BATTLE_ACTION_DEADLINE_MS;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::strategic::{StrategicFixtureBackend, StrategicHeadlessDriver};

use super::backend::PlayableFixtureBackend;
use super::types::{PlayableError, PlayableGateReport};

pub fn run_first_playable_backend_gate() -> Result<PlayableGateReport, PlayableError> {
    let fixture = first_playable_fixture();
    let strategic_backend = StrategicFixtureBackend::new(fixture.clone());
    let mut strategic_driver = StrategicHeadlessDriver::new(strategic_backend, fixture.clone());
    let strategic = strategic_driver.run_first_playable_gate()?;
    let strategic_backend = strategic_driver.into_backend();
    let mut backend = PlayableFixtureBackend::from_strategic(fixture.clone(), strategic_backend);
    let caller = fixture.principals.player_one;

    let prepared = backend.prepare_battle_public(caller, &strategic.session_id)?;
    let battle_id = prepared
        .battle_id
        .clone()
        .ok_or(PlayableError::MissingAftermathState)?;
    let battle_view =
        backend.inspect_battle_public(caller, &battle_id, TURN_DURATION_MS * 8 + 1_000)?;
    let active_stack_id = battle_view
        .active_stack_id
        .ok_or(PlayableError::MissingAftermathState)?;

    let first_action_at = TURN_DURATION_MS * 8;
    let first = backend.submit_battle_action_public(
        caller,
        &battle_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "gate-d-defend",
        first_action_at,
    )?;
    let retry = backend.submit_battle_action_public(
        caller,
        &battle_id,
        &active_stack_id,
        "Defend",
        None,
        None,
        "gate-d-defend",
        first_action_at + 1,
    )?;
    if first.command_id != retry.command_id || !retry.replayed {
        return Err(PlayableError::RetryDidNotReplay);
    }

    backend.sync_battle_public(
        caller,
        &battle_id,
        first_action_at + u64::from(BATTLE_ACTION_DEADLINE_MS),
    )?;
    let neutral_battle = backend.resolve_neutral_battle_public(caller)?;
    let neutral_battle_id = neutral_battle
        .battle_id
        .clone()
        .ok_or(PlayableError::MissingAftermathState)?;
    backend.apply_battle_aftermath_public(
        caller,
        &neutral_battle_id,
        "command:gate-d:neutral-aftermath",
        1_800_000_600_000,
    )?;

    let town = backend.resolve_town_capture_public(caller)?;
    let town_battle_id = town
        .battle_id
        .clone()
        .ok_or(PlayableError::MissingAftermathState)?;
    backend.apply_battle_aftermath_public(
        caller,
        &town_battle_id,
        "command:gate-d:town-aftermath",
        1_800_000_610_000,
    )?;

    let champion = backend.resolve_champion_defeat_public(caller)?;
    let champion_battle_id = champion
        .battle_id
        .clone()
        .ok_or(PlayableError::MissingAftermathState)?;
    backend.apply_battle_aftermath_public(
        caller,
        &champion_battle_id,
        "command:gate-d:champion-aftermath",
        1_800_000_620_000,
    )?;

    let event_page = backend.refresh_events_public(caller, 0, 128)?;
    let final_view = backend.inspect_match_public(caller)?;
    let concerns = gate_concerns(&final_view);

    Ok(PlayableGateReport {
        session_id: strategic.session_id.clone(),
        strategic,
        recovery_retry_count: final_view.recovery_retry_count,
        command_count: final_view.command_count,
        event_count: final_view.event_count,
        query_count: final_view.query_count,
        storage_row_count: final_view.storage_row_count,
        max_query_bytes: final_view.max_query_bytes,
        concerns,
        event_page,
        final_view,
    })
}

fn gate_concerns(final_view: &super::types::PlayableMatchView) -> Vec<String> {
    let mut concerns = Vec::new();
    if final_view.max_query_bytes > 24_000 {
        concerns.push("max_query_bytes_above_24k".to_string());
    }
    if final_view.storage_row_count > 512 {
        concerns.push("storage_rows_above_512".to_string());
    }
    concerns
}
