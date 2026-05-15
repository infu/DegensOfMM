use candid::{Decode, Encode};

use super::{
    BASE_TOWN_GOLD_INCOME, EconomyError, ResourceApplyBudget, ResourceBalances, ResourceCapMode,
    ResourceDelta, build_first_playable_economy_state, run_first_playable_economy_smoke,
};
use crate::fixtures::first_playable_fixture;

#[test]
fn resource_pickup_is_idempotent_by_command_and_ledger_key() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    let first = state
        .collect_resource_pile(
            participant_id,
            "pile:west-wood-1",
            1,
            "command:pickup:west-wood",
        )
        .expect("first pickup should apply");
    let after_first = state.participant(participant_id).unwrap().balances.clone();
    let retry = state
        .collect_resource_pile(
            participant_id,
            "pile:west-wood-1",
            1,
            "command:pickup:west-wood",
        )
        .expect("exact retry should be idempotent");

    assert_eq!(first.ledger_rows_touched, 1);
    assert_eq!(retry.ledger_rows_touched, 0);
    assert_eq!(after_first.wood, 15);
    assert_eq!(
        state.participant(participant_id).unwrap().balances,
        after_first
    );
    assert_eq!(
        state
            .ledger_entries
            .iter()
            .filter(|entry| entry.reason == "object_reward")
            .count(),
        1
    );
}

#[test]
fn resource_ledger_recovery_resumes_after_budget_exhaustion() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = fixture.ids.participant_one_id;
    let deltas = vec![
        ResourceDelta {
            participant_id: participant_id.clone(),
            resource_key: "wood".to_string(),
            delta: 2,
            reason: "test_reward".to_string(),
            effect_key: "effect:multi".to_string(),
            phase: "a".to_string(),
        },
        ResourceDelta {
            participant_id: participant_id.clone(),
            resource_key: "stone".to_string(),
            delta: 3,
            reason: "test_reward".to_string(),
            effect_key: "effect:multi".to_string(),
            phase: "b".to_string(),
        },
    ];

    let partial = state
        .apply_resource_deltas_with_budget(
            "command:multi",
            1,
            deltas.clone(),
            ResourceCapMode::RejectOnOverflow,
            ResourceApplyBudget { max_ledger_rows: 1 },
        )
        .expect("partial recovery should apply one row");
    assert!(partial.budget_exhausted);
    assert_eq!(partial.ledger_rows_touched, 1);

    let recovered = state
        .apply_resource_deltas(
            "command:multi",
            1,
            deltas,
            ResourceCapMode::RejectOnOverflow,
        )
        .expect("recovery should finish remaining row");
    let balances = &state.participant(&participant_id).unwrap().balances;

    assert_eq!(recovered.ledger_rows_touched, 1);
    assert_eq!(recovered.skipped_applied_rows, 1);
    assert_eq!(balances.wood, 12);
    assert_eq!(balances.stone, 13);
    assert_eq!(state.ledger_entries.len(), 2);
}

#[test]
fn lazy_income_materialization_is_capped_to_fourteen_turns() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    let outcome = state
        .materialize_income(participant_id, 30, "command:income:catchup")
        .expect("lazy income should materialize");
    let participant = state.participant(participant_id).unwrap();

    assert_eq!(outcome.ledger_rows_touched, 1);
    assert_eq!(participant.last_income_turn, 30);
    assert_eq!(
        participant.balances.gold,
        10_000 + BASE_TOWN_GOLD_INCOME * 14
    );
}

#[test]
fn system_income_saturates_at_resource_cap() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    state.participant_mut(participant_id).unwrap().balances.gold = 999_900;

    state
        .materialize_income(participant_id, 2, "command:income:saturating")
        .expect("system income should saturate");
    let participant = state.participant(participant_id).unwrap();

    assert_eq!(participant.balances.gold, 1_000_000);
    assert_eq!(state.ledger_entries[0].delta, 100);
}

#[test]
fn player_reward_that_exceeds_cap_is_rejected_without_writes() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;
    state.participant_mut(participant_id).unwrap().balances.wood = 9_998;

    let error = state
        .collect_resource_pile(
            participant_id,
            "pile:west-wood-1",
            1,
            "command:pickup:cap-reject",
        )
        .expect_err("player pickup should fail over cap");

    assert!(matches!(
        error,
        EconomyError::ValueCapExceeded {
            resource_key,
            attempted: 10003,
            cap: 10000,
        } if resource_key == "wood"
    ));
    assert_eq!(
        state.participant(participant_id).unwrap().balances.wood,
        9_998
    );
    assert!(state.ledger_entries.is_empty());
}

#[test]
fn income_capture_materializes_old_owner_and_delays_new_owner_source_income() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let west = &fixture.ids.participant_one_id;
    let east = &fixture.ids.participant_two_id;

    state
        .capture_income_source("town:west", east, 3, "command:capture:town-west")
        .expect("town capture should materialize cutover income");
    let west_after_capture = state.participant(west).unwrap().balances.gold;
    let east_after_capture = state.participant(east).unwrap().balances.gold;

    assert_eq!(west_after_capture, 10_000 + BASE_TOWN_GOLD_INCOME * 2);
    assert_eq!(east_after_capture, 10_000 + BASE_TOWN_GOLD_INCOME * 2);

    state
        .materialize_income(east, 4, "command:income:east-after-capture")
        .expect("new owner should accrue from subsequent turns");
    let east_after_next_turn = state.participant(east).unwrap().balances.gold;

    assert_eq!(
        east_after_next_turn,
        east_after_capture + BASE_TOWN_GOLD_INCOME * 2
    );
}

#[test]
fn turn_summary_is_idempotent_and_uses_applied_ledger_rows() {
    let fixture = first_playable_fixture();
    let mut state = build_first_playable_economy_state();
    let participant_id = &fixture.ids.participant_one_id;

    state
        .collect_resource_pile(
            participant_id,
            "pile:west-wood-1",
            1,
            "command:pickup:summary",
        )
        .expect("pickup should apply");
    let first = state
        .write_turn_summary(participant_id, 1)
        .expect("summary should write");
    let retry = state
        .write_turn_summary(participant_id, 1)
        .expect("summary retry should reuse row");

    assert_eq!(first, retry);
    assert_eq!(state.turn_summaries.len(), 1);
    assert!(first.summary_json.contains("\"entry_count\":1"));
    assert!(first.summary_json.contains("\"wood\":5"));
}

#[test]
fn first_playable_economy_smoke_pickup_capture_and_income() {
    let smoke = run_first_playable_economy_smoke().expect("economy smoke should pass");

    assert_eq!(smoke.after_pickup.wood, 15);
    assert_eq!(
        smoke.after_income.gold,
        10_000 + BASE_TOWN_GOLD_INCOME * 2 + 250
    );
    assert_eq!(smoke.captured_source_id, "mine:west-gold");
    assert!(smoke.ledger_entries >= 3);
}

#[test]
fn resource_balances_support_candid_roundtrip_shape() {
    let balances = ResourceBalances::starting();
    let encoded = candid::Encode!(&balances).expect("balances should encode");
    let decoded = candid::Decode!(&encoded, ResourceBalances).expect("balances should decode");

    assert_eq!(decoded, balances);
}
