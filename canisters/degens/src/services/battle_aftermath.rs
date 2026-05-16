use domm_degens_schema::schema::{
    Battle, Champion, ChampionArmyStack, GameCommand, GameParticipant, GameSession, MapOccupancy,
    NeutralArmy, PlayerAccount, Town, UnitDefinition,
};
use domm_game::{ApiError, ApiEventView, ChangedSubject, MAX_LIST_LIMIT};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{
    aftermath_history, battles, champions_artifacts, cleanup, commands_events_effects, content,
    map_visibility_occupancy, neutrals, players, sessions, towns,
};

use super::{
    command_response,
    session_context::{self, public_error},
};

pub(crate) fn apply_resolved_battle_aftermath(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle_id: Id<Battle>,
    events: &mut Vec<ApiEventView>,
    changed_subjects: &mut Vec<ChangedSubject>,
) -> Result<(), ApiError> {
    let battle = battles::load_battle(battle_id)?.ok_or_else(|| {
        public_error(
            "battle_not_found",
            format!("battle was not found: {battle_id}"),
            true,
        )
    })?;
    if battle.state != "resolved" {
        return Ok(());
    }
    let effect_key = format!("battle_aftermath:{battle_id}");
    if commands_events_effects::find_command_effect(command_id, &effect_key)?.is_some() {
        finalize_victory_if_ready(session, command_id, events, changed_subjects)?;
        return Ok(());
    }

    match battle.battle_type.as_str() {
        "neutral" => {
            apply_neutral_aftermath(session, command_id, &battle, events, changed_subjects)?
        }
        "town" => apply_town_aftermath(session, command_id, &battle, events, changed_subjects)?,
        "champion" => {
            apply_champion_aftermath(session, command_id, &battle, events, changed_subjects)?
        }
        _ => {}
    }

    command_response::ensure_command_effect(
        session.id(),
        command_id,
        effect_key,
        "battle_aftermath".to_string(),
        "battle".to_string(),
        battle_id.to_string(),
        format!(
            r#"{{"battle_id":"{}","battle_type":"{}"}}"#,
            battle_id,
            command_response::escape_json(&battle.battle_type)
        ),
    )?;
    let event = command_response::append_public_event(
        session,
        command_id,
        format!("battle_aftermath:{battle_id}"),
        "battle_aftermath_applied".to_string(),
        Some("battle".to_string()),
        Some(battle_id.to_string()),
        format!(
            r#"{{"battle_id":"{}","battle_type":"{}","winner_participant_id":{}}}"#,
            battle_id,
            command_response::escape_json(&battle.battle_type),
            json_opt_id::<GameParticipant>(battle.winner_participant_id)
        ),
    )?;
    events.push(event);
    changed_subjects.push(command_response::changed(
        "battle",
        &battle_id.to_string(),
        "aftermath",
    ));

    finalize_victory_if_ready(session, command_id, events, changed_subjects)
}

fn apply_neutral_aftermath(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle: &Battle,
    events: &mut Vec<ApiEventView>,
    changed_subjects: &mut Vec<ChangedSubject>,
) -> Result<(), ApiError> {
    let Some(winner_id) = battle
        .winner_participant_id
        .map(Id::<GameParticipant>::from_key)
    else {
        return Ok(());
    };
    let Some(neutral_id) = battle
        .defender_neutral_army_id
        .map(Id::<NeutralArmy>::from_key)
    else {
        return Ok(());
    };
    let Some(champion_id) = battle.attacker_champion_id.map(Id::<Champion>::from_key) else {
        return Ok(());
    };

    write_champion_survivors(battle.id(), command_id)?;
    let mut neutral = neutrals::load_neutral_army(neutral_id)?
        .ok_or_else(|| public_error("neutral_army_not_found", "neutral army not found", true))?;
    neutral.state = "defeated".to_string();
    neutral.last_command_id = Some(command_id.key());
    neutral = neutrals::update_neutral_army(neutral)?;
    cleanup_occupancy_by_occupant(session.id(), "neutral_army", &neutral_id.to_string())?;

    let mut champion = champions_artifacts::load_champion(champion_id)?
        .ok_or_else(|| public_error("champion_not_found", "champion not found", true))?;
    champion.status = "active".to_string();
    champion.in_battle_id = None;
    champion.x = neutral.x;
    champion.y = neutral.y;
    champion.chunk_x = chunk_coord(session, champion.x);
    champion.chunk_y = chunk_coord(session, champion.y);
    champion.experience = champion.experience.saturating_add(250);
    champion.last_command_id = Some(command_id.key());
    champion = champions_artifacts::update_champion(champion)?;
    move_champion_occupancy(session.id(), command_id, &champion)?;

    capture_guarded_object_if_present(session, command_id, winner_id, neutral.x, neutral.y)?;

    let event = command_response::append_public_event(
        session,
        command_id,
        format!("neutral_defeated:{neutral_id}:{}", battle.id()),
        "neutral_defeated".to_string(),
        Some("neutral_army".to_string()),
        Some(neutral_id.to_string()),
        format!(
            r#"{{"battle_id":"{}","neutral_army_id":"{}","champion_id":"{}"}}"#,
            battle.id(),
            neutral_id,
            champion.id()
        ),
    )?;
    events.push(event);
    changed_subjects.push(command_response::changed(
        "neutral_army",
        &neutral_id.to_string(),
        "defeated",
    ));
    changed_subjects.push(command_response::changed(
        "champion",
        &champion_id.to_string(),
        "active",
    ));
    Ok(())
}

