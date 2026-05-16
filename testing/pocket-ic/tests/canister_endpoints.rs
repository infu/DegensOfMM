use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use canic_testkit::pic::{StandaloneCanisterFixture, install_prebuilt_canister_with_cycles};
use domm_degens_canister::{CanisterEndpointView, REQUIRED_GAME_ENDPOINTS};
use domm_game::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview, ChampionView,
    CommandResponse, CommandStatus, CommandStatusView, ContentManifestResponse,
    FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG, GameView, GameViewRequest,
    LobbyCommandResponse, LobbyCommandResult, MAX_CHUNK_LIMIT, MAX_LIST_LIMIT,
    MAX_MOVE_PATH_STEPS_LIMIT, MAX_OBJECT_LIMIT, MapChunkPage, MatchHistoryPage, MoveCoord,
    MovementPreview, ObjectViewPage, ParticipantView, PlayerView, RecruitPreview, RecruitTarget,
    SessionView, opening_viewport_for_slot,
};

#[test]
fn pocket_ic_canister_exposes_every_required_game_endpoint() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-player-two");
    let battle_id = "battle:presence".to_string();
    let viewport = opening_viewport_for_slot(0);

    let inventory: Vec<CanisterEndpointView> = fixture
        .pic()
        .query_call(fixture.canister_id(), "get_canister_endpoint_inventory", ())
        .expect("endpoint inventory should decode");
    assert_eq!(inventory.len(), REQUIRED_GAME_ENDPOINTS.len());
    for endpoint in REQUIRED_GAME_ENDPOINTS {
        assert!(
            inventory.iter().any(|view| view.name == endpoint.name),
            "missing {} from inventory",
            endpoint.name
        );
    }

    let anonymous_player: Result<PlayerView, ApiError> = fixture
        .pic()
        .query_call(fixture.canister_id(), "get_my_player", ())
        .expect("anonymous get_my_player should decode");
    assert_eq!(
        anonymous_player
            .expect_err("anonymous player query should fail")
            .code,
        "anonymous_not_allowed"
    );

    let registered_one = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("player one registration call should decode")
    .expect("player one registration should succeed");
    assert_eq!(registered_one.status, CommandStatus::Applied);
    let player_one_id = match registered_one.result.clone() {
        LobbyCommandResult::Player(player) => {
            assert_eq!(player.display_name, "Presence One");
            assert_eq!(player.principal, player_one);
            player.player_id
        }
        other => panic!("register_player returned unexpected result: {other:?}"),
    };

    let registered_one_replay = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("register replay should decode")
    .expect("register replay should succeed");
    assert_eq!(registered_one_replay.command_id, registered_one.command_id);

    let register_mismatch = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one-renamed".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("register mismatch should decode")
    .expect("register mismatch should return a command response");
    assert_eq!(register_mismatch.status, CommandStatus::Failed);
    assert_eq!(
        register_mismatch
            .error
            .expect("mismatch should carry error")
            .code,
        "duplicate_nonce_payload_mismatch"
    );

    let registered_two = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "register_player",
        (
            Some("presence-two".to_string()),
            Some("Presence Two".to_string()),
            "nonce:presence:register:two".to_string(),
        ),
    )
    .expect("player two registration call should decode")
    .expect("player two registration should succeed");
    assert_eq!(registered_two.status, CommandStatus::Applied);

    let my_player = query_as::<PlayerView>(&fixture, player_one, "get_my_player", ())
        .expect("get_my_player should decode")
        .expect("player one should be readable");
    assert_eq!(my_player.player_id, player_one_id);

    let created = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "create_session",
        (
            "Presence Match".to_string(),
            FIRST_PLAYABLE_RULESET_ID.to_string(),
            1_u64,
            "nonce:presence:create".to_string(),
        ),
    )
    .expect("create_session should decode")
    .expect("create_session should succeed");
    let session_id = match created.result.clone() {
        LobbyCommandResult::Session(session) => {
            assert_eq!(session.state, "lobby");
            assert_eq!(session.participant_ids.len(), 1);
            session.session_id
        }
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    let fetched_session =
        query_as::<SessionView>(&fixture, player_one, "get_session", (session_id.clone(),))
            .expect("get_session should decode")
            .expect("created session should be readable");
    assert_eq!(fetched_session.session_id, session_id);

    let joined = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "join_session",
        (
            session_id.clone(),
            "faction:ashen-ledger".to_string(),
            "nonce:presence:join".to_string(),
        ),
    )
    .expect("join_session should decode")
    .expect("join_session should succeed");
    assert_eq!(joined.status, CommandStatus::Applied);

    update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "mark_ready",
        (session_id.clone(), "nonce:presence:ready:one".to_string()),
    )
    .expect("player one ready should decode")
    .expect("player one ready should succeed");
    update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "mark_ready",
        (session_id.clone(), "nonce:presence:ready:two".to_string()),
    )
    .expect("player two ready should decode")
    .expect("player two ready should succeed");

    let unauthorized_start = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "start_session",
        (session_id.clone(), "nonce:presence:start:wrong".to_string()),
    )
    .expect("unauthorized start should decode")
    .expect("unauthorized start should return a command response");
    assert_eq!(unauthorized_start.status, CommandStatus::Failed);
    assert_eq!(
        unauthorized_start
            .error
            .expect("unauthorized start should carry error")
            .code,
        "not_session_creator"
    );

    let mut active_start = None;
    for step in 0..16 {
        let nonce = format!("nonce:presence:start:{step}");
        let started = update_as::<LobbyCommandResponse>(
            &fixture,
            player_one,
            "start_session",
            (session_id.clone(), nonce.clone()),
        )
        .expect("start_session should decode")
        .expect("start_session should succeed");
        assert_eq!(started.status, CommandStatus::Applied);
        let state = match &started.result {
            LobbyCommandResult::Session(session) => session.state.as_str(),
            other => panic!("start_session returned unexpected result: {other:?}"),
        };
        if state == "active" {
            active_start = Some((started, nonce));
            break;
        }
    }
    let (active_start, active_start_nonce) =
        active_start.expect("phased start_session should finish setup");

    let participant_one = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("get_my_participant should decode")
    .expect("participant one should be readable");
    assert_eq!(participant_one.slot_index, 0);

    let participant_two = query_as::<ParticipantView>(
        &fixture,
        player_two,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("get_my_participant should decode")
    .expect("participant two should be readable");
    assert_eq!(participant_two.slot_index, 1);
    assert!(participant_two.ready);

    let history =
        query_as::<MatchHistoryPage>(&fixture, player_one, "get_match_history", (0_u32, 10_u32))
            .expect("get_match_history should decode")
            .expect("pending match shells should not appear in history yet");
    assert!(history.entries.is_empty());

    let manifest = query_as::<ContentManifestResponse>(
        &fixture,
        player_one,
        "get_content_manifest",
        (FIRST_PLAYABLE_RULESET_SLUG.to_string(), 1_u32),
    )
    .expect("get_content_manifest should decode")
    .expect("content manifest should be backed by seeded rows");
    assert_eq!(manifest.manifest.ruleset.slug, FIRST_PLAYABLE_RULESET_SLUG);
    assert_eq!(manifest.manifest.ruleset.content_manifest_hash.len(), 64);

    let chunk_page = query_as::<MapChunkPage>(
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 8_u32),
    )
    .expect("get_visible_map_chunks should decode")
    .expect("visible map chunks should load from IcyDB rows");
    assert_eq!(chunk_page.chunks.len(), 4);
    assert!(!chunk_page.has_more);

    let object_page = query_as::<ObjectViewPage>(
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 128_u32),
    )
    .expect("get_visible_objects should decode")
    .expect("visible objects should load from IcyDB rows");
    assert!(object_page.objects.iter().any(|object| {
        object.subject_kind == "champion"
            && object.display_name.as_deref() == Some("Mara of the Toll")
    }));
    assert!(object_page.objects.iter().any(|object| {
        object.subject_kind == "town" && object.display_name.as_deref() == Some("West Woe")
    }));
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "pile:west-wood-1")
    );
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "neutral:west-mine")
    );
    assert!(
        object_page
            .objects
            .iter()
            .all(|object| object.subject_id_text != "champion:east")
    );

    let champions = query_as::<Vec<ChampionView>>(
        &fixture,
        player_one,
        "get_my_champions",
        (session_id.clone(),),
    )
    .expect("get_my_champions should decode")
    .expect("champions should load from IcyDB rows");
    assert_eq!(champions.len(), 1);
    let champion_id = champions[0].champion_id.clone();
    assert_eq!(champions[0].name.as_deref(), Some("Mara of the Toll"));
    assert_eq!(champions[0].army_stacks.len(), 2);

    let champion = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("get_champion_view should decode")
    .expect("own champion should be visible");
    assert_eq!(champion.champion_id, champion_id);

    let game_view = query_as::<GameView>(
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id.clone(),
            GameViewRequest {
                viewport: viewport.clone(),
                chunk_cursor: None,
                chunk_limit: 2,
                object_cursor: None,
                object_limit: 4,
                events_after_seq: 0,
                event_limit: 10,
                include_battle: false,
            },
        ),
    )
    .expect("get_game_view should decode")
    .expect("game view should load from IcyDB rows");
    assert_eq!(game_view.map_chunks.len(), 2);
    assert!(game_view.map_page_info.has_more);
    assert_eq!(game_view.objects.len(), 4);
    assert!(game_view.object_page_info.has_more);
    assert!(
        game_view
            .events
            .iter()
            .any(|event| event.event_type == "session_started")
    );
    let town_id = "town:west".to_string();

    let town = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("get_town_view should decode")
    .expect("own town should be visible");
    assert_eq!(town.town.name, "West Woe");
    assert!(
        town.buildings
            .iter()
            .any(|building| building.building_slug == "crumbling-hall")
    );

    let event_page = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 10_u32),
    )
    .expect("get_events_after should decode")
    .expect("public event feed should load from IcyDB rows");
    assert_eq!(event_page.page_info.limit, 10);
    assert!(
        event_page
            .events
            .iter()
            .any(|event| event.event_type == "session_started")
    );

    let status_by_id = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), active_start.command_id.clone()),
    )
    .expect("get_command_status by id should decode")
    .expect("start command status should be readable by id");
    assert_eq!(status_by_id.status, CommandStatus::Applied);
    assert_eq!(status_by_id.command_id, active_start.command_id);

    let status_by_nonce = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), active_start_nonce),
    )
    .expect("get_command_status by nonce should decode")
    .expect("start command status should be readable by nonce");
    assert_eq!(status_by_nonce.status, CommandStatus::Applied);

    let movement_preview = query_as::<MovementPreview>(
        &fixture,
        player_one,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(champion.x.saturating_add(1), champion.y)],
            1_000_u64,
        ),
    )
    .expect("preview_move_path should decode")
    .expect("movement preview should be typed and read-only");
    assert_eq!(movement_preview.champion_id, champion_id);
    assert_eq!(movement_preview.total_cost, 5);

    let build_preview = query_as::<BuildPreview>(
        &fixture,
        player_one,
        "preview_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
        ),
    )
    .expect("preview_build_town_structure should decode")
    .expect("build preview should be typed and read-only");
    assert!(build_preview.allowed);
    assert_eq!(build_preview.building_slug, "freehold-training-yard");

    let recruit_preview = query_as::<RecruitPreview>(
        &fixture,
        player_one,
        "preview_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
        ),
    )
    .expect("preview_recruit_units should decode")
    .expect("recruit preview should be typed and read-only");
    assert!(!recruit_preview.allowed);
    assert_eq!(
        recruit_preview.disabled_reason.as_deref(),
        Some("recruit_pool_empty")
    );

    let forbidden_events = query_as::<ApiEventPage>(
        &fixture,
        player_two,
        "get_events_after",
        (
            session_id.clone(),
            format!("participant:{}", participant_one.participant_id),
            0_u64,
            10_u32,
        ),
    )
    .expect("forbidden audience event query should decode")
    .expect_err("participant event audience should be private");
    assert_eq!(forbidden_events.code, "audience_not_allowed");

    let anonymous_map = query_as::<MapChunkPage>(
        &fixture,
        candid::Principal::anonymous(),
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 1_u32),
    )
    .expect("anonymous map query should decode")
    .expect_err("anonymous map query should fail with typed auth");
    assert_eq!(anonymous_map.code, "anonymous_not_allowed");

    let oversized_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (
            session_id.clone(),
            "public".to_string(),
            0_u64,
            MAX_LIST_LIMIT + 1,
        ),
    )
    .expect("oversized event query should decode")
    .expect_err("oversized event query should fail typed limit validation");
    assert_eq!(oversized_events.code, "list_limit_exceeded");

    let oversized_chunks = query_as::<MapChunkPage>(
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (
            session_id.clone(),
            viewport.clone(),
            None::<u32>,
            MAX_CHUNK_LIMIT + 1,
        ),
    )
    .expect("oversized chunk query should decode")
    .expect_err("oversized chunk query should fail typed limit validation");
    assert_eq!(oversized_chunks.code, "viewport_chunk_limit_exceeded");

    let oversized_objects = query_as::<ObjectViewPage>(
        &fixture,
        player_one,
        "get_visible_objects",
        (
            session_id.clone(),
            viewport.clone(),
            None::<u32>,
            MAX_OBJECT_LIMIT + 1,
        ),
    )
    .expect("oversized object query should decode")
    .expect_err("oversized object query should fail typed limit validation");
    assert_eq!(oversized_objects.code, "list_limit_exceeded");

    let oversized_history = query_as::<MatchHistoryPage>(
        &fixture,
        player_one,
        "get_match_history",
        (0_u32, MAX_LIST_LIMIT + 1),
    )
    .expect("oversized history query should decode")
    .expect_err("oversized history query should fail typed limit validation");
    assert_eq!(oversized_history.code, "list_limit_exceeded");

    let oversized_game_view = query_as::<GameView>(
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id.clone(),
            GameViewRequest {
                viewport: viewport.clone(),
                chunk_cursor: None,
                chunk_limit: MAX_CHUNK_LIMIT + 1,
                object_cursor: None,
                object_limit: 4,
                events_after_seq: 0,
                event_limit: 10,
                include_battle: false,
            },
        ),
    )
    .expect("oversized game view query should decode")
    .expect_err("oversized game view query should fail typed limit validation");
    assert_eq!(oversized_game_view.code, "viewport_chunk_limit_exceeded");

    let oversized_path = query_as::<MovementPreview>(
        &fixture,
        player_one,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(champion.x, champion.y); MAX_MOVE_PATH_STEPS_LIMIT + 1],
            1_000_u64,
        ),
    )
    .expect("oversized movement preview should decode")
    .expect_err("oversized movement preview should fail typed limit validation");
    assert_eq!(oversized_path.code, "movement_path_too_long");

    let move_path = vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)];
    let moved = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            move_path.clone(),
            "nonce:presence:move:wood".to_string(),
            1_000_u64,
        ),
    )
    .expect("submit_move_intent should decode")
    .expect("submit_move_intent should succeed");
    assert_eq!(moved.status, CommandStatus::Applied);

    let moved_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            move_path,
            "nonce:presence:move:wood".to_string(),
            1_000_u64,
        ),
    )
    .expect("submit_move_intent replay should decode")
    .expect("submit_move_intent replay should succeed");
    assert_eq!(moved_replay.command_id, moved.command_id);

    let move_mismatch = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(9, 24)],
            "nonce:presence:move:wood".to_string(),
            1_000_u64,
        ),
    )
    .expect("submit_move_intent mismatch should decode")
    .expect("submit_move_intent mismatch should return command response");
    assert_eq!(move_mismatch.status, CommandStatus::Failed);
    assert_eq!(
        move_mismatch
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("duplicate_nonce_payload_mismatch")
    );

    let move_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), "nonce:presence:move:wood".to_string()),
    )
    .expect("move command status should decode")
    .expect("move command should be readable by nonce");
    assert_eq!(move_status.status, CommandStatus::Applied);

    let synced = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_session_turn",
        (
            session_id.clone(),
            61_000_u64,
            "nonce:presence:sync-turn:wood".to_string(),
        ),
    )
    .expect("sync_session_turn should decode")
    .expect("sync_session_turn should succeed");
    assert_eq!(synced.status, CommandStatus::Applied);
    assert!(
        synced
            .events
            .iter()
            .any(|event| event.event_type == "resource_picked_up")
    );

    let champion_after_move = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("moved champion query should decode")
    .expect("moved champion should be visible");
    assert_eq!((champion_after_move.x, champion_after_move.y), (9, 23));

    let participant_after_pickup = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after pickup should decode")
    .expect("participant after pickup should be readable");
    assert!(participant_after_pickup.resources.wood > participant_one.resources.wood);

    let built = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:presence:build:yard".to_string(),
        ),
    )
    .expect("submit_build_town_structure should decode")
    .expect("submit_build_town_structure should succeed");
    assert_eq!(built.status, CommandStatus::Applied);

    let town_after_build = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after build should decode")
    .expect("town after build should be visible");
    assert!(
        town_after_build
            .buildings
            .iter()
            .any(|building| building.building_slug == "freehold-training-yard")
    );
    assert!(
        town_after_build
            .recruit_pools
            .iter()
            .any(|pool| pool.unit_slug == "mudhook-levy" && pool.available > 0)
    );

    let recruited = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:presence:recruit:levy".to_string(),
        ),
    )
    .expect("submit_recruit_units should decode")
    .expect("submit_recruit_units should succeed");
    assert_eq!(recruited.status, CommandStatus::Applied);

    let town_after_recruit = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after recruit should decode")
    .expect("town after recruit should be visible");
    assert!(
        town_after_recruit
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );

    let (crystal_mine_sync, crystal_saw_partial_sync) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(14, 23),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 30),
        ],
        "nonce:presence:move:crystal-mine",
        "nonce:presence:sync-turn:crystal-mine:",
        244_000_u64,
        "mine_captured",
    );
    assert_eq!(crystal_mine_sync.status, CommandStatus::Applied);
    assert!(crystal_saw_partial_sync);
    assert!(
        crystal_mine_sync
            .events
            .iter()
            .any(|event| event.event_type == "mine_captured")
    );

    let income_sync = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_session_turn",
        (
            session_id.clone(),
            305_000_u64,
            "nonce:presence:sync-turn:income".to_string(),
        ),
    )
    .expect("income sync should decode")
    .expect("income sync should succeed");
    assert!(
        income_sync
            .events
            .iter()
            .any(|event| event.event_type == "income_materialized")
    );

    let (guarded_mine_sync, guarded_saw_partial_sync) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:presence:move:guarded-mine",
        "nonce:presence:sync-turn:guarded-mine:",
        488_000_u64,
        "neutral_encounter_pending",
    );
    assert_eq!(guarded_mine_sync.status, CommandStatus::Applied);
    assert!(guarded_saw_partial_sync);
    assert!(guarded_mine_sync.events.iter().any(|event| {
        event.event_type == "neutral_encounter_pending"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("\"battle_id\""))
    }));

    assert_query_unimplemented::<BattleView>(
        &fixture,
        "get_battle_state",
        (session_id.clone(), battle_id.clone(), 1_000_u64),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "sync_battle",
        (
            session_id.clone(),
            battle_id.clone(),
            1_000_u64,
            "nonce:presence:sync-battle".to_string(),
        ),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "submit_battle_action",
        (
            session_id,
            BattleActionInput {
                battle_id,
                battle_stack_id: "battle-stack:presence".to_string(),
                action: "Defend".to_string(),
                target_stack_id: None,
                destination: None,
            },
            "nonce:presence:battle-action".to_string(),
            1_000_u64,
        ),
    );
}

