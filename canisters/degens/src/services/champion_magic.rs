use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{Champion, GameCommand, SpellDefinition};
use domm_game::{
    ApiError, ChampionMagicReceipt, ChampionProgressionView, ChampionSkillChoiceView,
    ChangedSubject, CommandResponse, CommandResult,
};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{champions_artifacts, content};

use super::{
    command_response,
    session_context::{self, public_error},
    session_turn_runtime,
};

const SKILL_SOUR_SORCERY: &str = "sour_sorcery";
const SKILL_DIRTY_TACTICS: &str = "dirty_tactics";
const SKILL_GRIM_LOGISTICS: &str = "grim_logistics";

pub(crate) fn preview_champion_progression(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
) -> Result<ChampionProgressionView, ApiError> {
    let context = session_context::require_session_caller_runtime_first(caller, &session_id)?;
    let champion_id = session_context::parse_id::<Champion>(&champion_id, "champion_id")?;
    let champion = require_owned_champion(&context, champion_id)?;
    progression_view(&champion, context.session.current_turn)
}

pub(crate) fn select_champion_level_up(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    skill_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let champion_id = session_context::parse_id::<Champion>(&champion_id, "champion_id")?;
    let champion = require_owned_champion(&context, champion_id)?;
    let payload_json = format!(
        r#"{{"champion_id":"{}","skill_key":"{}"}}"#,
        command_response::escape_json(&champion.id().to_string()),
        command_response::escape_json(&skill_key)
    );
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "select_champion_level_up",
        &client_nonce,
        Some(champion.id()),
        payload_json,
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    match apply_level_choice(&context, command.clone(), champion, &skill_key) {
        Ok((receipt, events, changed)) => command_response::apply_runtime_command_with_result(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            magic_receipt_json(&receipt),
            events,
            changed,
            CommandResult::ChampionMagic(receipt),
        ),
        Err(error) => command_response::fail_runtime_command(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            error,
        ),
    }
}

pub(crate) fn learn_champion_spell(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    spell_slug: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let champion_id = session_context::parse_id::<Champion>(&champion_id, "champion_id")?;
    let champion = require_owned_champion(&context, champion_id)?;
    let payload_json = format!(
        r#"{{"champion_id":"{}","spell_slug":"{}"}}"#,
        command_response::escape_json(&champion.id().to_string()),
        command_response::escape_json(&spell_slug)
    );
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "learn_champion_spell",
        &client_nonce,
        Some(champion.id()),
        payload_json,
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    match apply_spell_learning(&context, command.clone(), champion, &spell_slug) {
        Ok((receipt, events, changed)) => command_response::apply_runtime_command_with_result(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            magic_receipt_json(&receipt),
            events,
            changed,
            CommandResult::ChampionMagic(receipt),
        ),
        Err(error) => command_response::fail_runtime_command(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            error,
        ),
    }
}

pub(crate) fn cast_adventure_spell(
    caller: CandidPrincipal,
    session_id: String,
    champion_id: String,
    spell_slug: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let context =
        session_context::require_active_session_caller_runtime_first(caller, &session_id)?;
    let champion_id = session_context::parse_id::<Champion>(&champion_id, "champion_id")?;
    let champion = require_owned_champion(&context, champion_id)?;
    let payload_json = format!(
        r#"{{"champion_id":"{}","spell_slug":"{}"}}"#,
        command_response::escape_json(&champion.id().to_string()),
        command_response::escape_json(&spell_slug)
    );
    let (command, runtime_receipt) = match command_response::begin_runtime_participant_command(
        caller,
        &context,
        "cast_adventure_spell",
        &client_nonce,
        Some(champion.id()),
        payload_json,
    )? {
        command_response::RuntimeGameCommandAction::Apply {
            command,
            runtime_receipt,
        } => (command, runtime_receipt),
        command_response::RuntimeGameCommandAction::Return(response) => return Ok(response),
    };
    match apply_adventure_cast(&context, command.clone(), champion, &spell_slug) {
        Ok((receipt, events, changed)) => command_response::apply_runtime_command_with_result(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            magic_receipt_json(&receipt),
            events,
            changed,
            CommandResult::ChampionMagic(receipt),
        ),
        Err(error) => command_response::fail_runtime_command(
            caller,
            &context,
            command,
            runtime_receipt,
            &client_nonce,
            error,
        ),
    }
}

fn apply_level_choice(
    context: &session_context::SessionCallerContext,
    command: GameCommand,
    mut champion: Champion,
    skill_key: &str,
) -> Result<
    (
        ChampionMagicReceipt,
        Vec<domm_game::ApiEventView>,
        Vec<ChangedSubject>,
    ),
    ApiError,
