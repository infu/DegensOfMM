use crate::battle::{
    BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER, BattleStackRecord, apply_damage_to_stack,
};
use crate::champion::ChampionArmyStackRecord;
use crate::content::first_playable_content_manifest;
use crate::fixtures::{FixtureIds, first_playable_fixture};
use crate::lifecycle::MatchHistoryEntry;
use crate::rng::{RollKey, hash64};
use crate::town::ArmyStackRecord;

use super::types::{
    AftermathError, AftermathEventRecord, AftermathState, BattleAftermathReport,
    PlayerMatchSummaryRecord, RetreatSurrenderPolicy, VictoryCheck, VictoryScore,
};

pub fn apply_battle_aftermath(
    state: &mut AftermathState,
    battle_id: &str,
    command_id: &str,
    finished_at: u64,
) -> Result<BattleAftermathReport, AftermathError> {
    if state.applied_commands.iter().any(|id| id == command_id) {
        return state
            .aftermath_reports
            .iter()
            .find(|(id, _)| id == command_id)
            .map(|(_, report)| report.clone())
            .ok_or_else(|| AftermathError::BattleNotResolved {
                battle_id: battle_id.to_string(),
            });
    }

    let battle = state.battle.battle(battle_id)?.clone();
    if battle.state != "resolved" {
        return Err(AftermathError::BattleNotResolved {
            battle_id: battle_id.to_string(),
        });
    }
    let winner = battle.winner_participant_id.clone().ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle_id.to_string(),
        }
    })?;

    let mut report = BattleAftermathReport::empty(battle_id, &battle.battle_type);
    report.winner_participant_id = Some(winner.clone());

    match battle.battle_type.as_str() {
        "neutral" => {
            apply_neutral_aftermath(state, &battle, &winner, command_id)?;
            report.victor_champion_id = battle.attacker_champion_id.clone();
            report.defeated_neutral_army_id = battle.defender_neutral_army_id.clone();
        }
        "town" => {
            apply_town_aftermath(state, &battle, &winner, command_id)?;
            report.victor_champion_id = battle.attacker_champion_id.clone();
            report.captured_town_id = battle.defender_town_id.clone();
        }
        "champion" => {
            let captured = apply_champion_aftermath(state, &battle, &winner, command_id)?;
            report.victor_champion_id = victor_champion_for(state, &battle, &winner);
            report.defeated_champion_id = defeated_champion_for(state, &battle, &winner);
            report.captured_artifacts = captured;
        }
        _ => {}
    }

    append_aftermath_event(
        state,
        command_id,
        "battle_aftermath",
        format!(
            "battle {} resolved as {}",
            report.battle_id, report.battle_type
        ),
    );
    report.victory = check_and_finalize_victory(state, command_id, finished_at)?;
    state.applied_commands.push(command_id.to_string());
    state
        .aftermath_reports
        .push((command_id.to_string(), report.clone()));
    Ok(report)
}

pub fn resolve_neutral_battle_for_fixture(
    state: &mut AftermathState,
    command_id: &str,
) -> Result<String, AftermathError> {
    let battle_id = state.battle.battles[0].battle_id.clone();
    let defender_ids = state
        .battle
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.side != BATTLE_SIDE_ATTACKER)
        .map(|stack| stack.battle_stack_id.clone())
        .collect::<Vec<_>>();
    for defender_id in defender_ids {
        apply_damage_to_stack(&mut state.battle, &defender_id, 1_000_000, command_id)?;
    }
    let winner = first_playable_fixture().ids.participant_one_id;
    let battle = state.battle.battle_mut(&battle_id)?;
    battle.state = "resolved".to_string();
    battle.winner_participant_id = Some(winner);
    battle.active_stack_id = None;
    battle.action_deadline_at = None;
    battle.resolved_at = Some(8 * 60_000);
    Ok(battle_id)
}