#[test]
fn pocket_ic_movement_crossing_conflict_uses_persisted_sync_cursor() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-conflict-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-conflict-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "conflict");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let east_champion_id = owned_champion_id(&fixture, player_two, &session_id);

    let west_preposition_path = (9_u16..=18)
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();
    let east_preposition_path = (29_u16..=38)
        .rev()
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();

    submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        west_preposition_path,
        "nonce:conflict:move:west-preposition",
        "nonce:conflict:sync:west-preposition:",
        1_000_u64,
        "session_turn_synced",
    );
    submit_move_and_sync_until_event(
        &fixture,
        player_two,
        &session_id,
        &east_champion_id,
        east_preposition_path,
        "nonce:conflict:move:east-preposition",
        "nonce:conflict:sync:east-preposition:",
        61_000_u64,
        "session_turn_synced",
    );

    let west_path = (19_u16..=24)
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();
    let east_path = (23_u16..=28)
        .rev()
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();

    submit_move_intent(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        west_path,
        "nonce:conflict:move:west",
        1_000_u64,
    );
    submit_move_intent(
        &fixture,
        player_two,
        &session_id,
        &east_champion_id,
        east_path,
        "nonce:conflict:move:east",
        1_000_u64,
    );

    let (synced, saw_partial_sync) = sync_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:conflict:sync:",
        122_000_u64,
        "champion_encounter_pending",
        8,
    );
    assert_eq!(synced.status, CommandStatus::Applied);
    assert!(saw_partial_sync);

    let west_after = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id),
    )
    .expect("west champion after conflict should decode")
    .expect("west champion after conflict should be visible");
    let east_after = query_as::<ChampionView>(
        &fixture,
        player_two,
        "get_champion_view",
        (session_id, east_champion_id),
    )
    .expect("east champion after conflict should decode")
    .expect("east champion after conflict should be visible");
    assert_eq!(west_after.status, "in_battle");
    assert_eq!(east_after.status, "in_battle");
    assert_eq!((west_after.x, west_after.y), (23, 24));
    assert_eq!((east_after.x, east_after.y), (24, 24));
}