fn apply_town_aftermath(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle: &Battle,
    events: &mut Vec<ApiEventView>,
    changed_subjects: &mut Vec<ChangedSubject>,
) -> Result<(), ApiError> {
    let Some(winner_id) = battle
        .winner_participant_id
        .map(Id::<GameParticipant>::from_key)
    else {
        return Ok(());
    };
    let Some(town_id) = battle.defender_town_id.map(Id::<Town>::from_key) else {
        return Ok(());
    };
    let Some(champion_id) = battle.attacker_champion_id.map(Id::<Champion>::from_key) else {
        return Ok(());
    };

    let mut town = towns::load_town(town_id)?
        .ok_or_else(|| public_error("town_not_found", "town not found", true))?;
    town.owner_participant_id = Some(winner_id.key());
    town.captured_turn = battle.created_turn;
    town.income_started_turn = battle.created_turn;
    town.unrest_until_turn = battle.created_turn.saturating_add(2);
    town.last_command_id = Some(command_id.key());
    town = towns::update_town(town)?;
    write_town_garrison_survivors(session.id(), battle.id(), town_id, command_id)?;

    let mut champion = champions_artifacts::load_champion(champion_id)?
        .ok_or_else(|| public_error("champion_not_found", "champion not found", true))?;
    champion.status = "active".to_string();
    champion.in_battle_id = None;
    champion.x = town.x;
    champion.y = town.y;
    champion.chunk_x = chunk_coord(session, champion.x);
    champion.chunk_y = chunk_coord(session, champion.y);
    champion.last_command_id = Some(command_id.key());
    champion = champions_artifacts::update_champion(champion)?;
    move_champion_occupancy(session.id(), command_id, &champion)?;

    let event = command_response::append_public_event(
        session,
        command_id,
        format!("town_captured:{town_id}:{}", battle.id()),
        "town_captured".to_string(),
        Some("town".to_string()),
        Some(town_id.to_string()),
        format!(
            r#"{{"battle_id":"{}","town_id":"{}","owner_participant_id":"{}"}}"#,
            battle.id(),
            town_id,
            winner_id
        ),
    )?;
    events.push(event);
    changed_subjects.push(command_response::changed(
        "town",
        &town_id.to_string(),
        "capture",
    ));
    Ok(())
}