pub fn check_and_finalize_victory(
    state: &mut AftermathState,
    command_id: &str,
    finished_at: u64,
) -> Result<VictoryCheck, AftermathError> {
    let fixture = first_playable_fixture();
    if state
        .battle
        .battles
        .iter()
        .any(|battle| battle.state == "active")
    {
        return Ok(VictoryCheck {
            finalized: false,
            winner_participant_id: None,
            finish_reason: None,
            scores: scores(state, &fixture.ids)?,
        });
    }

    let eliminated = eliminated_participants(state, &fixture.ids);
    let winner = if eliminated.contains(&fixture.ids.participant_one_id) {
        Some(fixture.ids.participant_two_id.clone())
    } else if eliminated.contains(&fixture.ids.participant_two_id) {
        Some(fixture.ids.participant_one_id.clone())
    } else {
        None
    };
    if let Some(winner) = winner {
        return finish_match(
            state,
            Some(winner),
            "elimination",
            command_id,
            finished_at,
            scores(state, &fixture.ids)?,
        );
    }
    if state.session.current_turn >= state.session.max_turns {
        return finalize_stalemate(state, command_id, finished_at);
    }
    Ok(VictoryCheck {
        finalized: false,
        winner_participant_id: None,
        finish_reason: None,
        scores: scores(state, &fixture.ids)?,
    })
}

pub fn finalize_stalemate(
    state: &mut AftermathState,
    command_id: &str,
    finished_at: u64,
) -> Result<VictoryCheck, AftermathError> {
    let fixture = first_playable_fixture();
    let scores = scores(state, &fixture.ids)?;
    let winner = match scores.as_slice() {
        [left, right] if left.total_score > right.total_score => Some(left.participant_id.clone()),
        [left, right] if right.total_score > left.total_score => Some(right.participant_id.clone()),
        _ => None,
    };
    let reason = if winner.is_some() {
        "max_turn_score"
    } else {
        "max_turn_draw"
    };
    finish_match(state, winner, reason, command_id, finished_at, scores)
}

pub fn retreat_surrender_policy() -> RetreatSurrenderPolicy {
    RetreatSurrenderPolicy {
        retreat_allowed: false,
        retreat_disabled_reason: Some("retreat_deferred_v1_no_rehire_flow".to_string()),
        surrender_allowed: false,
        surrender_disabled_reason: Some("surrender_deferred_v1_no_payment_terms".to_string()),
    }
}

pub fn require_retreat_or_surrender_enabled(action: &str) -> Result<(), AftermathError> {
    let policy = retreat_surrender_policy();
    let reason = match action {
        "Retreat" => policy.retreat_disabled_reason,
        "Surrender" => policy.surrender_disabled_reason,
        other => Some(format!("unsupported_battle_exit:{other}")),
    }
    .unwrap_or_else(|| "disabled".to_string());
    Err(AftermathError::RetreatSurrenderDisabled { reason })
}

fn apply_neutral_aftermath(
    state: &mut AftermathState,
    battle: &crate::battle::BattleRecord,
    winner: &str,
    command_id: &str,
) -> Result<(), AftermathError> {
    if Some(winner) != battle.winner_participant_id.as_deref() {
        return Ok(());
    }
    let neutral_army_id = battle.defender_neutral_army_id.as_deref().ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle.battle_id.clone(),
        }
    })?;
    let champion_id = battle.attacker_champion_id.as_deref().ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle.battle_id.clone(),
        }
    })?;
    let neutral_row = state.neutral.army(neutral_army_id)?.clone();
    crate::neutral::defeat_neutral_army(
        &mut state.neutral,
        &mut state.map,
        neutral_army_id,
        command_id,
    )?;
    write_champion_survivors(state, &battle.battle_id, command_id)?;
    set_champion_strategic_position(
        state,
        champion_id,
        neutral_row.x,
        neutral_row.y,
        "active",
        battle.created_turn,
        command_id,
    )?;
    state
        .champions
        .grant_experience(champion_id, 250, command_id)?;
    Ok(())
}

