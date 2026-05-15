use super::actions::{
    apply_battle_aftermath, check_and_finalize_victory, finalize_stalemate,
    require_retreat_or_surrender_enabled, resolve_neutral_battle_for_fixture,
    retreat_surrender_policy,
};
use super::build::build_first_playable_aftermath_state;
use super::smoke::{
    run_first_playable_aftermath_smoke, seed_resolved_champion_defeat_battle,
    seed_resolved_town_capture_battle,
};
use super::types::AftermathError;
use crate::battle::{BATTLE_SIDE_ATTACKER, BattleRecord};
use crate::champion::ArtifactInstanceRecord;
use crate::fixtures::first_playable_fixture;

#[test]
fn neutral_battle_aftermath_cleans_guard_and_restores_champion() {
    let mut state = build_first_playable_aftermath_state().expect("aftermath state builds");
    let battle_id =
        resolve_neutral_battle_for_fixture(&mut state, "command:test:neutral-resolve").unwrap();

    let report = apply_battle_aftermath(
        &mut state,
        &battle_id,
        "command:test:neutral-aftermath",
        1_800_000_500_000,
    )
    .expect("neutral aftermath should apply");

    assert_eq!(
        report.defeated_neutral_army_id.as_deref(),
        Some("neutral:west-mine")
    );
    assert_eq!(
        state.neutral.army("neutral:west-mine").unwrap().state,
        "defeated"
    );
    assert_eq!(
        state.champions.champion("champion:west").unwrap().status,
        "active"
    );
    assert_eq!(state.champions.champion("champion:west").unwrap().x, 12);
    assert_eq!(state.champions.champion("champion:west").unwrap().y, 22);
    assert!(
        state
            .map
            .occupancy_rows
            .iter()
            .any(|row| row.occupant_kind == "champion" && row.occupant_id_text == "champion:west")
    );
    let event_count = state.aftermath_events.len();

    let replay = apply_battle_aftermath(
        &mut state,
        &battle_id,
        "command:test:neutral-aftermath",
        1_800_000_500_000,
    )
    .expect("neutral aftermath should replay");
    assert_eq!(report, replay);
    assert_eq!(state.aftermath_events.len(), event_count);
}

#[test]
fn town_capture_updates_owner_garrison_income_and_map_owner() {
    let mut state = build_first_playable_aftermath_state().expect("aftermath state builds");
    let fixture = first_playable_fixture();
    let east_gold_before = state
        .economy
        .participant(&fixture.ids.participant_two_id)
        .unwrap()
        .balances
        .gold;
    let battle_id = seed_resolved_town_capture_battle(&mut state);

    let report = apply_battle_aftermath(
        &mut state,
        &battle_id,
        "command:test:town-aftermath",
        1_800_000_510_000,
    )
    .expect("town aftermath should apply");

    assert_eq!(report.captured_town_id.as_deref(), Some("town:east"));
    assert_eq!(
        state.town.town("town:east").unwrap().owner_participant_id,
        fixture.ids.participant_one_id
    );
    assert_eq!(state.town.town("town:east").unwrap().unrest_until_turn, 12);
    assert!(
        state
            .town
            .garrison_stacks
            .iter()
            .any(|stack| stack.owner_id == "town:east" && stack.quantity > 0)
    );
    let east_source = state
        .economy
        .income_sources
        .iter()
        .find(|source| source.source_id == "town:east")
        .unwrap();
    assert_eq!(
        east_source.owner_participant_id.as_deref(),
        Some(fixture.ids.participant_one_id.as_str())
    );
    assert!(
        state
            .economy
            .participant(&fixture.ids.participant_two_id)
            .unwrap()
            .balances
            .gold
            > east_gold_before
    );
    assert_eq!(
        state
            .map
            .subjects
            .iter()
            .find(|subject| subject.subject_kind == "town" && subject.subject_id_text == "town:east")
            .unwrap()
            .owner_participant_id
            .as_deref(),
        Some(fixture.ids.participant_one_id.as_str())
    );
}

