use crate::battle::{BATTLE_SIDE_ATTACKER, BattleRecord};

use super::actions::{
    apply_battle_aftermath, check_and_finalize_victory, resolve_neutral_battle_for_fixture,
};
use super::build::build_first_playable_aftermath_state;
use super::types::{AftermathError, AftermathSmokeView};

pub fn run_first_playable_aftermath_smoke() -> Result<AftermathSmokeView, AftermathError> {
    let mut state = build_first_playable_aftermath_state()?;
    let neutral_battle_id =
        resolve_neutral_battle_for_fixture(&mut state, "command:smoke:neutral-resolve")?;
    apply_battle_aftermath(
        &mut state,
        &neutral_battle_id,
        "command:smoke:neutral-aftermath",
        1_800_000_500_000,
    )?;

    let town_battle_id = seed_resolved_town_capture_battle(&mut state);
    apply_battle_aftermath(
        &mut state,
        &town_battle_id,
        "command:smoke:town-aftermath",
        1_800_000_510_000,
    )?;

    let champion_battle_id = seed_resolved_champion_defeat_battle(&mut state);
    apply_battle_aftermath(
        &mut state,
        &champion_battle_id,
        "command:smoke:champion-aftermath",
        1_800_000_520_000,
    )?;
    check_and_finalize_victory(&mut state, "command:smoke:victory", 1_800_000_530_000)?;

    Ok(AftermathSmokeView {
        final_session_state: state.session.state,
        winner_participant_id: state.session.winner_participant_id,
        defeated_neutral_state: state.neutral.army("neutral:west-mine")?.state.clone(),
        captured_town_owner: state.town.town("town:east")?.owner_participant_id.clone(),
        defeated_champion_status: state.champions.champion("champion:east")?.status.clone(),
        match_summary_count: state.player_match_summaries.len() as u32,
        match_history_count: state.match_history.len() as u32,
    })
}

pub fn seed_resolved_town_capture_battle(state: &mut super::types::AftermathState) -> String {
    let fixture = crate::fixtures::first_playable_fixture();
    let battle_id = "battle:fixture:town-capture".to_string();
    let mut battle = state.battle.battles[0].clone();
    battle.battle_id = battle_id.clone();
    battle.battle_type = "town".to_string();
    battle.defender_neutral_army_id = None;
    battle.defender_town_id = Some("town:east".to_string());
    battle.created_turn = 10;
    battle.state = "resolved".to_string();
    battle.winner_participant_id = Some(fixture.ids.participant_one_id);
    battle.active_stack_id = None;
    battle.action_deadline_at = None;
    battle.resolved_at = Some(10 * 60_000);
    state.battle.battles.push(battle);
    for stack in state
        .battle
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == state.battle.battles[0].battle_id)
        .cloned()
        .collect::<Vec<_>>()
    {
        let mut copy = stack;
        copy.battle_id = battle_id.clone();
        if copy.side != BATTLE_SIDE_ATTACKER {
            copy.status = "defeated".to_string();
            copy.quantity = 0;
            copy.front_hp = 0;
        }
        copy.battle_stack_id =
            format!("battle-stack:{battle_id}:{}:{}", copy.side, copy.slot_index);
        state.battle.stacks.push(copy);
    }
    battle_id
}

pub fn seed_resolved_champion_defeat_battle(state: &mut super::types::AftermathState) -> String {
    let fixture = crate::fixtures::first_playable_fixture();
    let battle_id = "battle:fixture:champion-defeat".to_string();
    state
        .champions
        .set_champion_status("champion:east", "in_battle", 11, "command:seed:east-battle")
        .expect("east champion exists");
    let battle = BattleRecord {
        battle_id: battle_id.clone(),
        session_id: state.session.session_id.clone(),
        state: "resolved".to_string(),
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
        winner_participant_id: Some(fixture.ids.participant_one_id),
        created_turn: 11,
        action_deadline_at: None,
        resolved_at: Some(11 * 60_000),
        cleanup_after_turn: 0,
        last_command_id: Some("command:seed:champion-battle".to_string()),
    };
    state.battle.battles.push(battle);
    battle_id
}