#[test]
fn pocket_ic_stationary_enemy_blocker_starts_champion_encounter() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-blocker-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-blocker-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "blocker");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let east_champion_id = owned_champion_id(&fixture, player_two, &session_id);

    for (leg, range, now_ms) in [
        (0_u8, (29_u16..=38_u16), 1_000_u64),
        (1_u8, (19_u16..=28_u16), 61_000_u64),
        (2_u8, (9_u16..=18_u16), 122_000_u64),
    ] {
        let east_to_blocker = range
            .rev()
            .map(|x| MoveCoord::new(x, 24))
            .collect::<Vec<_>>();
        let (_, saw_partial_sync) = submit_move_and_sync_until_event(
            &fixture,
            player_two,
            &session_id,
            &east_champion_id,
            east_to_blocker,
            &format!("nonce:blocker:move:east:{leg}"),
            &format!("nonce:blocker:sync:east:{leg}:"),
            now_ms,
            "session_turn_synced",
        );
        assert!(saw_partial_sync);
    }

    submit_move_intent(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![MoveCoord::new(9, 24)],
        "nonce:blocker:move:west",
        183_000_u64,
    );
    let (blocked_sync, _) = sync_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:blocker:sync:west:",
        244_000_u64,
        "champion_encounter_pending",
        4,
    );
    assert!(
        blocked_sync
            .events
            .iter()
            .any(|event| event.event_type == "champion_encounter_pending")
    );

    let west_after = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id),
    )
    .expect("west blocker champion query should decode")
    .expect("west blocker champion should be visible");
    let east_after = query_as::<ChampionView>(
        &fixture,
        player_two,
        "get_champion_view",
        (session_id, east_champion_id),
    )
    .expect("east blocker champion query should decode")
    .expect("east blocker champion should be visible");
    assert_eq!(west_after.status, "in_battle");
    assert_eq!(east_after.status, "in_battle");
    assert_eq!((west_after.x, west_after.y), (8, 24));
    assert_eq!((east_after.x, east_after.y), (9, 24));
}

