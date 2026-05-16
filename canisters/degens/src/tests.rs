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

    assert_eq!(REQUIRED_GAME_ENDPOINTS.len(), 28);
    assert_eq!(
        REQUIRED_GAME_ENDPOINTS
            .iter()
            .filter(|endpoint| endpoint.kind == EndpointKind::Update)
            .count(),
        11
    );
    assert_eq!(
        REQUIRED_GAME_ENDPOINTS
            .iter()
            .filter(|endpoint| endpoint.kind == EndpointKind::Query)
            .count(),
        17
    );
    assert!(names.contains("register_player"));
    assert!(names.contains("get_game_view"));
    assert!(names.contains("submit_battle_action"));
    assert!(names.contains("preview_move_path"));
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
    assert!(candid.contains("get_canister_endpoint_inventory :"));
}

#[test]
fn canister_domain_layout_has_required_module_files() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let required_files = [
        "src/api/account_lobby_session.rs",
        "src/api/game_view.rs",
        "src/api/movement.rs",
        "src/api/town.rs",
        "src/api/battle.rs",
        "src/api/events.rs",
        "src/api/content.rs",
        "src/api/history.rs",
        "src/api/cleanup.rs",
        "src/api/diagnostics.rs",
        "src/services/account_lobby_session.rs",
        "src/services/game_view.rs",
        "src/services/movement.rs",
        "src/services/town.rs",
        "src/services/battle.rs",
        "src/services/events.rs",
        "src/services/content.rs",
        "src/services/history.rs",
        "src/services/cleanup.rs",
        "src/services/diagnostics.rs",
        "src/repos/players.rs",
        "src/repos/sessions.rs",
        "src/repos/commands_events_effects.rs",
        "src/repos/content.rs",
        "src/repos/map_visibility_occupancy.rs",
        "src/repos/economy.rs",
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
