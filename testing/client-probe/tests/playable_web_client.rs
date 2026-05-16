use domm_client_probe::PlayableWebClient;
use domm_game::{FixtureApiBackend, first_playable_fixture};

#[test]
fn gate_e_web_client_plays_first_match_path_and_renders_required_panels() {
    let fixture = first_playable_fixture();
    let backend = FixtureApiBackend::new(fixture.clone());
    let mut client =
        PlayableWebClient::new(backend, fixture.clone(), fixture.principals.player_one);

    client
        .play_first_playable_walkthrough()
        .expect("web client should play the first match path");

    let state = client.state();
    let view = client.view_model();

    assert!(state.checklist_complete());
    assert!(state.rematch_available);
    assert!(state.retry_replays >= 3);
    assert!(state.match_result.is_some());
    assert!(state.match_history.entries_returned >= 1);
    assert!(
        state
            .command_log
            .iter()
            .all(|entry| entry.error_code.is_none())
    );
    assert_replayed(state, "submit_move_intent");
    assert_replayed(state, "sync_session_turn");
    assert_replayed(state, "submit_battle_action");
    assert_command(state, "submit_build_town_structure");
    assert_command(state, "submit_recruit_units");
    assert_command(state, "sync_battle");

    assert_eq!(view.screen, "result");
    assert_eq!(view.map_rows.len(), 24);
    assert!(view.map_rows.iter().all(|row| row.len() == 24));
    assert!(view.map_rows.iter().any(|row| row.contains('C')));
    assert!(view.map_rows.iter().any(|row| row.contains('T')));
    assert!(view.resources.contains("gold="));
    assert!(
        view.champion_panel.as_deref().is_some_and(|panel| {
            panel.contains("Mara of the Toll") && panel.contains("movement")
        })
    );
    assert!(
        view.town_panel
            .as_deref()
            .is_some_and(|panel| { panel.contains("West Woe") && panel.contains("buildings=") })
    );
    assert!(
        view.battle_panel
            .as_deref()
            .is_some_and(|panel| { panel.contains("neutral") || panel.contains("battle") })
    );
    assert!(view.command_status.is_some());
    assert!(
        view.event_feed
            .iter()
            .any(|event| event.contains("session_started"))
    );
    assert!(view.checklist.iter().all(|row| row.ends_with(":complete")));
    assert!(
        view.match_result
            .as_deref()
            .is_some_and(|result| { result.contains("win") || result.contains("finished") })
    );
    assert!(!view.match_history.is_empty());
    assert!(view.rematch_available);
}

fn assert_command(state: &domm_client_probe::WebClientState, command_type: &str) {
    assert!(
        state
            .command_log
            .iter()
            .any(|entry| entry.command_type == command_type),
        "missing command {command_type}; log={:?}",
        state.command_log
    );
}

fn assert_replayed(state: &domm_client_probe::WebClientState, command_type: &str) {
    assert!(
        state
            .command_log
            .iter()
            .any(|entry| entry.command_type == command_type && entry.replayed),
        "missing replayed command {command_type}; log={:?}",
        state.command_log
    );
}
