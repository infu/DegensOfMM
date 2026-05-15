use crate::fixtures::first_playable_fixture;
use crate::rng::RollKey;

use super::{
    ArtifactInstanceRecord, CHAMPION_LEVEL_CAP, ChampionError, ChampionViewResult,
    build_first_playable_champion_state,
};

#[test]
fn champion_movement_resets_lazily_by_turn_and_spends_points() {
    let mut state = build_first_playable_champion_state();

    assert_eq!(state.effective_movement("champion:west", 2).unwrap(), 240);
    assert_eq!(
        state
            .spend_movement("champion:west", 2, 35, "command:move")
            .unwrap(),
        205
    );
    assert_eq!(state.effective_movement("champion:west", 2).unwrap(), 205);
    assert_eq!(state.effective_movement("champion:west", 3).unwrap(), 240);
}

#[test]
fn champion_stack_caps_are_enforced() {
    let mut state = build_first_playable_champion_state();
    let stack = state
        .army_stacks
        .iter_mut()
        .find(|stack| stack.champion_id == "champion:west" && stack.slot_index == 0)
        .unwrap();
    stack.quantity = 99_990;

    let error = state
        .add_to_stack("champion:west", 0, 10, "command:stack-cap")
        .expect_err("stack cap should reject");

    assert!(matches!(
        error,
        ChampionError::StackCapExceeded {
            attempted: 100000,
            ..
        }
    ));
}

#[test]
fn champion_status_transitions_support_battle_garrison_and_defeat() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_champion_state();

    state
        .set_champion_status("champion:west", "in_battle", 2, "command:battle")
        .unwrap();
    state
        .set_champion_status("champion:west", "garrisoned", 3, "command:garrison")
        .unwrap();
    assert_eq!(
        state.active_or_recoverable_champions(&fixture.ids.participant_one_id),
        vec!["champion:west".to_string()]
    );
    state
        .set_champion_status("champion:west", "defeated", 4, "command:defeat")
        .unwrap();
    assert!(
        state
            .active_or_recoverable_champions(&fixture.ids.participant_one_id)
            .is_empty()
    );
    assert_eq!(state.champion("champion:west").unwrap().defeated_turn, 4);
}

#[test]
fn artifact_equipment_uniqueness_is_authoritative() {
    let mut state = build_first_playable_champion_state();
    state.artifact_instances.push(ArtifactInstanceRecord {
        artifact_id: "artifact-instance:second-banner".to_string(),
        session_id: state.session_id.clone(),
        artifact_def_id: "artifact:bent-banner".to_string(),
        owner_champion_id: Some("champion:west".to_string()),
        slot: None,
        x: 0,
        y: 0,
        state: "stored".to_string(),
        last_command_id: None,
    });

    let error = state
        .equip_artifact(
            "champion:west",
            "artifact-instance:second-banner",
            "banner",
            2,
            "command:equip",
        )
        .expect_err("slot uniqueness should reject");

    assert!(matches!(error, ChampionError::EquipmentSlotOccupied { .. }));
}

#[test]
fn deterministic_artifact_capture_transfers_equipped_artifact() {
    let mut state = build_first_playable_champion_state();
    let roll_key = RollKey::new(
        "domm:first-playable:v1",
        "artifact_capture",
        4,
        "command:capture",
        "champion:east",
        "champion:west",
        0,
    );

    let result = state
        .capture_artifacts(
            "champion:east",
            "champion:west",
            false,
            "command:capture",
            &roll_key,
        )
        .unwrap();

    assert_eq!(
        result.captured_artifact_ids,
        vec!["artifact-instance:bent-banner:west".to_string()]
    );
    let artifact = state
        .artifact_instances
        .iter()
        .find(|artifact| artifact.artifact_id == "artifact-instance:bent-banner:west")
        .unwrap();
    assert_eq!(artifact.owner_champion_id.as_deref(), Some("champion:east"));
    assert!(state.artifact_equipment.is_empty());
}

#[test]
fn champion_progression_caps_level_and_defers_skill_choice() {
    let mut state = build_first_playable_champion_state();
    let result = state
        .grant_experience("champion:west", 100_000, "command:xp")
        .unwrap();

    assert_eq!(result.level_after, CHAMPION_LEVEL_CAP);
    assert_eq!(
        state.champion("champion:west").unwrap().level,
        CHAMPION_LEVEL_CAP
    );
    assert_eq!(
        result.skill_choice_status,
        "deferred_v1_no_skill_tree_choice"
    );
}

#[test]
fn champion_views_redact_hidden_enemies_and_show_owned_details() {
    let fixture = first_playable_fixture();
    let state = build_first_playable_champion_state();

    let hidden =
        state.champion_view_for(&fixture.ids.participant_one_id, "champion:east", false, 1);
    assert!(matches!(
        hidden,
        ChampionViewResult::Hidden {
            visibility,
            champion_id,
        } if visibility == "hidden" && champion_id == "champion:east"
    ));

    let own = state.champion_view_for(&fixture.ids.participant_one_id, "champion:west", true, 1);
    let ChampionViewResult::Visible(view) = own else {
        panic!("own champion should be visible");
    };
    assert_eq!(view.name.as_deref(), Some("Mara of the Toll"));
    assert_eq!(view.army_stacks.len(), 2);
    assert_eq!(view.artifacts.len(), 1);
    assert!(!view.redacted);
}