fn apply_town_aftermath(
    state: &mut AftermathState,
    battle: &crate::battle::BattleRecord,
    winner: &str,
    command_id: &str,
) -> Result<(), AftermathError> {
    let town_id =
        battle
            .defender_town_id
            .as_deref()
            .ok_or_else(|| AftermathError::MissingBattleWinner {
                battle_id: battle.battle_id.clone(),
            })?;
    let attacker_champion_id = battle.attacker_champion_id.as_deref().ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle.battle_id.clone(),
        }
    })?;
    let old_owner = state.town.town(town_id)?.owner_participant_id.clone();
    if old_owner == winner {
        write_town_garrison_survivors(
            state,
            &battle.battle_id,
            town_id,
            BATTLE_SIDE_DEFENDER,
            command_id,
        )?;
        return Ok(());
    }

    state
        .economy
        .capture_income_source(town_id, winner, battle.created_turn, command_id)?;
    let town = state.town.town_mut(town_id)?;
    town.owner_participant_id = winner.to_string();
    town.captured_turn = battle.created_turn;
    town.income_started_turn = battle.created_turn;
    town.unrest_until_turn = battle.created_turn.saturating_add(2);
    town.last_command_id = Some(command_id.to_string());
    update_town_map_owner(state, town_id, winner);
    write_town_garrison_survivors(
        state,
        &battle.battle_id,
        town_id,
        BATTLE_SIDE_ATTACKER,
        command_id,
    )?;
    let captured_town = state.town.town(town_id)?.clone();
    set_champion_strategic_position(
        state,
        attacker_champion_id,
        captured_town.x,
        captured_town.y,
        "active",
        battle.created_turn,
        command_id,
    )?;
    Ok(())
}

fn apply_champion_aftermath(
    state: &mut AftermathState,
    battle: &crate::battle::BattleRecord,
    winner: &str,
    command_id: &str,
) -> Result<Vec<String>, AftermathError> {
    let victor = victor_champion_for(state, battle, winner).ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle.battle_id.clone(),
        }
    })?;
    let defeated = defeated_champion_for(state, battle, winner).ok_or_else(|| {
        AftermathError::MissingBattleWinner {
            battle_id: battle.battle_id.clone(),
        }
    })?;
    write_champion_survivors(state, &battle.battle_id, command_id)?;
    state
        .champions
        .set_champion_status(&victor, "active", battle.created_turn, command_id)?;
    state
        .champions
        .set_champion_status(&defeated, "defeated", battle.created_turn, command_id)?;
    state
        .map
        .cleanup_occupancy_by_subject("champion", &defeated);
    mark_champion_map_state(state, &defeated, "defeated", command_id);
    let roll_key = RollKey::new(
        state.battle.session_seed.clone(),
        "artifact_capture",
        battle.created_turn,
        command_id,
        &victor,
        &defeated,
        0,
    );
    let captured = state
        .champions
        .capture_artifacts(&victor, &defeated, true, command_id, &roll_key)?;
    Ok(captured.captured_artifact_ids)
}

fn write_champion_survivors(
    state: &mut AftermathState,
    battle_id: &str,
    command_id: &str,
) -> Result<(), AftermathError> {
    for battle_stack in state
        .battle
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.origin_kind == "champion_army")
    {
        if let Some(origin_stack_id) = battle_stack.origin_stack_id_text.as_deref() {
            upsert_champion_stack(
                &mut state.champions.army_stacks,
                battle_stack,
                origin_stack_id,
                command_id,
            );
        }
    }
    Ok(())
}

fn write_town_garrison_survivors(
    state: &mut AftermathState,
    battle_id: &str,
    town_id: &str,
    survivor_side: &str,
    command_id: &str,
) -> Result<(), AftermathError> {
    state
        .town
        .garrison_stacks
        .retain(|stack| stack.owner_id != town_id);
    for (slot_index, stack) in state
        .battle
        .stacks
        .iter()
        .filter(|stack| {
            stack.battle_id == battle_id && stack.side == survivor_side && stack.is_living()
        })
        .enumerate()
    {
        state.town.garrison_stacks.push(ArmyStackRecord {
            stack_id: format!("town-garrison:{town_id}:{}", slot_index),
            session_id: state.town.session_id.clone(),
            owner_kind: "town".to_string(),
            owner_id: town_id.to_string(),
            unit_slug: stack.unit_id.trim_start_matches("unit:").to_string(),
            slot_index: slot_index as u8,
            quantity: stack.quantity,
            front_hp: stack.front_hp,
            status: "active".to_string(),
            last_command_id: Some(command_id.to_string()),
        });
    }
    Ok(())
}

