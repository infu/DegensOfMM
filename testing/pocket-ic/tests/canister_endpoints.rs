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
    assert_eq!(movement_preview.total_cost, 1);

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

    assert_query_unimplemented::<BattleView>(
        &fixture,
        "get_battle_state",
        (session_id.clone(), battle_id.clone(), 1_000_u64),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(1, 1)],
            "nonce:presence:move".to_string(),
            1_000_u64,
        ),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "sync_session_turn",
        (
            session_id.clone(),
            1_000_u64,
            "nonce:presence:sync-turn".to_string(),
        ),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:training-yard".to_string(),
            "nonce:presence:build".to_string(),
        ),
    );
    assert_update_unimplemented::<CommandResponse>(
        &fixture,
        "submit_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:presence:recruit".to_string(),
        ),
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
