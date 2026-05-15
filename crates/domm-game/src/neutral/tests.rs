use crate::champion::build_first_playable_champion_state;
use crate::fixtures::{TURN_DURATION_MS, first_playable_fixture};
use crate::map::{FirstPlayableMapState, build_first_playable_map_state};
use crate::movement::{
    MoveCoord, MovementSyncBudget, build_first_playable_movement_state, submit_move_intent,
    sync_session_turn,
};

use super::actions::{
    apply_neutral_encounters_from_movement, defeat_neutral_army, materialize_neutral_growth,
    start_guarded_object_encounter,
};
use super::build::build_first_playable_neutral_state;
use super::smoke::run_first_playable_neutral_smoke;
use super::types::{NeutralArmyViewResult, NeutralBehaviorPolicy, strength_label_for_quantity};

#[test]
fn strength_labels_follow_spec_boundaries() {
    assert_eq!(strength_label_for_quantity(0), "None");
    assert_eq!(strength_label_for_quantity(1), "Few");
    assert_eq!(strength_label_for_quantity(9), "Few");
    assert_eq!(strength_label_for_quantity(10), "Pack");
    assert_eq!(strength_label_for_quantity(24), "Pack");
    assert_eq!(strength_label_for_quantity(25), "Group");
    assert_eq!(strength_label_for_quantity(49), "Group");
    assert_eq!(strength_label_for_quantity(50), "Company");
    assert_eq!(strength_label_for_quantity(99), "Company");
    assert_eq!(strength_label_for_quantity(100), "Host");
    assert_eq!(strength_label_for_quantity(250), "Legion");
}

#[test]
fn first_playable_neutral_rows_and_stacks_match_fixture() {
    let neutral = build_first_playable_neutral_state();

    assert_eq!(neutral.armies.len(), 6);
    assert_eq!(neutral.stacks.len(), 10);
    assert_eq!(neutral.quantity_for("neutral:west-mine"), 12);
    assert_eq!(neutral.stacks_for("neutral:north-objective").len(), 2);
    assert!(
        neutral
            .armies
            .iter()
            .all(|army| army.aggression == "guard" && army.growth_rule_key == "none")
    );
}

#[test]
fn neutral_views_redact_exact_stacks_without_scouting() {
    let fixture = first_playable_fixture();
    let neutral = build_first_playable_neutral_state();
    let map = build_first_playable_map_state();

    let visible = neutral.neutral_army_view_for(
        &map,
        &fixture.ids.participant_one_id,
        "neutral:west-mine",
        false,
    );
    let scouting = neutral.neutral_army_view_for(
        &map,
        &fixture.ids.participant_one_id,
        "neutral:west-mine",
        true,
    );
    let hidden = neutral.neutral_army_view_for(
        &map,
        &fixture.ids.participant_two_id,
        "neutral:west-mine",
        false,
    );

    let NeutralArmyViewResult::Visible(visible) = visible else {
        panic!("west mine guard should be visible to west");
    };
    assert!(visible.redacted);
    assert_eq!(visible.strength_label, "Pack");
    assert!(visible.exact_stacks.is_empty());
    let NeutralArmyViewResult::Visible(scouting) = scouting else {
        panic!("scouting view should be visible");
    };
    assert_eq!(scouting.exact_stacks.len(), 1);
    assert!(matches!(hidden, NeutralArmyViewResult::Hidden { .. }));
}

#[test]
fn neutral_occupancy_blocks_until_defeat_cleanup() {
    let mut neutral = build_first_playable_neutral_state();
    let mut map = build_first_playable_map_state();

    assert!(has_neutral_occupancy(&map, "neutral:west-mine"));
    defeat_neutral_army(
        &mut neutral,
        &mut map,
        "neutral:west-mine",
        "command:neutral:defeat",
    )
    .unwrap();

    assert_eq!(neutral.army("neutral:west-mine").unwrap().state, "defeated");
    assert!(!has_neutral_occupancy(&map, "neutral:west-mine"));
}

