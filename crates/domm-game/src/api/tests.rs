use candid::{Decode, Encode};

use crate::api::{ApiError, FixtureApiBackend, GameView, GameViewRequest};
use crate::command::CommandStatus;
use crate::content::{FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION};
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::movement::MoveCoord;
use crate::town::RecruitTarget;

#[test]
fn public_api_starts_session_and_returns_renderable_game_view() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();

    let player = backend
        .get_my_player(fixture.principals.player_one)
        .expect("registered player should be readable");
    let participant = backend
        .get_my_participant(fixture.principals.player_one, &session.session_id)
        .expect("participant should be readable");
    let view = backend
        .get_default_game_view(fixture.principals.player_one, &session.session_id)
        .expect("game view should be readable");
    let manifest = backend
        .get_content_manifest(FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION)
        .expect("content manifest should be readable");

    assert_eq!(player.player_id, fixture.ids.player_one_id);
    assert_eq!(participant.participant_id, fixture.ids.participant_one_id);
    assert_eq!(view.session.session_id, fixture.ids.session_id);
    assert_eq!(view.map_chunks.len(), 4);
    assert!(
        view.objects
            .iter()
            .any(|object| object.subject_id_text == "champion:west")
    );
    assert!(
        view.events
            .iter()
            .any(|event| event.event_type == "session_started")
    );
    assert_eq!(manifest.manifest.ruleset.slug, FIRST_PLAYABLE_RULESET_SLUG);
    assert!(!view.content_manifest_hash.is_empty());
}

#[test]
fn command_responses_replay_same_nonce_and_reject_payload_mismatch() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());

    let first = backend.register_player(fixture.principals.player_one, "One", "nonce:test");
    let replay = backend.register_player(fixture.principals.player_one, "One", "nonce:test");
    let mismatch = backend.register_player(fixture.principals.player_one, "Two", "nonce:test");

    assert_eq!(first.command_id, replay.command_id);
    assert_eq!(first.payload_hash, replay.payload_hash);
    assert_eq!(
        mismatch.error.as_ref().map(|error| error.code.as_str()),
        Some("duplicate_nonce_payload_mismatch")
    );

    let session = backend.start_first_playable_session();
    let move_first = backend.submit_move_intent(
        fixture.principals.player_one,
        &session.session_id,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:move:retry",
        1_000,
    );
    let move_replay = backend.submit_move_intent(
        fixture.principals.player_one,
        &session.session_id,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:move:retry",
        1_000,
    );
    let move_mismatch = backend.submit_move_intent(
        fixture.principals.player_one,
        &session.session_id,
        "champion:west",
        vec![MoveCoord::new(9, 24)],
        "nonce:move:retry",
        1_000,
    );

    assert_eq!(move_first.command_id, move_replay.command_id);
    assert_eq!(move_first.status, CommandStatus::Applied);
    assert_eq!(
        move_mismatch
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("duplicate_nonce_payload_mismatch")
    );
    assert!(backend.get_command_status("nonce:move:retry").is_some());
}

#[test]
fn queries_page_events_and_redact_private_payloads() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();
    let participant_one_audience = format!("participant:{}", fixture.ids.participant_one_id);
    let participant_two_audience = format!("participant:{}", fixture.ids.participant_two_id);

    let viewport = GameViewRequest::opening_for_slot(0).viewport;
    let chunks = backend
        .get_visible_map_chunks(
            fixture.principals.player_one,
            &session.session_id,
            &viewport,
            None,
            1,
        )
        .expect("chunk page should load");
    assert_eq!(chunks.chunks.len(), 1);
    assert!(chunks.has_more);

    backend.submit_move_intent(
        fixture.principals.player_one,
        &session.session_id,
        "champion:west",
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:move:redaction",
        1_000,
    );
    let p1_events = backend.get_events_after(&session.session_id, &participant_one_audience, 0, 64);
    let p2_events = backend.get_events_after(&session.session_id, &participant_two_audience, 0, 64);
    let p1_move = p1_events
        .events
        .iter()
        .find(|event| event.event_type == "submit_move_intent")
        .expect("p1 move event should be present");
    let p2_move = p2_events
        .events
        .iter()
        .find(|event| event.event_type == "submit_move_intent")
        .expect("p2 redacted move event should be present");

    assert!(p1_move.payload.is_some());
    assert!(p2_move.redacted);
    assert!(p2_move.payload.is_none());
    assert_eq!(
        backend
            .get_champion_view(
                fixture.principals.player_one,
                &session.session_id,
                "champion:east"
            )
            .expect_err("enemy champion should not be visible")
            .code,
        "not_visible"
    );
}

#[test]
fn previews_and_api_dtos_are_candid_stable() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureApiBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();

    let move_preview = backend
        .preview_move(
            fixture.principals.player_one,
            &session.session_id,
            "champion:west",
            vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
            1_000,
        )
        .expect("move preview should be readable");
    let build_preview = backend
        .preview_build_town_structure(
            fixture.principals.player_one,
            &session.session_id,
            "town:west",
            "freehold-training-yard",
            3,
        )
        .expect("build preview should be readable");
    let recruit_preview = backend
        .preview_recruit_units(
            fixture.principals.player_one,
            &session.session_id,
            "town:west",
            "mudhook-levy",
            1,
            &RecruitTarget::TownGarrison { slot_index: None },
            3,
        )
        .expect("recruit preview should be readable");
    let response = backend.sync_session_turn(
        fixture.principals.player_one,
        &session.session_id,
        TURN_DURATION_MS,
        "nonce:sync:candid",
    );
    let game_view = backend
        .get_default_game_view(fixture.principals.player_one, &session.session_id)
        .expect("game view should be readable");

    assert_eq!(move_preview.path.len(), 2);
    assert_eq!(build_preview.town_id, "town:west");
    assert_eq!(recruit_preview.town_id, "town:west");

    let encoded_view = Encode!(&game_view).expect("game view should encode");
    let decoded_view =
        Decode!(&encoded_view, GameView).expect("game view should decode from candid");
    assert_eq!(
        decoded_view.session.session_id,
        game_view.session.session_id
    );

    let encoded_response = Encode!(&response).expect("command response should encode");
    let decoded_response = Decode!(&encoded_response, crate::api::CommandResponse)
        .expect("command response should decode from candid");
    assert_eq!(decoded_response.command_id, response.command_id);

    let error = ApiError::new("example", "example error", false);
    let encoded_error = Encode!(&error).expect("api error should encode");
    let decoded_error =
        Decode!(&encoded_error, ApiError).expect("api error should decode from candid");
    assert_eq!(decoded_error.code, "example");
}