#[test]
fn champion_defeat_captures_artifacts_and_blocks_elimination_while_battle_active() {
    let mut state = build_first_playable_aftermath_state().expect("aftermath state builds");
    let fixture = first_playable_fixture();
    state
        .champions
        .artifact_instances
        .push(ArtifactInstanceRecord {
            artifact_id: "artifact-instance:east-prize".to_string(),
            session_id: state.session.session_id.clone(),
            artifact_def_id: "artifact:def-banner".to_string(),
            owner_champion_id: Some("champion:east".to_string()),
            slot: None,
            x: 0,
            y: 0,
            state: "stored".to_string(),
            last_command_id: Some("command:test:east-artifact".to_string()),
        });
    state
        .champions
        .equip_artifact(
            "champion:east",
            "artifact-instance:east-prize",
            "banner",
            8,
            "command:test:east-artifact",
        )
        .expect("east artifact should equip");
    let battle_id = seed_resolved_champion_defeat_battle(&mut state);

    let report = apply_battle_aftermath(
        &mut state,
        &battle_id,
        "command:test:champion-aftermath",
        1_800_000_520_000,
    )
    .expect("champion aftermath should apply");
    assert_eq!(
        report.defeated_champion_id.as_deref(),
        Some("champion:east")
    );
    assert_eq!(report.captured_artifacts, ["artifact-instance:east-prize"]);
    assert_eq!(
        state.champions.champion("champion:east").unwrap().status,
        "defeated"
    );
    let captured_artifact = state
        .champions
        .artifact_instances
        .iter()
        .find(|artifact| artifact.artifact_id == "artifact-instance:east-prize")
        .expect("captured artifact should remain indexed");
    assert_eq!(
        captured_artifact.owner_champion_id.as_deref(),
        Some("champion:west")
    );
    assert_eq!(captured_artifact.state, "stored");

    let mut blocked = state.clone();
    blocked
        .town
        .town_mut("town:east")
        .unwrap()
        .owner_participant_id = fixture.ids.participant_one_id.clone();
    blocked.battle.battles.push(BattleRecord {
        battle_id: "battle:still-active".to_string(),
        session_id: blocked.session.session_id.clone(),
        state: "active".to_string(),
        battle_type: "champion".to_string(),
        attacker_champion_id: Some("champion:west".to_string()),
        defender_champion_id: Some("champion:east".to_string()),
        defender_town_id: None,
        defender_neutral_army_id: None,
        current_round: 1,
        active_side: BATTLE_SIDE_ATTACKER.to_string(),
        active_stack_id: None,
        grid_width: 12,
        grid_height: 10,
        max_rounds: 20,
        turn_seed: 0,
        winner_participant_id: None,
        created_turn: 12,
        action_deadline_at: None,
        resolved_at: None,
        cleanup_after_turn: 0,
        last_command_id: None,
    });
    let check = check_and_finalize_victory(
        &mut blocked,
        "command:test:no-elim-active-battle",
        1_800_000_530_000,
    )
    .expect("active battle should block victory");
    assert!(!check.finalized);
}

#[test]
fn victory_finalization_writes_match_summaries_and_history() {
    let mut state = build_first_playable_aftermath_state().expect("aftermath state builds");
    let fixture = first_playable_fixture();
    state
        .town
        .town_mut("town:east")
        .unwrap()
        .owner_participant_id = fixture.ids.participant_one_id.clone();
    state.battle.battles.clear();
    state
        .champions
        .set_champion_status("champion:east", "defeated", 12, "command:test:defeat-east")
        .unwrap();

    let check = check_and_finalize_victory(&mut state, "command:test:victory", 1_800_000_540_000)
        .expect("victory should finalize");
    assert!(check.finalized);
    assert_eq!(
        check.winner_participant_id.as_deref(),
        Some(fixture.ids.participant_one_id.as_str())
    );
    assert_eq!(state.session.state, "finished");
    assert_eq!(state.player_match_summaries.len(), 2);
    assert_eq!(state.match_history.len(), 2);
    assert!(
        state
            .aftermath_events
            .iter()
            .any(|event| event.event_type == "match_finished")
    );
    assert!(
        state
            .player_match_summaries
            .iter()
            .any(
                |summary| summary.player_id == fixture.ids.player_one_id && summary.result == "win"
            )
    );
}

#[test]
fn max_turn_stalemate_uses_bounded_scores() {
    let mut state = build_first_playable_aftermath_state().expect("aftermath state builds");
    state.session.current_turn = state.session.max_turns;
    state.battle.battles.clear();
    let check = finalize_stalemate(&mut state, "command:test:stalemate", 1_800_000_550_000)
        .expect("stalemate should finalize");

    assert!(check.finalized);
    assert_eq!(check.finish_reason.as_deref(), Some("max_turn_score"));
    assert!(check.winner_participant_id.is_some());
    assert_eq!(check.scores.len(), 2);
    assert!(check.scores.iter().all(|score| score.town_count == 1));
    assert!(check.scores.iter().all(|score| score.army_power_score > 0));
    assert_ne!(
        check.scores[0].tie_break_score,
        check.scores[1].tie_break_score
    );
    assert_eq!(state.player_match_summaries.len(), 2);
    assert!(
        state
            .player_match_summaries
            .iter()
            .any(|summary| summary.result == "win")
    );
    assert!(
        state
            .aftermath_events
            .iter()
            .any(|event| event.event_type == "match_finished")
    );
}

#[test]
fn retreat_and_surrender_are_explicit_v1_disabled_paths() {
    let policy = retreat_surrender_policy();
    assert!(!policy.retreat_allowed);
    assert_eq!(
        policy.retreat_disabled_reason.as_deref(),
        Some("retreat_deferred_v1_no_rehire_flow")
    );
    assert!(!policy.surrender_allowed);
    assert!(matches!(
        require_retreat_or_surrender_enabled("Surrender"),
        Err(AftermathError::RetreatSurrenderDisabled { .. })
    ));
}

#[test]
fn first_playable_aftermath_smoke_reaches_victory() {
    let smoke = run_first_playable_aftermath_smoke().expect("aftermath smoke should pass");
    let fixture = first_playable_fixture();
    assert_eq!(smoke.final_session_state, "finished");
    assert_eq!(
        smoke.winner_participant_id.as_deref(),
        Some(fixture.ids.participant_one_id.as_str())
    );
    assert_eq!(smoke.defeated_neutral_state, "defeated");
    assert_eq!(smoke.captured_town_owner, fixture.ids.participant_one_id);
    assert_eq!(smoke.defeated_champion_status, "defeated");
    assert_eq!(smoke.match_summary_count, 2);
    assert_eq!(smoke.match_history_count, 2);
}
