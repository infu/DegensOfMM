use candid::Principal;
use domm_game::{CommandStatus, FIRST_PLAYABLE_RULESET_ID, LobbyCommandResult};
use icydb::types::{Id, Ulid};

use super::account_lobby_session;
use crate::repos::sessions;

fn bootstrap_service_memory() {
    icydb::__reexports::canic_memory::api::MemoryApi::bootstrap_owner_range(
        "domm-degens-canister",
        20,
        120,
    )
    .expect("service tests should reserve the generated canister memory range");
}

#[test]
fn lobby_session_setup_recovers_from_starting_state_and_replays_nonce() {
    bootstrap_service_memory();

    let player_one = Principal::self_authenticating(b"service-19d-player-one");
    let player_two = Principal::self_authenticating(b"service-19d-player-two");

    let registered = account_lobby_session::register_player(
        player_one,
        Some("service-19d-one".to_string()),
        Some("Service One".to_string()),
        "nonce:service:register:one".to_string(),
    )
    .expect("player one registration should not trap");
    assert_eq!(registered.status, CommandStatus::Applied);
    let replay = account_lobby_session::register_player(
        player_one,
        Some("service-19d-one".to_string()),
        Some("Service One".to_string()),
        "nonce:service:register:one".to_string(),
    )
    .expect("registration replay should not trap");
    assert_eq!(replay.command_id, registered.command_id);

    account_lobby_session::register_player(
        player_two,
        Some("service-19d-two".to_string()),
        Some("Service Two".to_string()),
        "nonce:service:register:two".to_string(),
    )
    .expect("player two registration should not trap");

    let created = account_lobby_session::create_session(
        player_one,
        "Service 19D Match".to_string(),
        FIRST_PLAYABLE_RULESET_ID.to_string(),
        19_004,
        "nonce:service:create".to_string(),
    )
    .expect("session creation should not trap");
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    account_lobby_session::join_session(
        player_two,
        session_id.clone(),
        "faction:ashen-ledger".to_string(),
        "nonce:service:join".to_string(),
    )
    .expect("join should not trap");
    account_lobby_session::mark_ready(
        player_one,
        session_id.clone(),
        "nonce:service:ready:one".to_string(),
    )
    .expect("player one ready should not trap");
    account_lobby_session::mark_ready(
        player_two,
        session_id.clone(),
        "nonce:service:ready:two".to_string(),
    )
    .expect("player two ready should not trap");

    let session_key = Ulid::from_str(&session_id).expect("service session ids are Ulids");
    let mut session = sessions::load_session(Id::from_key(session_key))
        .expect("session load should not fail")
        .expect("session row should exist");
    session.state = "starting".to_string();
    sessions::update_session(session).expect("starting state update should persist");

    let started = account_lobby_session::start_session(
        player_one,
        session_id.clone(),
        "nonce:service:start".to_string(),
    )
    .expect("start recovery should not trap");
    assert_eq!(started.status, CommandStatus::Applied);
    let started_command_id = started.command_id.clone();
    match started.result {
        LobbyCommandResult::Session(session) => assert_eq!(session.state, "active"),
        other => panic!("start_session returned unexpected result: {other:?}"),
    }

    let start_replay = account_lobby_session::start_session(
        player_one,
        session_id.clone(),
        "nonce:service:start".to_string(),
    )
    .expect("start replay should not trap");
    assert_eq!(start_replay.command_id, started_command_id);

    let participant_two = account_lobby_session::get_my_participant(player_two, session_id)
        .expect("participant two should be readable after recovery");
    assert_eq!(participant_two.slot_index, 1);
    assert!(participant_two.ready);
}