fn apply_champion_aftermath(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    battle: &Battle,
    events: &mut Vec<ApiEventView>,
    changed_subjects: &mut Vec<ChangedSubject>,
) -> Result<(), ApiError> {
    let Some(winner_id) = battle
        .winner_participant_id
        .map(Id::<GameParticipant>::from_key)
    else {
        return Ok(());
    };
    let champion_ids = [
        battle.attacker_champion_id.map(Id::<Champion>::from_key),
        battle.defender_champion_id.map(Id::<Champion>::from_key),
    ];
    let mut loaded = Vec::new();
    for champion_id in champion_ids.into_iter().flatten() {
        if let Some(champion) = champions_artifacts::load_champion(champion_id)? {
            loaded.push(champion);
        }
    }
    let Some(victor) = loaded
        .iter()
        .find(|champion| champion.participant_id == winner_id.key())
        .cloned()
    else {
        return Ok(());
    };
    let Some(defeated) = loaded
        .iter()
        .find(|champion| champion.participant_id != winner_id.key())
        .cloned()
    else {
        return Ok(());
    };

    write_champion_survivors(battle.id(), command_id)?;
    let mut defeated = defeated;
    defeated.status = "defeated".to_string();
    defeated.in_battle_id = None;
    defeated.defeated_turn = battle.created_turn;
    defeated.last_command_id = Some(command_id.key());
    defeated = champions_artifacts::update_champion(defeated)?;
    cleanup_occupancy_by_occupant(session.id(), "champion", &defeated.id().to_string())?;

    let mut victor = victor;
    victor.status = "active".to_string();
    victor.in_battle_id = None;
    victor.x = defeated.x;
    victor.y = defeated.y;
    victor.chunk_x = chunk_coord(session, victor.x);
    victor.chunk_y = chunk_coord(session, victor.y);
    victor.last_command_id = Some(command_id.key());
    victor = champions_artifacts::update_champion(victor)?;
    move_champion_occupancy(session.id(), command_id, &victor)?;
    capture_artifacts(command_id, victor.id(), defeated.id())?;

    let event = command_response::append_public_event(
        session,
        command_id,
        format!("champion_defeated:{}:{}", defeated.id(), battle.id()),
        "champion_defeated".to_string(),
        Some("champion".to_string()),
        Some(defeated.id().to_string()),
        format!(
            r#"{{"battle_id":"{}","victor_champion_id":"{}","defeated_champion_id":"{}"}}"#,
            battle.id(),
            victor.id(),
            defeated.id()
        ),
    )?;
    events.push(event);
    changed_subjects.push(command_response::changed(
        "champion",
        &defeated.id().to_string(),
        "defeated",
    ));
    Ok(())
}

fn write_champion_survivors(
    battle_id: Id<Battle>,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    for battle_stack in battles::page_battle_stacks(battle_id, MAX_LIST_LIMIT, None)?.items {
        if battle_stack.origin_kind != "champion_army" {
            continue;
        }
        let Some(origin_id_text) = battle_stack.origin_stack_id_text.as_deref() else {
            continue;
        };
        let origin_id =
            session_context::parse_id::<ChampionArmyStack>(origin_id_text, "origin_stack_id")?;
        let Some(mut stack) = champions_artifacts::load_champion_army_stack(origin_id)? else {
            continue;
        };
        stack.quantity = battle_stack.quantity;
        stack.front_hp = battle_stack.front_hp;
        stack.status = battle_stack.status;
        stack.last_command_id = Some(command_id.key());
        champions_artifacts::update_champion_army_stack(stack)?;
    }
    Ok(())
}

fn write_town_garrison_survivors(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    town_id: Id<Town>,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    for stack in towns::page_town_garrison(town_id, MAX_LIST_LIMIT, None)?.items {
        towns::delete_town_garrison_stack(stack.id())?;
    }
    for (slot_index, battle_stack) in battles::page_battle_stacks(battle_id, MAX_LIST_LIMIT, None)?
        .items
        .into_iter()
        .filter(|stack| stack.side == "attacker" && stack.status == "active" && stack.quantity > 0)
        .enumerate()
    {
        let unit = content::load_unit(Id::<UnitDefinition>::from_key(battle_stack.unit_id))?
            .ok_or_else(|| {
                ApiError::new(
                    "unit_not_found",
                    "battle stack unit definition was not found",
                    true,
                )
            })?;
        let mut stack = towns::create_town_garrison_stack(
            session_id,
            town_id,
            unit.id(),
            unit.slug,
            u8::try_from(slot_index).unwrap_or(u8::MAX),
            battle_stack.quantity,
            battle_stack.front_hp,
        )?;
        stack.last_command_id = Some(command_id.key());
        towns::update_town_garrison_stack(stack)?;
    }
    Ok(())
}