fn upsert_champion_stack(
    stacks: &mut [ChampionArmyStackRecord],
    battle_stack: &BattleStackRecord,
    origin_stack_id: &str,
    command_id: &str,
) {
    if let Some(stack) = stacks
        .iter_mut()
        .find(|stack| stack.stack_id == origin_stack_id)
    {
        stack.quantity = battle_stack.quantity;
        stack.front_hp = battle_stack.front_hp;
        stack.status = battle_stack.status.clone();
        stack.last_command_id = Some(command_id.to_string());
    }
}

fn set_champion_strategic_position(
    state: &mut AftermathState,
    champion_id: &str,
    x: u16,
    y: u16,
    status: &str,
    turn: u32,
    command_id: &str,
) -> Result<(), AftermathError> {
    {
        let champion = state.champions.champion_mut(champion_id)?;
        champion.x = x;
        champion.y = y;
    }
    state
        .champions
        .set_champion_status(champion_id, status, turn, command_id)?;
    state
        .map
        .cleanup_occupancy_by_subject("champion", champion_id);
    state.map.insert_occupancy_footprint(
        x,
        y,
        1,
        1,
        "unit",
        "champion",
        champion_id,
        true,
        Some(command_id.to_string()),
    )?;
    if let Some(subject) = state.map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.x = x;
        subject.y = y;
        subject.chunk_x = x / 16;
        subject.chunk_y = y / 16;
        subject.state = status.to_string();
        subject.public_json = format!(
            "{{\"type\":\"champion\",\"champion_id\":\"{}\",\"status\":\"{}\"}}",
            escape_json(champion_id),
            escape_json(status)
        );
    }
    Ok(())
}

fn update_town_map_owner(state: &mut AftermathState, town_id: &str, owner: &str) {
    if let Some(subject) = state
        .map
        .subjects
        .iter_mut()
        .find(|subject| subject.subject_kind == "town" && subject.subject_id_text == town_id)
    {
        subject.owner_participant_id = Some(owner.to_string());
        subject.public_json = format!(
            "{{\"type\":\"town\",\"town_id\":\"{}\",\"status\":\"active\",\"owner_participant_id\":\"{}\"}}",
            escape_json(town_id),
            escape_json(owner)
        );
    }
}

fn mark_champion_map_state(
    state: &mut AftermathState,
    champion_id: &str,
    status: &str,
    command_id: &str,
) {
    if let Some(subject) = state.map.subjects.iter_mut().find(|subject| {
        subject.subject_kind == "champion" && subject.subject_id_text == champion_id
    }) {
        subject.state = status.to_string();
        subject.public_json = format!(
            "{{\"type\":\"champion\",\"champion_id\":\"{}\",\"status\":\"{}\",\"last_command_id\":\"{}\"}}",
            escape_json(champion_id),
            escape_json(status),
            escape_json(command_id)
        );
    }
}

fn victor_champion_for(
    state: &AftermathState,
    battle: &crate::battle::BattleRecord,
    winner: &str,
) -> Option<String> {
    if battle.winner_participant_id.as_deref() != Some(winner) {
        return None;
    }
    [
        battle.attacker_champion_id.as_deref(),
        battle.defender_champion_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|champion_id| {
        state
            .champions
            .champion(champion_id)
            .map(|champion| champion.participant_id == winner)
            .unwrap_or(false)
    })
    .map(str::to_string)
}

