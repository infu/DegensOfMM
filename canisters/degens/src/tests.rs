use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use candid::{CandidType, Decode, Encode, Principal};
use domm_game::{
    ApiError, ApiEventPage, ApiEventView, CommandPhase, CommandResponse, CommandResult,
    CommandStatus, EventPageInfo, FIRST_PLAYABLE_CHUNK_SIZE, GameViewRequest, LobbyCommandResponse,
    LobbyCommandResult, MAX_CHUNK_LIMIT, MAX_EVENT_LIMIT, MAX_OBJECT_LIMIT, MapChunkPage,
    MapChunkView, ObjectView, ObjectViewPage, RecruitTarget, Viewport,
};

use super::{
    EndpointKind, REQUIRED_GAME_ENDPOINTS, deferred_endpoint_decisions,
    exported_candid_text_for_tests,
};

#[test]
fn endpoint_inventory_has_required_groups_without_duplicates() {
    let mut names = BTreeSet::new();
    for endpoint in REQUIRED_GAME_ENDPOINTS {
        assert!(names.insert(endpoint.name), "duplicate {}", endpoint.name);
        assert!(!endpoint.fixture_mapping.is_empty());
    }

    assert_eq!(REQUIRED_GAME_ENDPOINTS.len(), 59);
    assert_eq!(
        REQUIRED_GAME_ENDPOINTS
            .iter()
            .filter(|endpoint| endpoint.kind == EndpointKind::Update)
            .count(),
        25
    );
    assert_eq!(
        REQUIRED_GAME_ENDPOINTS
            .iter()
            .filter(|endpoint| endpoint.kind == EndpointKind::Query)
            .count(),
        34
    );
    assert!(names.contains("register_player"));
    assert!(names.contains("get_setup_progress"));
    assert!(names.contains("get_game_view"));
    assert!(names.contains("submit_battle_action"));
    assert!(names.contains("preview_move_path"));
    assert!(names.contains("preview_champion_progression"));
    assert!(names.contains("select_champion_level_up"));
    assert!(names.contains("learn_champion_spell"));
    assert!(names.contains("cast_adventure_spell"));
    assert!(names.contains("get_tavern_offers"));
    assert!(names.contains("hire_tavern_champion"));
    assert!(names.contains("submit_market_trade"));
    assert!(names.contains("submit_dwelling_recruit"));
    assert!(names.contains("get_objective_progress"));
    assert!(names.contains("get_scenario_rules"));
    assert!(names.contains("get_world_events"));
    assert!(names.contains("preview_quest"));
    assert!(names.contains("accept_quest"));
    assert!(names.contains("claim_quest_reward"));
    assert!(names.contains("sync_objectives"));
    assert!(names.contains("sync_world_events"));
    assert!(names.contains("sync_advanced_victory"));
    assert!(names.contains("get_skirmish_settings"));
    assert!(names.contains("get_procedural_map_state"));
    assert!(names.contains("get_naval_routes"));
    assert!(names.contains("get_siege_rules"));
    assert!(names.contains("sync_world_generation"));
}

