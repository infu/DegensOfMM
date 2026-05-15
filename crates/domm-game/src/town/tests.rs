use super::{
    ArmyStackRecord, RecruitTarget, TownError, build_first_playable_town_state,
    run_first_playable_town_smoke,
};
use crate::economy::build_first_playable_economy_state;
use crate::fixtures::first_playable_fixture;

#[test]
fn build_preview_enforces_prerequisites_and_affordability() {
    let fixture = first_playable_fixture();
    let town = build_first_playable_town_state();
    let economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    let blocked = town
        .preview_build_town_structure(&economy, participant_id, "town:west", "skirmisher-stall", 2)
        .expect("preview should run");
    assert_eq!(
        blocked.disabled_reason.as_deref(),
        Some("missing_prerequisite:freehold-training-yard")
    );

    let allowed = town
        .preview_build_town_structure(
            &economy,
            participant_id,
            "town:west",
            "freehold-training-yard",
            2,
        )
        .expect("preview should run");
    assert!(allowed.allowed);
}

#[test]
fn build_command_spends_once_and_duplicate_build_is_rejected() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:training-yard",
    )
    .expect("build should apply");
    let after = economy
        .participant(participant_id)
        .unwrap()
        .balances
        .clone();
    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:training-yard",
    )
    .expect("exact command retry should be idempotent");
    assert_eq!(economy.participant(participant_id).unwrap().balances, after);

    let duplicate = town
        .preview_build_town_structure(
            &economy,
            participant_id,
            "town:west",
            "freehold-training-yard",
            3,
        )
        .expect("preview should run");
    assert_eq!(duplicate.disabled_reason.as_deref(), Some("already_built"));
}

#[test]
fn build_recovery_continues_after_resource_spend_without_double_charge() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    let interrupted = town
        .submit_build_town_structure_with_interruption(
            &mut economy,
            participant_id,
            "town:west",
            "freehold-training-yard",
            2,
            "command:build:recover",
        )
        .expect_err("test path should interrupt after spend");
    assert!(matches!(interrupted, TownError::InterruptedAfterSpend));
    let after_spend = economy
        .participant(participant_id)
        .unwrap()
        .balances
        .clone();
    assert!(!town.has_building("town:west", "freehold-training-yard"));

    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:recover",
    )
    .expect("retry should finish structural mutation");
    assert_eq!(
        economy.participant(participant_id).unwrap().balances,
        after_spend
    );
    assert!(town.has_building("town:west", "freehold-training-yard"));
}

#[test]
fn recruit_pool_growth_is_lazy_and_bounded_by_week() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:growth",
    )
    .expect("build unlocks pool");
    town.materialize_recruit_pool_growth("town:west", "mudhook-levy", 15, "command:growth")
        .expect("growth should materialize");
    let pool = town
        .recruit_pools
        .iter()
        .find(|pool| pool.town_id == "town:west" && pool.unit_slug == "mudhook-levy")
        .expect("pool should exist");

    assert_eq!(pool.available, 32);
    assert_eq!(pool.last_growth_week, 3);
}

#[test]
fn recruit_preview_validates_champion_position_and_target_slots() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:champion-check",
    )
    .expect("build should apply");
    town.materialize_recruit_pool_growth(
        "town:west",
        "mudhook-levy",
        8,
        "command:growth:champion-check",
    )
    .expect("growth should apply");

    let away = town
        .preview_recruit_units(
            &economy,
            participant_id,
            "town:west",
            "mudhook-levy",
            4,
            &RecruitTarget::Champion {
                champion_id: "champion:west".to_string(),
                slot_index: None,
            },
            8,
        )
        .expect_err("champion away from town should be rejected");
    assert!(matches!(away, TownError::Disabled { reason } if reason == "champion_not_at_town"));

    let champion = town
        .champions
        .iter_mut()
        .find(|champion| champion.champion_id == "champion:west")
        .expect("champion exists");
    champion.x = 6;
    champion.y = 24;
    let allowed = town
        .preview_recruit_units(
            &economy,
            participant_id,
            "town:west",
            "mudhook-levy",
            4,
            &RecruitTarget::Champion {
                champion_id: "champion:west".to_string(),
                slot_index: None,
            },
            8,
        )
        .expect("preview should run");
    assert!(allowed.allowed);
}