fn start_active_two_player_session(
    fixture: &StandaloneCanisterFixture,
    player_one: candid::Principal,
    player_two: candid::Principal,
    nonce_stem: &str,
) -> String {
    update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "register_player",
        (
            Some(format!("{nonce_stem}-one")),
            Some(format!("{nonce_stem} One")),
            format!("nonce:{nonce_stem}:register:one"),
        ),
    )
    .expect("player one registration should decode")
    .expect("player one registration should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "register_player",
        (
            Some(format!("{nonce_stem}-two")),
            Some(format!("{nonce_stem} Two")),
            format!("nonce:{nonce_stem}:register:two"),
        ),
    )
    .expect("player two registration should decode")
    .expect("player two registration should succeed");

    let created = update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "create_session",
        (
            format!("{nonce_stem} Match"),
            FIRST_PLAYABLE_RULESET_ID.to_string(),
            1_u64,
            format!("nonce:{nonce_stem}:create"),
        ),
    )
    .expect("create_session should decode")
    .expect("create_session should succeed");
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "join_session",
        (
            session_id.clone(),
            "faction:ashen-ledger".to_string(),
            format!("nonce:{nonce_stem}:join"),
        ),
    )
    .expect("join_session should decode")
    .expect("join_session should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:one")),
    )
    .expect("player one ready should decode")
    .expect("player one ready should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:two")),
    )
    .expect("player two ready should decode")
    .expect("player two ready should succeed");

    for step in 0..16 {
        let started = update_as::<LobbyCommandResponse>(
            fixture,
            player_one,
            "start_session",
            (
                session_id.clone(),
                format!("nonce:{nonce_stem}:start:{step}"),
            ),
        )
        .expect("start_session should decode")
        .expect("start_session should succeed");
        assert_eq!(started.status, CommandStatus::Applied);
        if matches!(
            started.result,
            LobbyCommandResult::Session(ref session) if session.state == "active"
        ) {
            return session_id;
        }
    }

    panic!("phased start_session should finish setup");
}

