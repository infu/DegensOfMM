use std::collections::BTreeSet;
use std::path::Path;

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