> {
    if champion.last_command_id == Some(command.id().key())
        && champion.skill_keys.iter().any(|key| key == skill_key)
    {
        return Ok((
            receipt(
                &command,
                &champion,
                "select_champion_level_up",
                Some(skill_key),
                None,
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
        ));
    }
    if !matches!(
        skill_key,
        SKILL_SOUR_SORCERY | SKILL_DIRTY_TACTICS | SKILL_GRIM_LOGISTICS
    ) {
        return Err(public_error(
            "invalid_skill_choice",
            format!("skill choice is not legal: {skill_key}"),
            false,
        ));
    }
    if champion.skill_points == 0 {
        return Err(public_error(
            "no_pending_skill_point",
            "champion has no pending skill point",
            false,
        ));
    }
    if champion.skill_keys.iter().any(|key| key == skill_key) {
        return Err(public_error(
            "skill_already_selected",
            "champion already selected this skill",
            false,
        ));
    }
    let attempted = champion.skill_keys.len().saturating_add(1);
    if attempted > domm_game::CHAMPION_SKILL_CAP {
        return Err(public_error(
            "skill_cap_exceeded",
            "champion skill cap would be exceeded",
            false,
        ));
    }
    champion.skill_keys.push(skill_key.to_string());
    champion.skill_keys.sort();
    champion.skill_points = champion.skill_points.saturating_sub(1);
    match skill_key {
        SKILL_SOUR_SORCERY => {
            champion.wisdom = champion.wisdom.saturating_add(1);
            champion.mana_max = champion.mana_max.saturating_add(2);
        }
        SKILL_DIRTY_TACTICS => {
            champion.might = champion.might.saturating_add(1);
        }
        SKILL_GRIM_LOGISTICS => {
            champion.command = champion.command.saturating_add(1);
            champion.movement_max = champion.movement_max.saturating_add(10);
        }
        _ => {}
    }
    champion.last_command_id = Some(command.id().key());
    champion = champions_artifacts::update_champion(champion)?;
    session_turn_runtime::mirror_champion_update(&champion);
    command_response::create_fresh_command_effect(
        context.session.id(),
        command.id(),
        format!("skill:{skill_key}:{}", champion.id()),
        "champion_skill_choice".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        format!(
            r#"{{"skill_key":"{}"}}"#,
            command_response::escape_json(skill_key)
        ),
    )?;
    let mut session = context.session.clone();
    let event = command_response::append_fresh_public_event(
        &mut session,
        command.id(),
        format!("champion_skill:{}:{skill_key}", champion.id()),
        "champion_skill_selected".to_string(),
        Some("champion".to_string()),
        Some(champion.id().to_string()),
        format!(
            r#"{{"skill_key":"{}"}}"#,
            command_response::escape_json(skill_key)
        ),
    )?;
    Ok((
        receipt(
            &command,
            &champion,
            "select_champion_level_up",
            Some(skill_key),
            None,
            Vec::new(),
        ),
        vec![event],
        vec![command_response::changed(
            "champion",
            &champion.id().to_string(),
            "update",
        )],
    ))
}

fn apply_spell_learning(
    context: &session_context::SessionCallerContext,
    command: GameCommand,
    mut champion: Champion,
    spell_slug: &str,
) -> Result<
    (
        ChampionMagicReceipt,
        Vec<domm_game::ApiEventView>,
        Vec<ChangedSubject>,
    ),
    ApiError,
> {
    let spell =
        content::find_spell_by_ruleset_slug(Id::from_key(context.session.ruleset_id), spell_slug)?
            .ok_or_else(|| {
                public_error("spell_not_found", "spell definition was not found", false)
            })?;
    if !champion
        .skill_keys
        .iter()
        .any(|key| key == SKILL_SOUR_SORCERY)
    {
        return Err(public_error(
            "spell_prerequisite_missing",
            "Sour Sorcery is required to learn first-tier spells",
            false,
        ));
    }
    if let Some(existing) = champions_artifacts::find_champion_spell(champion.id(), spell.id())? {
        if existing.last_command_id == Some(command.id().key()) {
            return Ok((
                receipt(
                    &command,
                    &champion,
                    "learn_champion_spell",
                    None,
                    Some(spell_slug),
                    Vec::new(),
                ),
                Vec::new(),
                Vec::new(),
            ));
        }
        return Err(public_error(
            "spell_already_learned",
            "champion already knows this spell",
            false,
        ));
    }
    let known_count =
        champions_artifacts::page_champion_spells(champion.id(), domm_game::MAX_LIST_LIMIT, None)?
            .items
            .len();
    if known_count.saturating_add(1) > domm_game::CHAMPION_SPELLBOOK_CAP {
        return Err(public_error(
            "spellbook_cap_exceeded",
            "champion spellbook cap would be exceeded",
            false,
        ));
    }
    champions_artifacts::create_champion_spell(
        context.session.id(),
        champion.id(),
        spell.id(),
        &spell.slug,
        context.session.current_turn,
        command.id(),
    )?;
    champion.last_command_id = Some(command.id().key());
    champion = champions_artifacts::update_champion(champion)?;
    session_turn_runtime::mirror_champion_update(&champion);
    command_response::create_fresh_command_effect(
        context.session.id(),
        command.id(),
        format!("learn_spell:{spell_slug}:{}", champion.id()),
        "champion_spell_learned".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        format!(
            r#"{{"spell_slug":"{}"}}"#,
            command_response::escape_json(spell_slug)
        ),
    )?;
    let mut session = context.session.clone();
    let event = command_response::append_fresh_public_event(
        &mut session,
        command.id(),
        format!("champion_spell_learned:{}:{spell_slug}", champion.id()),
        "champion_spell_learned".to_string(),
        Some("champion".to_string()),
        Some(champion.id().to_string()),
        format!(
            r#"{{"spell_slug":"{}"}}"#,
            command_response::escape_json(spell_slug)
        ),
    )?;
    Ok((
        receipt(
            &command,
            &champion,
            "learn_champion_spell",
            None,
            Some(spell_slug),
            Vec::new(),
        ),
        vec![event],
        vec![command_response::changed(
            "champion",
            &champion.id().to_string(),
            "update",
        )],
    ))
}

fn apply_adventure_cast(
    context: &session_context::SessionCallerContext,
    command: GameCommand,
    mut champion: Champion,
    spell_slug: &str,
) -> Result<
    (
        ChampionMagicReceipt,
        Vec<domm_game::ApiEventView>,
        Vec<ChangedSubject>,
    ),
    ApiError,
> {
    let spell = require_known_spell(&champion, spell_slug, context)?;
    if spell.target_type != "self_champion" {
        return Err(public_error(
            "invalid_spell_target",
            "adventure spell must target the casting champion",
            false,
        ));
    }
    if champion.last_command_id == Some(command.id().key()) {
        return Ok((
            receipt(
                &command,
                &champion,
                "cast_adventure_spell",
                None,
                Some(spell_slug),
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
        ));
    }
    let available_mana = if champion.mana_turn == context.session.current_turn {
        champion.mana
    } else {
        champion.mana_max
    };
    if spell.mana_cost > available_mana {
        return Err(public_error(
            "insufficient_mana",
            "champion does not have enough mana",
            false,
        ));
    }
    champion.mana_turn = context.session.current_turn;
    champion.mana = available_mana - spell.mana_cost;
    if spell.effect_key == "spell:spite_march_movement_30" {
        let available_movement = if champion.movement_turn == context.session.current_turn {
            champion.movement_remaining
        } else {
            champion.movement_max
        };
        champion.movement_turn = context.session.current_turn;
        champion.movement_remaining = available_movement
            .saturating_add(30)
            .min(champion.movement_max);
    }
    champion.last_command_id = Some(command.id().key());
    champion = champions_artifacts::update_champion(champion)?;
    session_turn_runtime::mirror_champion_update(&champion);
    command_response::create_fresh_command_effect(
        context.session.id(),
        command.id(),
        format!("cast_adventure_spell:{spell_slug}:{}", champion.id()),
        "adventure_spell_cast".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        format!(
            r#"{{"spell_slug":"{}"}}"#,
            command_response::escape_json(spell_slug)
        ),
    )?;
    let mut session = context.session.clone();
    let event = command_response::append_fresh_public_event(
        &mut session,
        command.id(),
        format!(
            "adventure_spell:{}:{spell_slug}:{}",
            champion.id(),
            command.id()
        ),
        "adventure_spell_cast".to_string(),
        Some("champion".to_string()),
        Some(champion.id().to_string()),
        format!(
            r#"{{"spell_slug":"{}","mana_after":{}}}"#,
            command_response::escape_json(spell_slug),
            champion.mana
        ),
    )?;
    Ok((
        receipt(
            &command,
            &champion,
            "cast_adventure_spell",
            None,
            Some(spell_slug),
            Vec::new(),
        ),
        vec![event],
        vec![command_response::changed(
            "champion",
            &champion.id().to_string(),
            "update",
        )],
    ))
}

fn progression_view(
    champion: &Champion,
    current_turn: u32,
) -> Result<ChampionProgressionView, ApiError> {
    Ok(ChampionProgressionView {
        champion_id: champion.id().to_string(),
        level: champion.level,
        experience: champion.experience,
        skill_points: champion.skill_points,
        skill_keys: champion.skill_keys.clone(),
        mana: if champion.mana_turn == current_turn {
            champion.mana
        } else {
            champion.mana_max
        },
        mana_max: champion.mana_max,
        mana_turn: current_turn,
        learned_spell_slugs: learned_spell_slugs(champion.id())?,
        level_up_choices: skill_choices(champion),
    })
}

fn skill_choices(champion: &Champion) -> Vec<ChampionSkillChoiceView> {
    [
        (
            SKILL_SOUR_SORCERY,
            "Sour Sorcery",
            "Unlocks first-tier misery spells and adds one wisdom.",
        ),
        (
            SKILL_DIRTY_TACTICS,
            "Dirty Tactics",
            "Adds one might for direct stack damage.",
        ),
        (
            SKILL_GRIM_LOGISTICS,
            "Grim Logistics",
            "Adds one command and ten maximum movement.",
        ),
    ]
    .into_iter()
    .map(|(skill_key, name, description)| {
        let already_selected = champion.skill_keys.iter().any(|key| key == skill_key);
        ChampionSkillChoiceView {
            skill_key: skill_key.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            rank: u8::from(already_selected),
            enabled: champion.skill_points > 0 && !already_selected,
            disabled_reason: if champion.skill_points == 0 {
                Some("no_pending_skill_point".to_string())
            } else if already_selected {
                Some("skill_already_selected".to_string())
            } else {
                None
            },
        }
    })
    .collect()
}

fn learned_spell_slugs(champion_id: Id<Champion>) -> Result<Vec<String>, ApiError> {
    let page =
        champions_artifacts::page_champion_spells(champion_id, domm_game::MAX_LIST_LIMIT, None)?;
    let mut slugs = Vec::new();
    for known in page.items {
        if let Some(slug) = known.spell_slug.as_deref().filter(|slug| !slug.is_empty()) {
            slugs.push(slug.to_string());
            continue;
        }
        let spell = content::load_spell(Id::from_key(known.spell_id))?.ok_or_else(|| {
            public_error(
                "spell_not_found",
                "known spell definition was not found",
                false,
            )
        })?;
        slugs.push(spell.slug);
    }
    slugs.sort();
    Ok(slugs)
}

fn require_known_spell(
    champion: &Champion,
    spell_slug: &str,
    context: &session_context::SessionCallerContext,
) -> Result<SpellDefinition, ApiError> {
    let spell =
        content::find_spell_by_ruleset_slug(Id::from_key(context.session.ruleset_id), spell_slug)?
            .ok_or_else(|| {
                public_error("spell_not_found", "spell definition was not found", false)
            })?;
    if champions_artifacts::find_champion_spell(champion.id(), spell.id())?.is_none() {
        return Err(public_error(
            "spell_not_learned",
            "champion has not learned this spell",
            false,
        ));
    }
    Ok(spell)
}

fn require_owned_champion(
    context: &session_context::SessionCallerContext,
    champion_id: Id<Champion>,
) -> Result<Champion, ApiError> {
    let champion_id_text = champion_id.to_string();
    let champion = match session_turn_runtime::champion_snapshot(
        &context.session.id().to_string(),
        &champion_id_text,
    ) {
        Some(champion) => champion,
        None => champions_artifacts::load_champion(champion_id)?
            .ok_or_else(|| public_error("champion_not_found", "champion was not found", false))?,
    };
    if champion.session_id != context.session.id().key() {
        return Err(public_error(
            "champion_wrong_session",
            "champion does not belong to this session",
            false,
        ));
    }
    if champion.participant_id != context.participant.id().key() {
        return Err(public_error(
            "champion_not_owned",
            "champion does not belong to the caller",
            false,
        ));
    }
    Ok(champion)
}

fn receipt(
    command: &GameCommand,
    champion: &Champion,
    action: &str,
    skill_key: Option<&str>,
    spell_slug: Option<&str>,
    status_keys: Vec<String>,
) -> ChampionMagicReceipt {
    ChampionMagicReceipt {
        command_id: command.id().to_string(),
        champion_id: champion.id().to_string(),
        action: action.to_string(),
        skill_key: skill_key.map(str::to_string),
        spell_slug: spell_slug.map(str::to_string),
        mana_after: champion.mana,
        movement_remaining_after: champion.movement_remaining,
        status_keys,
    }
}

fn magic_receipt_json(receipt: &ChampionMagicReceipt) -> String {
    format!(
        r#"{{"command_kind":"{}","champion_id":"{}","action":"{}","skill_key":{},"spell_slug":{},"mana_after":{},"movement_remaining_after":{},"command_count":1,"event_count":1}}"#,
        command_response::escape_json(&receipt.action),
        command_response::escape_json(&receipt.champion_id),
        command_response::escape_json(&receipt.action),
        receipt
            .skill_key
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        receipt
            .spell_slug
            .as_deref()
            .map(json_string)
            .unwrap_or_else(|| "null".to_string()),
        receipt.mana_after,
        receipt.movement_remaining_after
    )
}

fn json_string(value: &str) -> String {
    format!(r#""{}""#, command_response::escape_json(value))
}