fn defeated_champion_for(
    state: &AftermathState,
    battle: &crate::battle::BattleRecord,
    winner: &str,
) -> Option<String> {
    [
        battle.attacker_champion_id.as_deref(),
        battle.defender_champion_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|champion_id| {
        state
            .champions
            .champion(champion_id)
            .map(|champion| champion.participant_id != winner)
            .unwrap_or(false)
    })
    .map(str::to_string)
}

fn eliminated_participants(state: &AftermathState, ids: &FixtureIds) -> Vec<String> {
    [&ids.participant_one_id, &ids.participant_two_id]
        .into_iter()
        .filter(|participant_id| {
            let has_town = state.town.towns.iter().any(|town| {
                town.owner_participant_id == **participant_id && town.status == "active"
            });
            let has_champion = !state
                .champions
                .active_or_recoverable_champions(participant_id)
                .is_empty();
            !has_town && !has_champion
        })
        .cloned()
        .collect()
}

fn scores(state: &AftermathState, ids: &FixtureIds) -> Result<Vec<VictoryScore>, AftermathError> {
    [&ids.participant_one_id, &ids.participant_two_id]
        .into_iter()
        .map(|participant_id| {
            let town_count = state
                .town
                .towns
                .iter()
                .filter(|town| {
                    town.owner_participant_id == *participant_id && town.status == "active"
                })
                .count() as u32;
            let mine_count = state
                .economy
                .income_sources
                .iter()
                .filter(|source| {
                    source.owner_participant_id.as_deref() == Some(participant_id)
                        && source.source_kind == "mine"
                })
                .count() as u32;
            let army_power_score = participant_army_power(state, participant_id);
            let tie_break_score = stalemate_tie_break(state, participant_id);
            Ok(VictoryScore {
                participant_id: (*participant_id).clone(),
                town_count,
                mine_count,
                army_power_score,
                tie_break_score,
                total_score: u64::from(town_count) * 1_000_000
                    + u64::from(mine_count) * 100_000
                    + army_power_score.saturating_mul(1_000)
                    + tie_break_score,
            })
        })
        .collect()
}

fn participant_army_power(state: &AftermathState, participant_id: &str) -> u64 {
    let manifest = first_playable_content_manifest();
    let champion_ids = state
        .champions
        .champions
        .iter()
        .filter(|champion| {
            champion.participant_id == participant_id
                && matches!(
                    champion.status.as_str(),
                    "active" | "in_battle" | "garrisoned"
                )
        })
        .map(|champion| champion.champion_id.as_str())
        .collect::<Vec<_>>();
    let champion_power = state
        .champions
        .army_stacks
        .iter()
        .filter(|stack| champion_ids.contains(&stack.champion_id.as_str()))
        .map(|stack| stack_power(&manifest, &stack.unit_slug, stack.quantity))
        .sum::<u64>();
    let owned_town_ids = state
        .town
        .towns
        .iter()
        .filter(|town| town.owner_participant_id == participant_id && town.status == "active")
        .map(|town| town.town_id.as_str())
        .collect::<Vec<_>>();
    let garrison_power = state
        .town
        .garrison_stacks
        .iter()
        .filter(|stack| owned_town_ids.contains(&stack.owner_id.as_str()))
        .map(|stack| stack_power(&manifest, &stack.unit_slug, stack.quantity))
        .sum::<u64>();
    champion_power.saturating_add(garrison_power)
}

fn stack_power(manifest: &crate::content::ContentManifest, unit_slug: &str, quantity: u32) -> u64 {
    let Some(unit) = manifest.unit(unit_slug) else {
        return u64::from(quantity);
    };
    let stat_score = i32::from(unit.attack.max(0))
        + i32::from(unit.defense.max(0))
        + i32::from(unit.speed)
        + i32::from(unit.initiative);
    let damage_score = u64::from(unit.damage_min) + u64::from(unit.damage_max);
    let trait_score = u64::from(unit.ranged) * 2 + u64::from(unit.flying) * 2;
    let unit_score = u64::try_from(stat_score).unwrap_or_default()
        + damage_score
        + u64::from(unit.max_hp)
        + trait_score;
    unit_score.saturating_mul(u64::from(quantity.max(1)))
}