#[test]
fn endpoint_inventory_docs_match_contract_table() {
    let docs = include_str!("../../../docs/canister-endpoints.md");
    let documented_rows = docs
        .lines()
        .skip_while(|line| *line != "| Endpoint | Kind | Fixture mapping |")
        .skip(2)
        .take_while(|line| line.starts_with("| `"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let expected_rows = REQUIRED_GAME_ENDPOINTS
        .iter()
        .map(|endpoint| {
            format!(
                "| `{}` | {} | `{}` |",
                endpoint.name,
                endpoint_kind_label(endpoint.kind),
                endpoint.fixture_mapping
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(documented_rows, expected_rows);
}

#[test]
fn perf1_test_command_matrix_documents_required_lanes() {
    let docs = include_str!("../../../docs/perf1-test-command-matrix.md");
    let runner = include_str!("../../../scripts/run-test-groups.sh");
    let required_lanes = [
        "fast-unit-service",
        "focused-pocketic-gameplay",
        "projection-recovery",
        "long-form-pocketic",
        "full-benchmark-reliability",
    ];
    let required_commands = [
        "scripts/run-test-groups.sh perf1-fast",
        "scripts/run-test-groups.sh perf1-focused",
        "scripts/run-test-groups.sh projection-recovery",
        "scripts/run-test-groups.sh perf1-long-form",
        "cargo test -p domm-game property_ -- --nocapture",
        "cargo test -p domm-degens-canister service_ -- --nocapture",
        "cargo fmt --all --check",
        "git diff --check",
    ];
    let required_failure_artifacts = [
        "target/test-artifacts",
        "DOMM_TEST_ARTIFACT_DIR",
        "failure-summary.md",
        "seed.txt",
        "step-log.txt",
        "last-successful-view-snapshots.txt",
        "command-event-ids.txt",
        "active-runtime-diagnostics.txt",
        "projection-snapshot.txt",
        "timer-job-snapshot.txt",
        "replay.sh",
        "logs/",
    ];
    let required_runner_terms = [
        "pure-property",
        "service-regression",
        "projection-recovery",
        "PERF1_FAST_GROUPS",
        "PERF1_FOCUSED_GROUPS",
        "PERF1_LONG_FORM_GROUPS",
        "perf1-fast)",
        "perf1-focused)",
        "perf1-long-form)",
        "write_failure_artifacts",
        "DOMM_TEST_ARTIFACT_DIR",
        "DOMM_ENABLE_FAILURE_ARTIFACT_SELF_TEST",
        "failure-artifact-self-test",
    ];

    for lane in required_lanes {
        assert!(
            docs.contains(lane),
            "perf1 command matrix must document {lane}"
        );
    }
    for command in required_commands {
        assert!(
            docs.contains(command),
            "perf1 command matrix must include `{command}`"
        );
    }
    for artifact in required_failure_artifacts {
        assert!(
            docs.contains(artifact),
            "perf1 command matrix must document failure artifact `{artifact}`"
        );
    }
    for term in required_runner_terms {
        assert!(
            runner.contains(term),
            "perf1 test runner must wire `{term}`"
        );
    }
}

#[test]
fn get_game_view_contract_is_shell_with_dedicated_projection_endpoints() {
    let source = include_str!("services/game_view.rs");
    let docs = include_str!("../../../docs/canister-endpoints.md");
    let endpoint_names = REQUIRED_GAME_ENDPOINTS
        .iter()
        .map(|endpoint| endpoint.name)
        .collect::<BTreeSet<_>>();

    assert!(
        docs.contains("`get_game_view` is intentionally a lightweight session shell"),
        "canister endpoint docs must state the get_game_view shell contract"
    );
    assert!(
        source.contains("Keep the aggregate view as a session shell"),
        "service source should carry the shell-contract rationale near the implementation"
    );
    for field in ["map_chunks", "objects", "champions", "towns"] {
        assert!(
            source.contains(&format!(r#""{field}".to_string()"#)),
            "get_game_view must report `{field}` in omitted_fields while shell-only"
        );
    }
    for initializer in [
        "let chunks = Vec::new();",
        "let objects = Vec::new();",
        "let champions = Vec::new();",
        "let towns = Vec::new();",
        "battle: None,",
        "battle_summary: None,",
    ] {
        assert!(
            source.contains(initializer),
            "get_game_view shell contract should keep `{initializer}`"
        );
    }
    for endpoint in [
        "get_visible_map_chunks",
        "get_visible_objects",
        "get_object_view",
        "get_my_champions",
        "get_champion_view",
        "get_town_view",
        "get_battle_state",
        "get_events_after",
    ] {
        assert!(
            endpoint_names.contains(endpoint),
            "dedicated endpoint `{endpoint}` must remain in required gameplay coverage"
        );
    }
}

fn endpoint_kind_label(kind: EndpointKind) -> &'static str {
    match kind {
        EndpointKind::Query => "query",
        EndpointKind::Update => "update",
    }
}

#[test]
fn deferred_endpoint_decisions_are_explicit() {
    let names = deferred_endpoint_decisions()
        .iter()
        .map(|decision| decision.name)
        .collect::<BTreeSet<_>>();

    assert_eq!(names.len(), 5);
    assert!(names.contains("leave_session"));
    assert!(names.contains("cancel_session"));
    assert!(names.contains("surrender"));
    assert!(names.contains("retreat"));
    assert!(names.contains("request_rematch"));
    assert!(
        deferred_endpoint_decisions()
            .iter()
            .all(|decision| !decision.decision.is_empty())
    );
}

#[test]
fn deferred_endpoint_policy_stays_out_of_v1_canister_surface() {
    let required_names = REQUIRED_GAME_ENDPOINTS
        .iter()
        .map(|endpoint| endpoint.name)
        .collect::<BTreeSet<_>>();
    let candid = exported_candid_text_for_tests();
    let expected_policy = [
        (
            "leave_session",
            "lobby cancellation/leave semantics are promoted",
        ),
        (
            "cancel_session",
            "lobby cancellation/leave semantics are promoted",
        ),
        (
            "surrender",
            "explicitly disabled in v1 alongside retreat/surrender policy",
        ),
        (
            "retreat",
            "explicitly disabled in v1 alongside retreat/surrender policy",
        ),
        (
            "request_rematch",
            "durable rematch creation remains deferred",
        ),
    ];

    for (name, decision_snippet) in expected_policy {
        let decision = deferred_endpoint_decisions()
            .iter()
            .find(|decision| decision.name == name)
            .unwrap_or_else(|| panic!("missing deferred endpoint decision for {name}"));
        assert!(
            decision.decision.contains(decision_snippet),
            "deferred endpoint {name} should document policy containing {decision_snippet:?}"
        );
        assert!(
            !required_names.contains(name),
            "deferred endpoint {name} must not count as required v1 gameplay coverage"
        );
        assert!(
            !candid.contains(&format!("{name} :")),
            "deferred endpoint {name} must not be exported in v1 Candid"
        );
    }
}

#[test]
fn exported_candid_contains_every_required_game_endpoint() {
    let candid = exported_candid_text_for_tests();

    for endpoint in REQUIRED_GAME_ENDPOINTS {
        let needle = format!("{} :", endpoint.name);
        assert!(
            candid.contains(&needle),
            "missing {} in exported Candid:\n{candid}",
            endpoint.name
        );
    }
    #[cfg(not(feature = "benchmark"))]
    assert!(candid.contains("get_canister_endpoint_inventory :"));
    assert!(candid.contains("get_diagnostic_storage_snapshot :"));
    #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
    assert!(candid.contains("get_diagnostic_projection_snapshot :"));
    #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
    assert!(candid.contains("run_diagnostic_projection_flush :"));
    #[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
    assert!(candid.contains("run_diagnostic_battle_projection_flush :"));
}

#[test]
fn generated_client_candid_contract_decodes_required_payloads() {
    let candid = exported_candid_text_for_tests();
    assert!(candid.contains("submit_move_intent :"));
    assert!(candid.contains("Ok : CommandResponse"));
    assert!(candid.contains("Err : ApiError"));
    assert_candid_variants(
        &candid,
        "CommandStatus",
        &[
            "Pending",
            "Applying",
            "Applied",
            "Failed",
            "Cancelled",
            "Superseded",
            "AppliedNoop",
        ],
    );
    assert_candid_variants(
        &candid,
        "CommandPhase",
        &[
            "Created",
            "Validated",
            "Applying",
            "EffectsApplied",
            "EventsApplied",
            "Recovered",
            "Complete",
            "Failed",
        ],
    );
    assert_candid_variants(
        &candid,
        "CommandResult",
        &[
            "None",
            "StrategicReceipt",
            "MovementSync",
            "BuildPreview",
            "RecruitPreview",
            "MovementPreview",
            "BattleAction",
            "BattleSync",
            "ChampionMagic",
            "ExpandedEconomy",
            "AdvancedScenario",
            "WorldGeneration",
        ],
    );
    assert_candid_variants(
        &candid,
        "LobbyCommandResult",
        &["None", "Player", "Session"],
    );
    assert_candid_variants(&candid, "RecruitTarget", &["TownGarrison", "Champion"]);
    assert_generated_typescript_client_bindings_compile(&candid);

    let command_response = sample_command_response(CommandResult::None);
    let command_ok = Encode!(&Ok::<CommandResponse, ApiError>(command_response.clone())).unwrap();
    let decoded_command_ok = Decode!(&command_ok, Result<CommandResponse, ApiError>).unwrap();
    assert_eq!(decoded_command_ok, Ok(command_response));

    let command_error = ApiError::new("stale_turn", "client used an old turn number", false);
    let command_err = Encode!(&Err::<CommandResponse, ApiError>(command_error.clone())).unwrap();
    let decoded_command_err = Decode!(&command_err, Result<CommandResponse, ApiError>).unwrap();
    assert_eq!(decoded_command_err, Err(command_error));

    let lobby_response = sample_lobby_response();
    let lobby_ok = Encode!(&Ok::<LobbyCommandResponse, ApiError>(
        lobby_response.clone()
    ))
    .unwrap();
    let decoded_lobby_ok = Decode!(&lobby_ok, Result<LobbyCommandResponse, ApiError>).unwrap();
    assert_eq!(decoded_lobby_ok, Ok(lobby_response));

    assert_legacy_game_view_request_decode_fails();
    assert_legacy_recruit_target_decode_fails();
    assert_large_view_response_size("get_visible_map_chunks", sample_map_chunk_page());
    assert_large_view_response_size("get_visible_objects", sample_object_view_page());
    assert_large_view_response_size("get_events_after", sample_event_page());
}

fn assert_candid_variants(candid: &str, type_name: &str, variants: &[&str]) {
    let header = format!("type {type_name} = variant {{");
    let start = candid
        .find(&header)
        .unwrap_or_else(|| panic!("missing Candid variant block for {type_name}"));
    let tail = &candid[start..];
    let end = tail.find("\ntype ").unwrap_or(tail.len());
    let block = &tail[..end];
    for variant in variants {
        assert!(
            block.contains(variant),
            "Candid type {type_name} must expose stable variant {variant}; block={block}"
        );
    }
}

fn assert_generated_typescript_client_bindings_compile(candid: &str) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("canister manifest should live under canisters/degens");
    let output_dir = workspace_root.join("target/generated-client-candid-contract");
    fs::create_dir_all(&output_dir).expect("should create generated client contract output dir");
    let did_path = output_dir.join("domm_degens.did");
    fs::write(&did_path, candid).expect("should write exported DID for generated client check");

    let check = Command::new("didc")
        .arg("check")
        .arg(&did_path)
        .output()
        .expect("didc must be installed for generated client Candid contract coverage");
    assert!(
        check.status.success(),
        "didc check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let bindings = Command::new("didc")
        .args(["bind", "--target", "ts"])
        .arg(&did_path)
        .output()
        .expect("didc must generate TypeScript bindings for the web client contract");
    assert!(
        bindings.status.success(),
        "didc TypeScript binding generation failed: stdout={} stderr={}",
        String::from_utf8_lossy(&bindings.stdout),
        String::from_utf8_lossy(&bindings.stderr)
    );
    let typescript = String::from_utf8(bindings.stdout)
        .expect("generated TypeScript bindings should be valid UTF-8");
    for needle in [
        "export interface _SERVICE",
        "submit_move_intent",
        "get_visible_map_chunks",
        "CommandResponse",
        "CommandResult",
        "GameViewRequest",
        "RecruitTarget",
    ] {
        assert!(
            typescript.contains(needle),
            "generated TypeScript client binding must include {needle}"
        );
    }
}

fn sample_command_response(result: CommandResult) -> CommandResponse {
    CommandResponse {
        command_id: "command:web-contract".to_string(),
        command_type: "submit_move_intent".to_string(),
        actor_principal: Principal::anonymous(),
        actor_participant_id: Some("participant:web-contract".to_string()),
        client_nonce: "nonce:web-contract".to_string(),
        payload_hash: "payload:web-contract".to_string(),
        status: CommandStatus::Applied,
        phase: CommandPhase::Complete,
        retryable: false,
        effective_turn: 3,
        durable_turn: 3,
        events: vec![sample_event(1)],
        changed_subjects: Vec::new(),
        result,
        error: None,
    }
}

fn sample_lobby_response() -> LobbyCommandResponse {
    LobbyCommandResponse {
        command_id: "lobby-command:web-contract".to_string(),
        command_type: "register_player".to_string(),
        actor_principal: Principal::anonymous(),
        client_nonce: "nonce:lobby-web-contract".to_string(),
        payload_hash: "payload:lobby-web-contract".to_string(),
        status: CommandStatus::Applied,
        phase: CommandPhase::Complete,
        retryable: false,
        effective_turn: 0,
        durable_turn: 0,
        events: Vec::new(),
        changed_subjects: Vec::new(),
        result: LobbyCommandResult::None,
        error: None,
    }
}

fn sample_event(event_seq: u64) -> ApiEventView {
    ApiEventView {
        session_id: "session:web-contract".to_string(),
        event_seq,
        event_key: format!("event:web-contract:{event_seq}"),
        audience_key: "public".to_string(),
        turn_number: 1,
        event_type: "contract_sample".to_string(),
        subject_kind: Some("command".to_string()),
        subject_id_text: Some("command:web-contract".to_string()),
        payload: Some(r#"{"sample":true}"#.to_string()),
        redacted: false,
    }
}

fn assert_legacy_game_view_request_decode_fails() {
    #[derive(CandidType)]
    struct LegacyGameViewRequest {
        viewport: Viewport,
        chunk_cursor: Option<u32>,
        chunk_limit: u32,
        object_cursor: Option<u32>,
        object_limit: u32,
    }

    let legacy = LegacyGameViewRequest {
        viewport: Viewport::new(0, 0, 24, 24),
        chunk_cursor: None,
        chunk_limit: MAX_CHUNK_LIMIT,
        object_cursor: None,
        object_limit: MAX_OBJECT_LIMIT,
    };
    let legacy_bytes = Encode!(&legacy).unwrap();
    let error = Decode!(&legacy_bytes, GameViewRequest)
        .expect_err("old aggregate view request shape must not silently decode");
    let message = error.to_string();
    assert!(
        message.contains("event_limit") || message.contains("include_battle"),
        "legacy request decode should name the missing current fields: {message}"
    );
}

fn assert_legacy_recruit_target_decode_fails() {
    #[derive(CandidType)]
    enum LegacyRecruitTarget {
        Garrison { slot_index: Option<u8> },
    }

    let legacy_bytes = Encode!(&LegacyRecruitTarget::Garrison {
        slot_index: Some(0),
    })
    .unwrap();
    let error = Decode!(&legacy_bytes, RecruitTarget)
        .expect_err("old recruit target variant must not silently decode");
    let message = error.to_string();
    assert!(
        message.contains("Garrison") || message.contains("RecruitTarget"),
        "legacy recruit target decode should surface the incompatible variant: {message}"
    );
}

fn assert_large_view_response_size<T>(method: &str, response: T)
where
    T: CandidType,
{
    const CLIENT_RESPONSE_SIZE_CEILING_BYTES: usize = 256 * 1024;

    let bytes = Encode!(&Ok::<T, ApiError>(response)).unwrap();
    assert!(
        bytes.len() <= CLIENT_RESPONSE_SIZE_CEILING_BYTES,
        "{method} representative response encoded to {} bytes, above client ceiling {CLIENT_RESPONSE_SIZE_CEILING_BYTES}",
        bytes.len()
    );
}

fn sample_map_chunk_page() -> MapChunkPage {
    let chunk_side = usize::from(FIRST_PLAYABLE_CHUNK_SIZE);
    let tile_count = chunk_side * chunk_side;
    let chunks = (0..MAX_CHUNK_LIMIT)
        .map(|index| MapChunkView {
            chunk_id: format!("chunk:{index}"),
            chunk_x: (index % 3) as u16,
            chunk_y: (index / 3) as u16,
            width: u16::from(FIRST_PLAYABLE_CHUNK_SIZE),
            height: u16::from(FIRST_PLAYABLE_CHUNK_SIZE),
            terrain_blob: vec![1; tile_count],
            movement_blob: vec![2; tile_count],
            flags_blob: vec![3; tile_count],
            discovered_blob: vec![255; tile_count],
            visible_blob: vec![255; tile_count],
        })
        .collect();

    MapChunkPage {
        chunks,
        next_cursor: None,
        has_more: false,
    }
}

fn sample_object_view_page() -> ObjectViewPage {
    let objects = (0..MAX_OBJECT_LIMIT)
        .map(|index| ObjectView {
            subject_kind: "world_object".to_string(),
            subject_id_text: format!("object:{index}"),
            visibility: "visible".to_string(),
            redaction_level: "full".to_string(),
            x: (index % 48) as u16,
            y: (index / 48) as u16,
            last_seen_turn: Some(3),
            display_name: Some(format!("Contract Object {index}")),
            asset_key: Some("object.contract".to_string()),
            owner_participant_id: Some("participant:web-contract".to_string()),
            details_json: r#"{"contract":"visible-object","state":"sample"}"#.to_string(),
        })
        .collect();

    ObjectViewPage {
        objects,
        next_cursor: None,
        has_more: false,
    }
}

fn sample_event_page() -> ApiEventPage {
    ApiEventPage {
        events: (0..MAX_EVENT_LIMIT)
            .map(|index| sample_event(u64::from(index) + 1))
            .collect(),
        page_info: EventPageInfo {
            next_event_seq: None,
            has_more: false,
            limit: MAX_EVENT_LIMIT,
        },
    }
}

#[cfg(feature = "benchmark")]
#[test]
fn benchmark_feature_exports_diagnostic_benchmark_endpoints() {
    let candid = exported_candid_text_for_tests();

    assert!(candid.contains("get_diagnostic_benchmark_metrics :"));
    assert!(candid.contains("reset_diagnostic_benchmark_metrics :"));
}

#[test]
fn public_time_sensitive_endpoints_derive_canister_time() {
    let movement_api = include_str!("api/movement.rs");
    let battle_api = include_str!("api/battle.rs");

    for source in [movement_api, battle_api] {
        assert!(
            !source.contains("now_ms:"),
            "public Candid entrypoints must not accept caller-controlled time"
        );
    }
    assert_eq!(movement_api.matches("services::clock::now_ms()").count(), 3);
    assert_eq!(battle_api.matches("clock::now_ms()").count(), 3);
}

#[test]
fn final_gameplay_services_do_not_call_fixture_or_placeholder_backends() {
    let service_sources = [
        include_str!("services/account_lobby_session.rs"),
        include_str!("services/battle.rs"),
        include_str!("services/battle_aftermath.rs"),
        include_str!("services/battle_rows.rs"),
        include_str!("services/battle_start.rs"),
        include_str!("services/champion_magic.rs"),
        include_str!("services/command_response.rs"),
        include_str!("services/content.rs"),
        include_str!("services/economy_expansion.rs"),
        include_str!("services/events.rs"),
        include_str!("services/first_playable_setup.rs"),
        include_str!("services/game_view.rs"),
        include_str!("services/history.rs"),
        include_str!("services/movement.rs"),
        include_str!("services/scenario_progress.rs"),
        include_str!("services/render_projection.rs"),
        include_str!("services/session_context.rs"),
        include_str!("services/town.rs"),
    ];

    for source in service_sources {
        assert!(!source.contains("FixtureApiBackend"));
        assert!(!source.contains("repository_not_implemented"));
        assert!(!source.contains("placeholder::"));
    }
}

#[test]
fn time_sensitive_idempotency_payloads_exclude_server_time() {
    let movement_service = include_str!("services/movement.rs");
    let battle_service = include_str!("services/battle.rs");

    assert!(!movement_service.contains(r#""now_ms""#));
    assert!(!battle_service.contains(r#""now_ms""#));
}

#[test]
fn canister_domain_layout_has_required_module_files() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let required_files = [
        "src/api/account_lobby_session.rs",
        "src/api/game_view.rs",
        "src/api/movement.rs",
        "src/api/scenario_progress.rs",
        "src/api/worldgen.rs",
        "src/api/town.rs",
        "src/api/battle.rs",
        "src/api/champion_magic.rs",
        "src/api/events.rs",
        "src/api/content.rs",
        "src/api/history.rs",
        "src/api/cleanup.rs",
        "src/api/economy_expansion.rs",
        "src/api/diagnostics.rs",
        "src/services/account_lobby_session.rs",
        "src/services/game_view.rs",
        "src/services/movement.rs",
        "src/services/scenario_progress.rs",
        "src/services/worldgen.rs",
        "src/services/town.rs",
        "src/services/battle.rs",
        "src/services/champion_magic.rs",
        "src/services/events.rs",
        "src/services/content.rs",
        "src/services/history.rs",
        "src/services/cleanup.rs",
        "src/services/economy_expansion.rs",
        "src/services/diagnostics.rs",
        "src/repos/players.rs",
        "src/repos/scenario_progress.rs",
        "src/repos/worldgen.rs",
        "src/repos/sessions.rs",
        "src/repos/commands_events_effects.rs",
        "src/repos/content.rs",
        "src/repos/map_visibility_occupancy.rs",
        "src/repos/economy.rs",
        "src/repos/economy_expansion.rs",
        "src/repos/towns.rs",
        "src/repos/champions_artifacts.rs",
        "src/repos/movement.rs",
        "src/repos/neutrals.rs",
        "src/repos/battles.rs",
        "src/repos/aftermath_history.rs",
        "src/repos/cleanup.rs",
        "src/dto/public.rs",
        "src/auth/mod.rs",
        "src/errors.rs",
        "src/metrics/mod.rs",
    ];

    for file in required_files {
        assert!(
            manifest_dir.join(file).is_file(),
            "missing canister domain module {file}"
        );
    }
}
