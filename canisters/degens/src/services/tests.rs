use candid::Principal;
use domm_degens_schema::schema::{Battle, GameParticipant};
use domm_game::{CommandStatus, FIRST_PLAYABLE_RULESET_ID, LobbyCommandResult, MoveCoord};
use icydb::{
    traits::EntityValue,
    types::{Id, Ulid},
};

use super::{account_lobby_session, command_response, movement as movement_service};
use crate::repos::{
    battles, champions_artifacts, commands_events_effects, movement as movement_repo, sessions,
};

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

    let mut final_start = None;
    let mut final_start_nonce = String::new();
    for step in 0..18 {
        let nonce = format!("nonce:service:start:{step}");
        let response =
            account_lobby_session::start_session(player_one, session_id.clone(), nonce.clone())
                .expect("start recovery step should not trap");
        assert_eq!(response.status, CommandStatus::Applied);
        let state = match &response.result {
            LobbyCommandResult::Session(session) => session.state.as_str(),
            other => panic!("start_session returned unexpected result: {other:?}"),
        };
        if state == "active" {
            final_start_nonce = nonce;
            final_start = Some(response);
            break;
        }
    }
    let started = final_start.expect("phased setup should reach active state");
    let started_command_id = started.command_id.clone();

    let start_replay =
        account_lobby_session::start_session(player_one, session_id.clone(), final_start_nonce)
            .expect("start replay should not trap");
    assert_eq!(start_replay.command_id, started_command_id);

    let started_session_id = session_id.clone();
    let participant_two = account_lobby_session::get_my_participant(player_two, session_id)
        .expect("participant two should be readable after recovery");
    assert_eq!(participant_two.slot_index, 1);
    assert!(participant_two.ready);

    let moved = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        "champion:west".to_string(),
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:service:move:wood".to_string(),
        1_000,
    )
    .expect("movement intent should submit against seeded IcyDB rows");
    assert_eq!(moved.status, CommandStatus::Applied);

    let participant_one =
        account_lobby_session::get_my_participant(player_one, started_session_id.clone())
            .expect("participant one should be readable before movement recovery");
    let session_key = Ulid::from_str(&started_session_id).expect("service session ids are Ulids");
    let session_id = Id::from_key(session_key);
    let participant_id = Id::<GameParticipant>::from_key(
        Ulid::from_str(&participant_one.participant_id).expect("participant id should be Ulid"),
    );
    let sync_nonce = "nonce:service:sync:wood".to_string();
    let sync_payload_json = format!(r#"{{"session_id":"{started_session_id}"}}"#);
    let seeded_sync_command = commands_events_effects::create_game_command(
        session_id,
        "player".to_string(),
        participant_one.participant_id.clone(),
        None,
        Some(participant_id),
        None,
        1,
        command_response::nonce_u64("sync_session_turn", &sync_nonce),
        "sync_session_turn".to_string(),
        command_response::payload_hash(
            "sync_session_turn",
            &participant_one.participant_id,
            &sync_nonce,
            &sync_payload_json,
        ),
        sync_payload_json,
    )
    .expect("pending sync command seed should persist");

    let synced = movement_service::sync_session_turn(
        player_one,
        started_session_id.clone(),
        u64::MAX,
        sync_nonce,
    )
    .expect("turn sync should write movement snapshots");
    assert_eq!(synced.status, CommandStatus::Applied, "{synced:?}");
    assert_eq!(synced.command_id, seeded_sync_command.id().to_string());
    assert!(
        synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete"),
        "one canister sync slice should park the remaining movement work"
    );

    let synced = movement_service::sync_session_turn(
        player_one,
        started_session_id.clone(),
        u64::MAX,
        "nonce:service:sync:wood:finish".to_string(),
    )
    .expect("second turn sync should finish the two-step movement");
    assert_eq!(synced.status, CommandStatus::Applied, "{synced:?}");

    let session_key = Ulid::from_str(&started_session_id).expect("service session ids are Ulids");
    let session_id = Id::from_key(session_key);
    let champion = champions_artifacts::find_champion_by_session_xy(session_id, 9, 23)
        .expect("champion lookup should not fail")
        .expect("champion should have moved to the resource pile");
    let snapshots = movement_repo::page_movement_snapshots_for_champion_turn(
        session_id,
        1,
        champion.id(),
        10,
        None,
    )
    .expect("movement snapshots should page through typed IcyDB rows");
    assert!(
        snapshots
            .items
            .iter()
            .any(|snapshot| snapshot.outcome == "stopped_object_interaction"),
        "turn sync should persist a first-class movement snapshot row for object stops"
    );

    let guarded = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        champion.id().to_string(),
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:service:move:guarded".to_string(),
        122_000,
    )
    .expect("guarded movement intent should submit");
    assert_eq!(guarded.status, CommandStatus::Applied);

    let mut guarded_sync = None;
    let mut saw_partial_guarded_sync = false;
    for step in 0..6 {
        let synced = movement_service::sync_session_turn(
            player_one,
            started_session_id.clone(),
            u64::MAX,
            format!("nonce:service:sync:guarded:{step}"),
        )
        .expect("guarded movement sync should progress");
        saw_partial_guarded_sync |= synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        if synced.events.iter().any(|event| {
            event.event_type == "neutral_encounter_pending"
                && event
                    .payload
                    .as_deref()
                    .is_some_and(|payload| payload.contains("\"battle_id\""))
        }) {
            guarded_sync = Some(synced);
            break;
        }
    }
    assert!(
        saw_partial_guarded_sync,
        "guarded movement should park at least one partial sync slice"
    );
    let guarded_sync = guarded_sync.expect("guarded movement should start neutral battle");
    assert!(guarded_sync.events.iter().any(|event| {
        event.event_type == "neutral_encounter_pending"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("\"battle_id\""))
    }));

    let champion_after_guard = champions_artifacts::load_champion(champion.id())
        .expect("champion reload should not fail")
        .expect("champion should still exist");
    assert_eq!(champion_after_guard.status, "in_battle");
    let battle_id = champion_after_guard
        .in_battle_id
        .map(Id::<Battle>::from_key)
        .expect("neutral contact should set champion battle id");
    let battle = battles::find_battle_by_attacker(champion.id())
        .expect("battle lookup should not fail")
        .expect("neutral contact should persist battle row");
    assert_eq!(battle.id(), battle_id);
    assert_eq!(battle.battle_type, "neutral");
    assert_eq!(battle.state, "active");
    assert_eq!(battle.attacker_champion_id, Some(champion.id().key()));
    assert!(battle.defender_neutral_army_id.is_some());
    assert!(battle.active_stack_id.is_some());
    assert!(
        !battles::page_battle_stacks_by_side(battle_id, "attacker", 10, None)
            .expect("attacker battle stacks should page")
            .items
            .is_empty()
    );
    assert!(
        !battles::page_battle_stacks_by_side(battle_id, "defender", 10, None)
            .expect("defender battle stacks should page")
            .items
            .is_empty()
    );
}