fn stalemate_tie_break(state: &AftermathState, participant_id: &str) -> u64 {
    hash64(&RollKey::new(
        state.battle.session_seed.clone(),
        "stalemate_tie_break",
        state.session.current_turn,
        "system:stalemate",
        participant_id,
        state.session.session_id.as_str(),
        0,
    )) % 1_000
}

fn finish_match(
    state: &mut AftermathState,
    winner: Option<String>,
    finish_reason: &str,
    command_id: &str,
    finished_at: u64,
    scores: Vec<VictoryScore>,
) -> Result<VictoryCheck, AftermathError> {
    if state.session.state == "finished" {
        return Ok(VictoryCheck {
            finalized: true,
            winner_participant_id: state.session.winner_participant_id.clone(),
            finish_reason: state.session.finish_reason.clone(),
            scores,
        });
    }
    state.session.state = "finished".to_string();
    state.session.winner_participant_id = winner.clone();
    state.session.finish_reason = Some(finish_reason.to_string());
    state.session.last_command_id = Some(command_id.to_string());
    write_match_summaries(state, winner.as_deref(), finish_reason, finished_at)?;
    append_aftermath_event(
        state,
        command_id,
        "match_finished",
        format!(
            "match finished by {} with winner {}",
            finish_reason,
            winner.as_deref().unwrap_or("draw")
        ),
    );
    Ok(VictoryCheck {
        finalized: true,
        winner_participant_id: winner,
        finish_reason: Some(finish_reason.to_string()),
        scores,
    })
}

fn write_match_summaries(
    state: &mut AftermathState,
    winner: Option<&str>,
    finish_reason: &str,
    finished_at: u64,
) -> Result<(), AftermathError> {
    let fixture = first_playable_fixture();
    let players = [
        (
            fixture.ids.player_one_id,
            fixture.ids.participant_one_id,
            Some("Mayhem Two".to_string()),
        ),
        (
            fixture.ids.player_two_id,
            fixture.ids.participant_two_id,
            Some("Misery One".to_string()),
        ),
    ];
    for (player_id, participant_id, opponent_name) in players {
        if state
            .player_match_summaries
            .iter()
            .any(|summary| summary.player_id == player_id)
        {
            continue;
        }
        let result = match winner {
            Some(winner) if winner == participant_id => "win",
            Some(_) => "loss",
            None => "draw",
        };
        let summary_json = format!(
            "{{\"finish_reason\":\"{}\",\"participant_id\":\"{}\"}}",
            escape_json(finish_reason),
            escape_json(&participant_id)
        );
        let entry = MatchHistoryEntry {
            session_id: state.session.session_id.clone(),
            result: result.to_string(),
            opponent_name: opponent_name.clone(),
            turns_played: state.session.current_turn,
            summary_json: Some(summary_json.clone()),
        };
        state.match_history.push((player_id.clone(), entry));
        state.player_match_summaries.push(PlayerMatchSummaryRecord {
            summary_id: format!("match-summary:{}:{player_id}", state.session.session_id),
            player_id,
            session_id: state.session.session_id.clone(),
            result: result.to_string(),
            opponent_name,
            turns_played: state.session.current_turn,
            summary_json: Some(summary_json),
            finished_at,
        });
    }
    Ok(())
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn append_aftermath_event(
    state: &mut AftermathState,
    command_id: &str,
    event_type: &str,
    summary: String,
) {
    if state
        .aftermath_events
        .iter()
        .any(|event| event.command_id == command_id && event.event_type == event_type)
    {
        return;
    }
    let sequence = state.aftermath_events.len() as u64 + 1;
    state.aftermath_events.push(AftermathEventRecord {
        event_id: format!("aftermath-event:{}:{sequence}", state.session.session_id),
        sequence,
        command_id: command_id.to_string(),
        event_type: event_type.to_string(),
        summary,
    });
}
