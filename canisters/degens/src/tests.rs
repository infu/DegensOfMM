use std::collections::BTreeSet;

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