fn capture_artifacts(
    command_id: Id<GameCommand>,
    victor_id: Id<Champion>,
    defeated_id: Id<Champion>,
) -> Result<(), ApiError> {
    for mut equipment in
        champions_artifacts::page_artifact_equipment_by_champion(defeated_id, MAX_LIST_LIMIT, None)?
            .items
    {
        if champions_artifacts::find_equipment_by_champion_slot(victor_id, &equipment.slot)?
            .is_some()
        {
            continue;
        }
        let artifact_id = equipment.artifact_id;
        equipment.champion_id = victor_id.key();
        equipment.last_command_id = Some(command_id.key());
        champions_artifacts::update_artifact_equipment(equipment)?;
        if let Some(mut artifact) =
            champions_artifacts::load_artifact_instance(Id::from_key(artifact_id))?
        {
            artifact.owner_champion_id = Some(victor_id.key());
            artifact.last_command_id = Some(command_id.key());
            champions_artifacts::update_artifact_instance(artifact)?;
        }
    }
    Ok(())
}

fn capture_guarded_object_if_present(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    winner_id: Id<GameParticipant>,
    x: u16,
    y: u16,
) -> Result<(), ApiError> {
    let Some(mut object) =
        map_visibility_occupancy::find_world_object_by_session_xy(session.id(), x, y)?
    else {
        return Ok(());
    };
    object.guarded_neutral_army_id = None;
    object.owner_participant_id = Some(winner_id.key());
    object.captured_turn = session.current_turn;
    object.income_started_turn = session.current_turn;
    object.last_visited_turn = session.current_turn;
    object.last_command_id = Some(command_id.key());
    map_visibility_occupancy::update_world_object(object)?;
    Ok(())
}

fn finalize_victory_if_ready(
    session: &mut GameSession,
    command_id: Id<GameCommand>,
    events: &mut Vec<ApiEventView>,
    changed_subjects: &mut Vec<ChangedSubject>,
) -> Result<(), ApiError> {
    if session.state == "finished" {
        return Ok(());
    }
    if !battles::page_battles_by_session_state(session.id(), "active", 1, None)?
        .items
        .is_empty()
    {
        return Ok(());
    }

    let participants = session_context::participants_for_session(session.id())?;
    let mut live = Vec::new();
    for participant in &participants {
        if participant_has_live_assets(session.id(), participant.id())? {
            live.push(participant.id());
        }
    }
    let winner = if live.len() == 1 {
        Some(live[0])
    } else if session.current_turn >= session.max_turns {
        score_winner(session.id(), &participants)?
    } else {
        None
    };
    let Some(winner_id) = winner else {
        return Ok(());
    };

    let effect_key = format!("victory_finalized:{}", session.id());
    if commands_events_effects::find_command_effect(command_id, &effect_key)?.is_some() {
        return Ok(());
    }

    session.state = "finished".to_string();
    session.winner_participant_id = Some(winner_id.key());
    session.finish_reason = Some("elimination".to_string());
    session.last_command_id = Some(command_id.key());
    *session = sessions::update_session(session.clone())?;
    write_match_summaries(session, winner_id, &participants)?;
    command_response::ensure_command_effect(
        session.id(),
        command_id,
        effect_key,
        "victory_finalized".to_string(),
        "session".to_string(),
        session.id().to_string(),
        format!(
            r#"{{"winner_participant_id":"{}","finish_reason":"{}"}}"#,
            winner_id,
            command_response::escape_json(
                session.finish_reason.as_deref().unwrap_or("elimination")
            )
        ),
    )?;
    let event = command_response::append_public_event(
        session,
        command_id,
        format!("victory_finalized:{}", session.id()),
        "victory_finalized".to_string(),
        Some("session".to_string()),
        Some(session.id().to_string()),
        format!(
            r#"{{"winner_participant_id":"{}","finish_reason":"{}"}}"#,
            winner_id,
            command_response::escape_json(
                session.finish_reason.as_deref().unwrap_or("elimination")
            )
        ),
    )?;
    events.push(event);
    changed_subjects.push(command_response::changed(
        "session",
        &session.id().to_string(),
        "finished",
    ));
    Ok(())
}

