use candid::Principal;
use domm_degens_schema::schema::{Battle, GameParticipant};
use domm_game::{
    BattleActionInput, BattleView, CommandStatus, FIRST_PLAYABLE_RULESET_ID, LobbyCommandResult,
    MoveCoord, Viewport,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp, Ulid},
};

use super::{
    account_lobby_session, battle as battle_service, command_response, events as event_service,
    game_view as game_view_service, movement as movement_service,
};
use crate::repos::{
    battles, champions_artifacts, commands_events_effects, content, movement as movement_repo,
    sessions, system_jobs as system_job_repo,
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

    let session_key = Ulid::from_str(&started_session_id).expect("service session ids are Ulids");
    let session_id = Id::from_key(session_key);
    let closing_job = system_job_repo::create_system_job(system_job_repo::SystemJobDraft {
        job_key: format!("test:turn_resolution:{started_session_id}:1"),
        job_kind: "turn_resolution".to_string(),
        session_id,
        battle_id: None,
        turn_number: Some(1),
        due_at: Timestamp::now(),
        command_id: None,
        cursor_json: None,
    })
    .expect("turn-resolution job seed should persist");
    let late_nonce = "nonce:service:move:late-closing".to_string();
    let late_move = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        "champion:west".to_string(),
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        late_nonce.clone(),
        1_000,
    )
    .expect_err("accepted turn job should block new old-turn movement before command creation");
    assert_eq!(late_move.code, "backend_work_pending");
    let missing_late_status = event_service::get_command_status_by_nonce(
        player_one,
        started_session_id.clone(),
        "submit_move_intent".to_string(),
        late_nonce,
    )
    .expect_err("pre-command late movement denial should not leave a command row");
    assert_eq!(missing_late_status.code, "command_status_not_found");
    system_job_repo::complete_system_job(closing_job)
        .expect("test turn-resolution job should be cleared before normal movement");

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
    let object_page = game_view_service::get_visible_objects(
        player_one,
        started_session_id.clone(),
        Viewport::new(0, 16, 24, 24),
        None,
        32,
    )
    .expect("visible objects should hydrate from live rows after movement");
    let champion_object = object_page
        .objects
        .iter()
        .find(|object| {
            object.subject_kind == "champion" && object.subject_id_text == "champion:west"
        })
        .expect("visible object projection should include the moved champion");
    assert_eq!((champion_object.x, champion_object.y), (9, 23));
    assert!(
        object_page
            .objects
            .iter()
            .all(|object| object.subject_id_text != "pile:west-wood-1"),
        "collected resource piles must not render as available objects"
    );
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

    let session_row = sessions::load_session(session_id)
        .expect("session reload should not fail")
        .expect("session should exist for spell setup");
    let hex_spark =
        content::find_spell_by_ruleset_slug(Id::from_key(session_row.ruleset_id), "hex-spark")
            .expect("spell lookup should not fail")
            .expect("hex spark should be seeded");
    champions_artifacts::create_champion_spell(
        session_id,
        champion.id(),
        hex_spark.id(),
        "hex-spark",
        session_row.current_turn,
        Id::from_key(
            battle
                .last_command_id
                .expect("battle should carry the setup command id"),
        ),
    )
    .expect("learned battle spell should persist");

    let battle_id_text = battle_id.to_string();
    let own_battle = battle_service::get_battle_state(
        player_one,
        started_session_id.clone(),
        battle_id_text.clone(),
        0,
    )
    .expect("involved participant should see neutral battle tactics");
    assert_eq!(own_battle.battle_type, "neutral");
    assert!(!own_battle.legal_actions_for_caller.is_empty());
    let cast_action = own_battle
        .legal_actions_for_caller
        .iter()
        .find(|action| {
            action.action == "CastAbility"
                && action.ability_key.as_deref() == Some("spell:hex-spark")
        })
        .expect("learned battle spell should appear as CastAbility");
    assert!(cast_action.enabled);
    assert!(!cast_action.targets.is_empty());
    assert!(own_battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Retreat"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("retreat_deferred_v1_no_rehire_flow")
    }));
    assert!(own_battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Surrender"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("surrender_deferred_v1_no_payment_terms")
    }));

    let denied = battle_service::get_battle_state(
        player_two,
        started_session_id.clone(),
        battle_id_text.clone(),
        0,
    )
    .expect_err("uninvolved participant must not see neutral battle tactics");
    assert_eq!(denied.code, "battle_not_visible");

    let battle_action = battle_service::submit_battle_action(
        player_one,
        started_session_id.clone(),
        first_enabled_battle_action(&own_battle),
        "nonce:service:battle:privacy:action".to_string(),
        0,
    )
    .expect("involved participant should submit a battle action");
    assert_eq!(battle_action.status, CommandStatus::Applied);

    let private_events = event_service::get_events_after(
        player_one,
        started_session_id.clone(),
        format!("participant:{}", participant_one.participant_id),
        0,
        50,
    )
    .expect("participant battle event feed should load");
    let private_action = private_events
        .events
        .iter()
        .find(|event| event.event_type == "battle_action_applied")
        .expect("participant feed should include detailed battle action event");
    let private_payload = private_action
        .payload
        .as_deref()
        .expect("private battle event should include payload");
    assert!(private_payload.contains("subject_id_text"));
    assert!(private_payload.contains(r#""payload":"#));

    let public_events = event_service::get_events_after(
        player_one,
        started_session_id.clone(),
        "public".to_string(),
        0,
        50,
    )
    .expect("public battle event feed should load");
    let public_action = public_events
        .events
        .iter()
        .find(|event| event.event_type == "battle_action_applied")
        .expect("public feed should include redacted battle action event");
    let public_payload = public_action
        .payload
        .as_deref()
        .expect("public battle event should include redacted payload");
    assert!(public_payload.contains(r#""redacted":true"#));
    assert!(!public_payload.contains("subject_id_text"));
    assert!(!public_payload.contains("stack_id"));

    let forbidden_participant_one_events = event_service::get_events_after(
        player_two,
        started_session_id,
        format!("participant:{}", participant_one.participant_id),
        0,
        50,
    )
    .expect_err("participant event audiences must not be readable by opponents");
    assert_eq!(
        forbidden_participant_one_events.code,
        "audience_not_allowed"
    );
}

fn first_enabled_battle_action(view: &BattleView) -> BattleActionInput {
    let active_stack_id = view
        .active_stack_id
        .clone()
        .expect("active battle should have an active stack");
    let action = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled && action.action == "Defend")
        .or_else(|| {
            view.legal_actions_for_caller
                .iter()
                .find(|action| action.enabled)
        })
        .expect("battle view should expose an enabled action");
    BattleActionInput {
        battle_id: view.battle_id.clone(),
        battle_stack_id: active_stack_id,
        action: action.action.clone(),
        ability_key: action.ability_key.clone(),
        target_stack_id: action.targets.first().cloned(),
        destination: action.path.first().copied(),
    }
}