fn owned_champion_id(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> String {
    let champions = query_as::<Vec<ChampionView>>(
        fixture,
        player,
        "get_my_champions",
        (session_id.to_string(),),
    )
    .expect("get_my_champions should decode")
    .expect("owned champions should load");
    assert_eq!(champions.len(), 1);
    champions[0].champion_id.clone()
}

fn submit_move_intent(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    client_nonce: &str,
    now_ms: u64,
) -> CommandResponse {
    let response = update_as::<CommandResponse>(
        fixture,
        player,
        "submit_move_intent",
        (
            session_id.to_string(),
            champion_id.to_string(),
            path,
            client_nonce.to_string(),
            now_ms,
        ),
    )
    .expect("submit_move_intent should decode")
    .expect("submit_move_intent should succeed");
    assert_eq!(response.status, CommandStatus::Applied);
    response
}

fn submit_move_and_sync_until_event(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    move_nonce: &str,
    sync_nonce_prefix: &str,
    now_ms: u64,
    expected_event_type: &str,
) -> (CommandResponse, bool) {
    let max_sync_calls = path.len().saturating_add(2);
    submit_move_intent(
        fixture,
        player,
        session_id,
        champion_id,
        path,
        move_nonce,
        now_ms,
    );
    sync_until_event(
        fixture,
        player,
        session_id,
        sync_nonce_prefix,
        now_ms,
        expected_event_type,
        max_sync_calls,
    )
}

fn sync_until_event(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    sync_nonce_prefix: &str,
    now_ms: u64,
    expected_event_type: &str,
    max_sync_calls: usize,
) -> (CommandResponse, bool) {
    let mut saw_partial_sync = false;
    for attempt in 0..max_sync_calls {
        let synced = update_as::<CommandResponse>(
            fixture,
            player,
            "sync_session_turn",
            (
                session_id.to_string(),
                now_ms.saturating_add((attempt as u64).saturating_mul(1_000)),
                format!("{sync_nonce_prefix}{attempt}"),
            ),
        )
        .expect("sync_session_turn should decode")
        .expect("sync_session_turn should succeed");
        assert_eq!(synced.status, CommandStatus::Applied);
        saw_partial_sync |= synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return (synced, saw_partial_sync);
        }
    }

    panic!("sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls");
}