#[test]
fn recruit_rejects_full_or_incompatible_targets() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:target-check",
    )
    .expect("build should apply");
    town.materialize_recruit_pool_growth("town:west", "mudhook-levy", 8, "command:growth:target")
        .expect("growth should apply");
    town.garrison_stacks
        .push(test_stack("town:west", 0, "tollroad-skirmisher", 1));

    let incompatible = town
        .preview_recruit_units(
            &economy,
            participant_id,
            "town:west",
            "mudhook-levy",
            1,
            &RecruitTarget::TownGarrison {
                slot_index: Some(0),
            },
            8,
        )
        .expect_err("explicit incompatible stack should fail");
    assert!(matches!(incompatible, TownError::UnitStackIncompatible));

    for slot in 1..7 {
        town.garrison_stacks
            .push(test_stack("town:west", slot, "tollroad-skirmisher", 1));
    }
    let full = town
        .preview_recruit_units(
            &economy,
            participant_id,
            "town:west",
            "mudhook-levy",
            1,
            &RecruitTarget::TownGarrison { slot_index: None },
            8,
        )
        .expect_err("full incompatible garrison should fail");
    assert!(matches!(full, TownError::RecruitTargetFull));
}

#[test]
fn recruit_command_spends_decrements_pool_and_merges_stack() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:recruit",
    )
    .expect("build should apply");
    town.submit_recruit_units(
        &mut economy,
        participant_id,
        "town:west",
        "mudhook-levy",
        4,
        RecruitTarget::TownGarrison { slot_index: None },
        8,
        "command:recruit:four",
    )
    .expect("recruit should apply");

    let stack = town
        .garrison_stacks
        .iter()
        .find(|stack| stack.owner_id == "town:west" && stack.unit_slug == "mudhook-levy")
        .expect("garrison stack should be created");
    let pool = town
        .recruit_pools
        .iter()
        .find(|pool| pool.town_id == "town:west" && pool.unit_slug == "mudhook-levy")
        .expect("pool should exist");

    assert_eq!(stack.quantity, 4);
    assert_eq!(pool.available, 12);
}

#[test]
fn recruit_recovery_continues_after_resource_spend_without_double_charge() {
    let fixture = first_playable_fixture();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:build:recruit-recover",
    )
    .expect("build should apply");
    town.materialize_recruit_pool_growth("town:west", "mudhook-levy", 8, "command:growth:recover")
        .expect("growth should apply");

    let interrupted = town
        .submit_recruit_units_with_interruption(
            &mut economy,
            participant_id,
            "town:west",
            "mudhook-levy",
            4,
            RecruitTarget::TownGarrison { slot_index: None },
            8,
            "command:recruit:recover",
        )
        .expect_err("test path should interrupt after spend");
    assert!(matches!(interrupted, TownError::InterruptedAfterSpend));
    let after_spend = economy
        .participant(participant_id)
        .unwrap()
        .balances
        .clone();
    assert!(town.garrison_stacks.is_empty());

    town.submit_recruit_units(
        &mut economy,
        participant_id,
        "town:west",
        "mudhook-levy",
        4,
        RecruitTarget::TownGarrison { slot_index: None },
        8,
        "command:recruit:recover",
    )
    .expect("retry should finish stack mutation");

    assert_eq!(
        economy.participant(participant_id).unwrap().balances,
        after_spend
    );
    assert_eq!(town.garrison_stacks.len(), 1);
    assert_eq!(town.garrison_stacks[0].quantity, 4);
}

#[test]
fn town_cache_repair_uses_authoritative_building_rows() {
    let mut town = build_first_playable_town_state();
    town.town_mut("town:west").unwrap().hall_level = 0;

    town.repair_town_caches("town:west")
        .expect("repair should use buildings");

    assert_eq!(town.town("town:west").unwrap().hall_level, 1);
}

#[test]
fn first_playable_town_smoke_builds_and_recruits() {
    let smoke = run_first_playable_town_smoke().expect("town smoke should pass");

    assert_eq!(smoke.town_id, "town:west");
    assert_eq!(smoke.built_building_slug, "freehold-training-yard");
    assert_eq!(smoke.recruited_unit_slug, "mudhook-levy");
    assert_eq!(smoke.recruited_quantity, 4);
    assert!(smoke.final_resources.gold < 13_000);
}

fn test_stack(owner_id: &str, slot_index: u8, unit_slug: &str, quantity: u32) -> ArmyStackRecord {
    ArmyStackRecord {
        stack_id: format!("test-stack:{owner_id}:{slot_index}"),
        session_id: "fixture-session-first-playable".to_string(),
        owner_kind: "town".to_string(),
        owner_id: owner_id.to_string(),
        unit_slug: unit_slug.to_string(),
        slot_index,
        quantity,
        front_hp: 10,
        status: "active".to_string(),
        last_command_id: None,
    }
}