#[test]
fn movement_neutral_contact_creates_idempotent_encounter() {
    let fixture = first_playable_fixture();
    let mut neutral = build_first_playable_neutral_state();
    let mut map = build_first_playable_map_state();
    let mut champions = build_first_playable_champion_state();
    let mut movement = build_first_playable_movement_state();

    submit_move_intent(
        &mut movement,
        &map,
        &champions,
        &fixture.ids.participant_one_id,
        "champion:west",
        path_to_west_mine_guard(),
        12_101,
        1_000,
    )
    .unwrap();
    let movement_outcome = sync_session_turn(
        &mut movement,
        &mut map,
        &mut champions,
        TURN_DURATION_MS,
        MovementSyncBudget::default(),
    )
    .unwrap();
    let first = apply_neutral_encounters_from_movement(
        &mut neutral,
        &mut map,
        &champions,
        &movement_outcome,
    )
    .unwrap();
    let replay = apply_neutral_encounters_from_movement(
        &mut neutral,
        &mut map,
        &champions,
        &movement_outcome,
    )
    .unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(replay[0].encounter_id, first[0].encounter_id);
    assert_eq!(neutral.encounters.len(), 1);
    assert_eq!(
        neutral.army("neutral:west-mine").unwrap().state,
        "in_battle"
    );
}

#[test]
fn guarded_object_interaction_can_start_guard_encounter() {
    let mut neutral = build_first_playable_neutral_state();
    let mut map = build_first_playable_map_state();
    let champions = build_first_playable_champion_state();

    let encounter = start_guarded_object_encounter(
        &mut neutral,
        &mut map,
        &champions,
        "champion:west",
        "mine:west-gold",
        2,
        "command:object:guard",
    )
    .unwrap();

    assert_eq!(encounter.neutral_army_id, "neutral:west-mine");
    assert_eq!(encounter.source_kind, "guarded_object");
    assert_eq!(encounter.source_id_text, "mine:west-gold");
}

#[test]
fn neutral_growth_is_explicit_v1_noop() {
    let mut neutral = build_first_playable_neutral_state();
    let before = neutral.stacks.clone();

    let outcome = materialize_neutral_growth(&mut neutral, 3, "command:neutral:growth");

    assert!(!outcome.materialized);
    assert_eq!(outcome.stacks_changed, 0);
    assert_eq!(outcome.armies_checked, 6);
    assert_eq!(neutral.stacks, before);
    assert!(outcome.disabled_reason.contains("noop_v1"));
}

#[test]
fn neutral_behavior_policy_disables_roaming_join_and_bribe_for_v1() {
    let policy = NeutralBehaviorPolicy::default();

    assert_eq!(policy.aggression, "guard");
    assert!(!policy.roaming_enabled);
    assert!(!policy.join_enabled);
    assert!(!policy.bribe_enabled);
    assert_eq!(policy.disabled_reasons.len(), 3);
}

#[test]
fn first_playable_neutral_smoke_moves_into_guard_and_cleans_up_defeat() {
    let smoke = run_first_playable_neutral_smoke().unwrap();

    assert_eq!(smoke.neutral_army_id, "neutral:west-mine");
    assert_eq!(smoke.strength_label, "Pack");
    assert!(smoke.battle_key.contains("neutral:west-mine"));
    assert_eq!(smoke.defeated_state, "defeated");
    assert_eq!(smoke.occupancy_rows_after_defeat, 0);
}

fn path_to_west_mine_guard() -> Vec<MoveCoord> {
    vec![
        MoveCoord::new(9, 24),
        MoveCoord::new(10, 24),
        MoveCoord::new(11, 24),
        MoveCoord::new(12, 24),
        MoveCoord::new(12, 23),
        MoveCoord::new(12, 22),
    ]
}

fn has_neutral_occupancy(map: &FirstPlayableMapState, neutral_army_id: &str) -> bool {
    map.occupancy_rows
        .iter()
        .any(|row| row.occupant_kind == "neutral_army" && row.occupant_id_text == neutral_army_id)
}