fn assert_query_unimplemented<T>(
    fixture: &StandaloneCanisterFixture,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    let response: Result<T, ApiError> = fixture
        .pic()
        .query_call(fixture.canister_id(), method, args)
        .unwrap_or_else(|error| panic!("{method} should decode from query call: {error:?}"));
    assert_repository_unimplemented(method, response);
}

fn assert_update_unimplemented<T>(
    fixture: &StandaloneCanisterFixture,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    let response: Result<T, ApiError> = fixture
        .pic()
        .update_call(fixture.canister_id(), method, args)
        .unwrap_or_else(|error| panic!("{method} should decode from update call: {error:?}"));
    assert_repository_unimplemented(method, response);
}

fn assert_repository_unimplemented<T>(method: &str, response: Result<T, ApiError>) {
    let error = match response {
        Ok(_) => panic!("{method} should return repository-not-implemented error"),
        Err(error) => error,
    };
    assert_eq!(error.code, "icydb_repository_not_implemented", "{method}");
    assert!(error.retryable, "{method}");
    assert!(
        error.message.contains(method),
        "{method}: {}",
        error.message
    );
}

fn query_as<T>(
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<Result<T, ApiError>, String>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    fixture
        .pic()
        .query_call_as(fixture.canister_id(), caller, method, args)
        .map_err(|error| format!("{error:?}"))
}