fn participant_has_live_assets(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
) -> Result<bool, ApiError> {
    if !towns::page_towns_by_owner(session_id, participant_id, 1, None)?
        .items
        .is_empty()
    {
        return Ok(true);
    }
    for status in ["active", "in_battle", "garrisoned"] {
        if !champions_artifacts::page_champions_by_owner_status(participant_id, status, 1, None)?
            .items
            .is_empty()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn score_winner(
    session_id: Id<GameSession>,
    participants: &[GameParticipant],
) -> Result<Option<Id<GameParticipant>>, ApiError> {
    let mut scores = Vec::new();
    for participant in participants {
        let town_count =
            towns::page_towns_by_owner(session_id, participant.id(), MAX_LIST_LIMIT, None)?
                .items
                .len() as u64;
        let champion_count = champions_artifacts::page_champions_by_owner_status(
            participant.id(),
            "active",
            MAX_LIST_LIMIT,
            None,
        )?
        .items
        .len() as u64;
        scores.push((
            participant.id(),
            town_count.saturating_mul(100) + champion_count,
        ));
    }
    scores.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.key().cmp(&right.0.key()))
    });
    Ok(scores.first().map(|(participant_id, _)| *participant_id))
}

fn write_match_summaries(
    session: &GameSession,
    winner_id: Id<GameParticipant>,
    participants: &[GameParticipant],
) -> Result<(), ApiError> {
    for participant in participants {
        let result = if participant.id() == winner_id {
            "win"
        } else {
            "loss"
        };
        let opponent_name = opponent_name(participants, participant.id())?;
        let summary_json = Some(format!(
            r#"{{"state":"finished","winner_participant_id":"{}","finish_reason":"{}"}}"#,
            winner_id,
            command_response::escape_json(
                session.finish_reason.as_deref().unwrap_or("elimination")
            )
        ));
        if let Some(mut summary) = aftermath_history::find_match_summary_for_player_session(
            Id::<PlayerAccount>::from_key(participant.player_id),
            session.id(),
        )? {
            summary.result = result.to_string();
            summary.opponent_name = opponent_name;
            summary.turns_played = session.current_turn;
            summary.summary_json = summary_json;
            aftermath_history::update_match_summary(summary)?;
        } else {
            aftermath_history::create_match_summary_shell(
                Id::<PlayerAccount>::from_key(participant.player_id),
                session.id(),
                result.to_string(),
                opponent_name,
                session.current_turn,
                summary_json,
            )?;
        }
    }
    Ok(())
}

fn opponent_name(
    participants: &[GameParticipant],
    participant_id: Id<GameParticipant>,
) -> Result<Option<String>, ApiError> {
    let Some(opponent) = participants
        .iter()
        .find(|participant| participant.id() != participant_id)
    else {
        return Ok(None);
    };
    Ok(
        players::load_player_account(Id::<PlayerAccount>::from_key(opponent.player_id))?
            .and_then(|player| player.display_name.or(player.username)),
    )
}

fn cleanup_occupancy_by_occupant(
    session_id: Id<GameSession>,
    occupant_kind: &str,
    occupant_id_text: &str,
) -> Result<(), ApiError> {
    if let Some(row) = map_visibility_occupancy::find_occupancy_by_occupant(
        session_id,
        occupant_kind,
        occupant_id_text,
        0,
    )? {
        cleanup::delete_row_by_id::<MapOccupancy>("map.delete_occupancy_by_occupant", row.id())?;
    }
    Ok(())
}

fn move_champion_occupancy(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    champion: &Champion,
) -> Result<(), ApiError> {
    cleanup_occupancy_by_occupant(session_id, "champion", &champion.id().to_string())?;
    if let Some(row) = map_visibility_occupancy::find_occupancy_cell(
        session_id, champion.x, champion.y, "champion",
    )? {
        cleanup::delete_row_by_id::<MapOccupancy>("map.delete_occupancy_by_cell", row.id())?;
    }
    let mut row = map_visibility_occupancy::create_occupancy_cell(
        session_id,
        champion.x,
        champion.y,
        champion.chunk_x,
        champion.chunk_y,
        "champion".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        0,
        true,
    )?;
    row.last_command_id = Some(command_id.key());
    map_visibility_occupancy::update_occupancy_cell(row)?;
    Ok(())
}

fn chunk_coord(session: &GameSession, value: u16) -> u16 {
    let chunk_size = u16::from(session.chunk_size.max(1));
    value / chunk_size
}

fn json_opt_id<E>(id: Option<icydb::types::Ulid>) -> String
where
    E: icydb::traits::EntityKey<Key = icydb::types::Ulid>,
{
    id.map(|id| format!(r#""{}""#, Id::<E>::from_key(id)))
        .unwrap_or_else(|| "null".to_string())
}
