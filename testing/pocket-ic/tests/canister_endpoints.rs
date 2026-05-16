use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use canic_testkit::pic::{StandaloneCanisterFixture, install_prebuilt_canister};
use domm_degens_canister::{CanisterEndpointView, REQUIRED_GAME_ENDPOINTS};
use domm_game::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview, ChampionView,
    CommandResponse, CommandStatusView, ContentManifestResponse, GameView, GameViewRequest,
    LobbyCommandResponse, MapChunkPage, MatchHistoryPage, MoveCoord, MovementPreview,
    ObjectViewPage, ParticipantView, PlayerView, RecruitPreview, RecruitTarget, SessionView,
    opening_viewport_for_slot,
};

#[test]
fn pocket_ic_canister_exposes_every_required_game_endpoint() {
    let fixture = install_degens_canister_fixture();
    let session_id = "session:presence".to_string();
    let champion_id = "champion:presence".to_string();
    let town_id = "town:presence".to_string();
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

    assert_update_unimplemented::<LobbyCommandResponse>(
        &fixture,
        "register_player",
        (
            None::<String>,
            Some("Presence Tester".to_string()),
            "nonce:presence:register".to_string(),
        ),
    );
    assert_query_unimplemented::<PlayerView>(&fixture, "get_my_player", ());
    assert_update_unimplemented::<LobbyCommandResponse>(
        &fixture,
        "create_session",
        (
            "Presence Match".to_string(),
            "ruleset:first-playable".to_string(),
            1_u64,
            "nonce:presence:create".to_string(),
        ),
    );
    assert_update_unimplemented::<LobbyCommandResponse>(
        &fixture,
        "join_session",
        (
            session_id.clone(),
            "faction:misery".to_string(),
            "nonce:presence:join".to_string(),
        ),
    );
    assert_update_unimplemented::<LobbyCommandResponse>(
        &fixture,
        "mark_ready",
        (session_id.clone(), "nonce:presence:ready".to_string()),
    );
    assert_update_unimplemented::<LobbyCommandResponse>(
        &fixture,
        "start_session",
        (session_id.clone(), "nonce:presence:start".to_string()),
    );
    assert_query_unimplemented::<SessionView>(&fixture, "get_session", (session_id.clone(),));
    assert_query_unimplemented::<ParticipantView>(
        &fixture,
        "get_my_participant",
        (session_id.clone(),),
    );
    assert_query_unimplemented::<MatchHistoryPage>(&fixture, "get_match_history", (0_u32, 10_u32));
    assert_query_unimplemented::<GameView>(
        &fixture,
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
    );
    assert_query_unimplemented::<MapChunkPage>(
        &fixture,
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 2_u32),
    );
    assert_query_unimplemented::<ObjectViewPage>(
        &fixture,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 4_u32),
    );
    assert_query_unimplemented::<Vec<ChampionView>>(
        &fixture,
        "get_my_champions",
        (session_id.clone(),),
    );
    assert_query_unimplemented::<ChampionView>(
        &fixture,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    );
    assert_query_unimplemented::<ApiTownView>(
        &fixture,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    );
    assert_query_unimplemented::<BattleView>(
        &fixture,
        "get_battle_state",
        (session_id.clone(), battle_id.clone(), 1_000_u64),
    );
    assert_query_unimplemented::<ContentManifestResponse>(
        &fixture,
        "get_content_manifest",
        ("ruleset:first-playable".to_string(), 1_u32),
    );
    assert_query_unimplemented::<ApiEventPage>(
        &fixture,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 10_u32),
    );
    assert_query_unimplemented::<CommandStatusView>(
        &fixture,
        "get_command_status",
        (session_id.clone(), "nonce:presence:status".to_string()),
    );
    assert_query_unimplemented::<MovementPreview>(
        &fixture,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(1, 1)],
            1_000_u64,
        ),
    );
    assert_query_unimplemented::<BuildPreview>(
        &fixture,
        "preview_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:training-yard".to_string(),
        ),
    );
    assert_query_unimplemented::<RecruitPreview>(
        &fixture,
        "preview_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
        ),
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

fn install_degens_canister_fixture() -> StandaloneCanisterFixture {
    let wasm_path = build_degens_canister();
    let wasm = fs::read(&wasm_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", wasm_path.display()));
    install_prebuilt_canister(
        wasm,
        candid::encode_args(()).expect("empty init args encode"),
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