fn update_as<T>(
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<Result<T, ApiError>, String>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    fixture
        .pic()
        .update_call_as(fixture.canister_id(), caller, method, args)
        .map_err(|error| format!("{error:?}"))
}

fn install_degens_canister_fixture() -> StandaloneCanisterFixture {
    let wasm_path = build_degens_canister();
    let wasm = fs::read(&wasm_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", wasm_path.display()));
    install_prebuilt_canister_with_cycles(
        wasm,
        candid::encode_args(()).expect("empty init args encode"),
        10_000_000_000_000,
    )
}

fn build_degens_canister() -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let target_dir = workspace_root.join("target/pocket-ic-endpoint-presence");
    let linker_wrapper_dir = write_host_linker_wrapper(&target_dir);
    let nested_path = path_with_prefix(&linker_wrapper_dir);
    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args([
            "build",
            "-p",
            "domm-degens-canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("PATH", nested_path)
        .output()
        .expect("failed to run cargo build for degens canister");
    assert!(
        output.status.success(),
        "canister wasm build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("domm_degens_canister.wasm")
}

fn write_host_linker_wrapper(target_dir: &Path) -> PathBuf {
    let wrapper_dir = target_dir.join("host-linker-wrapper");
    fs::create_dir_all(&wrapper_dir)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", wrapper_dir.display()));
    let wrapper_path = wrapper_dir.join("cc");
    let real_cc = system_cc_path();
    fs::write(
        &wrapper_path,
        format!(
            "#!/bin/sh\nexec '{}' \"$@\" -fuse-ld=bfd\n",
            shell_single_quote(&real_cc)
        ),
    )
    .unwrap_or_else(|error| panic!("failed to write {}: {error}", wrapper_path.display()));
    make_executable(&wrapper_path);
    wrapper_dir
}

fn system_cc_path() -> String {
    let output = Command::new("sh")
        .args(["-c", "command -v cc"])
        .output()
        .expect("failed to resolve system cc");
    assert!(
        output.status.success(),
        "failed to resolve system cc\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("system cc path should be UTF-8")
        .trim()
        .to_string()
}

fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn path_with_prefix(prefix: &Path) -> std::ffi::OsString {
    let mut paths = vec![prefix.to_path_buf()];
    if let Some(existing_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing_path));
    }
    env::join_paths(paths).expect("nested PATH should join")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()))
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|error| panic!("failed to chmod {}: {error}", path.display()));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
