use crate::battle::build_first_playable_battle_state;
use crate::content::first_playable_content_manifest;
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
fn champion_progression_caps_level_and_creates_skill_choices() {
    let mut state = build_first_playable_champion_state();
    let result = state
        .grant_experience("champion:west", 100_000, "command:xp")
        .unwrap();

    assert_eq!(result.level_after, CHAMPION_LEVEL_CAP);
    assert_eq!(
        state.champion("champion:west").unwrap().level,
        CHAMPION_LEVEL_CAP
    );
    assert_eq!(result.skill_choice_status, "pending_skill_choice");
    assert_eq!(result.skill_points_after, CHAMPION_LEVEL_CAP - 1);
}

#[test]
fn level_up_choice_spends_skill_point_and_unlocks_spell_learning() {
    let manifest = first_playable_content_manifest();
    let mut state = build_first_playable_champion_state();
    state
        .grant_experience("champion:west", 1_000, "command:xp")
        .unwrap();

    let choices = state.level_up_choices("champion:west").unwrap();
    assert_eq!(choices.len(), 3);
    assert!(
        choices
            .iter()
            .any(|choice| choice.skill_key == "sour_sorcery" && choice.enabled)
    );

    let selected = state
        .select_level_up_choice("champion:west", "sour_sorcery", "command:skill")
        .unwrap();
    assert_eq!(selected.skill_key.as_deref(), Some("sour_sorcery"));
    let champion = state.champion("champion:west").unwrap();
    assert_eq!(champion.skill_points, 0);
    assert_eq!(champion.wisdom, 2);
    assert_eq!(champion.mana_max, 12);

    let learned = state
        .learn_spell("champion:west", &manifest, "hex-spark", 2, "command:learn")
        .unwrap();
    assert_eq!(learned.spell_slug.as_deref(), Some("hex-spark"));
    assert_eq!(
        state.learned_spell_slugs("champion:west"),
        vec!["hex-spark".to_string()]
    );
}

#[test]
fn adventure_spell_resets_mana_by_turn_and_caps_movement_gain() {
    let manifest = first_playable_content_manifest();
    let mut state = build_first_playable_champion_state();
    state
        .grant_experience("champion:west", 1_000, "command:xp")
        .unwrap();
    state
        .select_level_up_choice("champion:west", "sour_sorcery", "command:skill")
        .unwrap();
    state
        .learn_spell(
            "champion:west",
            &manifest,
            "spite-march",
            2,
            "command:learn",
        )
        .unwrap();
    state
        .spend_movement("champion:west", 2, 60, "command:move")
        .unwrap();

    let receipt = state
        .cast_adventure_spell("champion:west", &manifest, "spite-march", 3, "command:cast")
        .unwrap();

    assert_eq!(receipt.mana_after, 10);
    assert_eq!(receipt.movement_remaining_after, 240);
    assert_eq!(state.effective_mana("champion:west", 4).unwrap(), 12);
}

#[test]
fn battle_spell_spends_mana_damages_target_and_applies_bounded_status() {
    let manifest = first_playable_content_manifest();
    let mut champions = build_first_playable_champion_state();
    champions
        .grant_experience("champion:west", 1_000, "command:xp")
        .unwrap();
    champions
        .select_level_up_choice("champion:west", "sour_sorcery", "command:skill")
        .unwrap();
    champions
        .learn_spell("champion:west", &manifest, "hex-spark", 2, "command:learn")
        .unwrap();
    let mut battle = build_first_playable_battle_state().unwrap();
    let battle_id = battle.battles[0].battle_id.clone();
    let caster_stack_id = battle
        .stacks
        .iter()
        .find(|stack| stack.side == "attacker")
        .unwrap()
        .battle_stack_id
        .clone();
    let target_stack_id = battle
        .stacks
        .iter()
        .find(|stack| stack.side == "defender")
        .unwrap()
        .battle_stack_id
        .clone();
    let target_before = battle.stack(&target_stack_id).unwrap().quantity;

    let receipt = champions
        .apply_battle_spell(
            &mut battle,
            &battle_id,
            "champion:west",
            &caster_stack_id,
            &target_stack_id,
            "hex-spark",
            "command:battle-cast",
            0,
        )
        .unwrap();

    assert_eq!(receipt.spell_slug.as_deref(), Some("hex-spark"));
    assert_eq!(receipt.mana_after, 7);
    let target = battle.stack(&target_stack_id).unwrap();
    assert!(target.quantity <= target_before);
    assert!(
        target
            .status_keys
            .iter()
            .any(|status| status.starts_with("hexed_until_round:"))
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
