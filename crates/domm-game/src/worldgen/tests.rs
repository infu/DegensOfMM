use super::{
    deterministic_procedural_map, first_playable_naval_route, first_playable_siege_rule,
    first_playable_skirmish_settings, validate_boat_movement, validate_generation_caps,
};

#[test]
fn procedural_map_preview_is_seed_stable_and_bounded() {
    let left = deterministic_procedural_map(42, 48, 48, 16, 1).expect("map should validate");
    let right = deterministic_procedural_map(42, 48, 48, 16, 1).expect("map should validate");
    let other_seed = deterministic_procedural_map(43, 48, 48, 16, 1).expect("map should validate");

    assert_eq!(left, right);
    assert_ne!(left.scenario_hash, other_seed.scenario_hash);
    assert_eq!(left.chunk_count, 9);
    assert!(left.water_tile_count > 0);
    assert_eq!(left.land_tile_count + left.water_tile_count, 48 * 48);
}

#[test]
fn generation_caps_reject_large_or_over_chunked_maps() {
    assert_eq!(
        validate_generation_caps(65, 48, 16)
            .expect_err("width beyond checkpoint cap should fail")
            .code,
        "generated_map_too_large"
    );
    assert_eq!(
        validate_generation_caps(64, 64, 8)
            .expect_err("too many chunks should fail")
            .code,
        "generated_chunk_cap_exceeded"
    );
}

#[test]
fn skirmish_settings_keep_deferred_systems_disabled() {
    let settings = first_playable_skirmish_settings(7);
    assert_eq!(settings.profile_key, "skirmish:first-playable-compact");
    assert_eq!(settings.player_count, 2);
    assert!(settings.fog_enabled);
    assert!(!settings.naval_enabled);
    assert!(!settings.siege_enabled);
    assert!(!settings.larger_map_enabled);
}

#[test]
fn naval_routes_require_enabled_route_and_boat() {
    let mut route = first_playable_naval_route();
    assert_eq!(
        validate_boat_movement(&route, true)
            .expect_err("disabled route should fail")
            .code,
        "naval_route_disabled"
    );

    route.status = "active".to_string();
    route.disabled_reason = None;
    assert_eq!(
        validate_boat_movement(&route, false)
            .expect_err("boatless water crossing should fail")
            .code,
        "boat_required"
    );
    validate_boat_movement(&route, true).expect("boat should satisfy active route");
}

#[test]
fn siege_rule_is_bounded_and_explicitly_disabled() {
    let rule = first_playable_siege_rule();
    assert_eq!(rule.rule_key, "siege:first-playable-disabled");
    assert_eq!(rule.status, "disabled");
    assert_eq!(rule.gate_count, 1);
    assert_eq!(rule.tower_count, 1);
    assert_eq!(
        rule.disabled_reason.as_deref(),
        Some("checkpoint_25_schema_only")
    );
    assert!(rule.battle_obstacle_cap <= 24);
}
