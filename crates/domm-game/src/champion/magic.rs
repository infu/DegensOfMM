use crate::battle::{BattleError, BattleState, apply_damage_to_stack};
use crate::content::{ContentManifest, SpellContent};
use crate::effects::validate_status_keys;
use crate::rng::RollKey;

use super::types::{
    CHAMPION_BATTLE_CASTS_PER_ROUND, CHAMPION_SKILL_CAP, CHAMPION_SKILL_OPTIONS_PER_LEVEL,
    CHAMPION_SPELLBOOK_CAP, ChampionError, ChampionMagicReceipt, ChampionProgressionView,
    ChampionSkillChoiceView, ChampionSpellRecord, ChampionState,
};

const SKILL_SOUR_SORCERY: &str = "sour_sorcery";
const SKILL_DIRTY_TACTICS: &str = "dirty_tactics";
const SKILL_GRIM_LOGISTICS: &str = "grim_logistics";
struct SkillDefinition {
    key: &'static str,
    name: &'static str,
    description: &'static str,
}

const SKILLS: &[SkillDefinition] = &[
    SkillDefinition {
        key: SKILL_SOUR_SORCERY,
        name: "Sour Sorcery",
        description: "Unlocks first-tier misery spells and adds one wisdom.",
    },
    SkillDefinition {
        key: SKILL_DIRTY_TACTICS,
        name: "Dirty Tactics",
        description: "Adds one might for direct stack damage.",
    },
    SkillDefinition {
        key: SKILL_GRIM_LOGISTICS,
        name: "Grim Logistics",
        description: "Adds one command and ten maximum movement.",
    },
];

impl ChampionState {
    pub fn progression_view(
        &self,
        champion_id: &str,
        current_turn: u32,
    ) -> Result<ChampionProgressionView, ChampionError> {
        let champion = self.champion(champion_id)?;
        Ok(ChampionProgressionView {
            champion_id: champion.champion_id.clone(),
            level: champion.level,
            experience: champion.experience,
            skill_points: champion.skill_points,
            skill_keys: champion.skill_keys.clone(),
            mana: self.effective_mana(champion_id, current_turn)?,
            mana_max: champion.mana_max,
            mana_turn: if champion.mana_turn == current_turn {
                champion.mana_turn
            } else {
                current_turn
            },
            learned_spell_slugs: self.learned_spell_slugs(champion_id),
            level_up_choices: self.level_up_choices(champion_id)?,
        })
    }

    pub fn level_up_choices(
        &self,
        champion_id: &str,
    ) -> Result<Vec<ChampionSkillChoiceView>, ChampionError> {
        let champion = self.champion(champion_id)?;
        Ok(SKILLS
            .iter()
            .take(CHAMPION_SKILL_OPTIONS_PER_LEVEL)
            .map(|skill| {
                let already_selected = champion.skill_keys.iter().any(|key| key == skill.key);
                ChampionSkillChoiceView {
                    skill_key: skill.key.to_string(),
                    name: skill.name.to_string(),
                    description: skill.description.to_string(),
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
            .collect())
    }

    pub fn select_level_up_choice(
        &mut self,
        champion_id: &str,
        skill_key: &str,
        command_id: &str,
    ) -> Result<ChampionMagicReceipt, ChampionError> {
        if !SKILLS.iter().any(|skill| skill.key == skill_key) {
            return Err(ChampionError::InvalidSkillChoice {
                champion_id: champion_id.to_string(),
                skill_key: skill_key.to_string(),
            });
        }
        let champion = self.champion_mut(champion_id)?;
        if champion.skill_points == 0 {
            return Err(ChampionError::NoPendingSkillPoint {
                champion_id: champion_id.to_string(),
            });
        }
        if champion.skill_keys.iter().any(|key| key == skill_key) {
            return Err(ChampionError::SkillAlreadySelected {
                champion_id: champion_id.to_string(),
                skill_key: skill_key.to_string(),
            });
        }
        let attempted = champion.skill_keys.len().saturating_add(1);
        if attempted > CHAMPION_SKILL_CAP {
            return Err(ChampionError::SkillCapExceeded {
                champion_id: champion_id.to_string(),
                attempted,
            });
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
        champion.last_command_id = Some(command_id.to_string());

        Ok(ChampionMagicReceipt {
            command_id: command_id.to_string(),
            champion_id: champion_id.to_string(),
            action: "select_level_up_choice".to_string(),
            skill_key: Some(skill_key.to_string()),
            spell_slug: None,
            mana_after: champion.mana,
            movement_remaining_after: champion.movement_remaining,
            status_keys: Vec::new(),
        })
    }

    pub fn learn_spell(
        &mut self,
        champion_id: &str,
        manifest: &ContentManifest,
        spell_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<ChampionMagicReceipt, ChampionError> {
        let spell = manifest
            .spell(spell_slug)
            .ok_or_else(|| ChampionError::SpellNotFound {
                spell_slug: spell_slug.to_string(),
            })?;
        let champion = self.champion(champion_id)?;
        if !champion
            .skill_keys
            .iter()
            .any(|key| key == SKILL_SOUR_SORCERY)
        {
            return Err(ChampionError::SpellPrerequisiteMissing {
                champion_id: champion_id.to_string(),
                required_skill_key: SKILL_SOUR_SORCERY.to_string(),
            });
        }
        if self
            .champion_spells
            .iter()
            .any(|known| known.champion_id == champion_id && known.spell_slug == spell_slug)
        {
            return Err(ChampionError::SpellAlreadyLearned {
                champion_id: champion_id.to_string(),
                spell_slug: spell_slug.to_string(),
            });
        }
        let attempted = self
            .champion_spells
            .iter()
            .filter(|known| known.champion_id == champion_id)
            .count()
            .saturating_add(1);
        if attempted > CHAMPION_SPELLBOOK_CAP {
            return Err(ChampionError::SpellbookCapExceeded {
                champion_id: champion_id.to_string(),
                attempted,
            });
        }
        self.champion_spells.push(ChampionSpellRecord {
            champion_spell_id: format!("champion-spell:{champion_id}:{spell_slug}"),
            session_id: self.session_id.clone(),
            champion_id: champion_id.to_string(),
            spell_slug: spell.slug.clone(),
            learned_turn: current_turn,
            last_command_id: Some(command_id.to_string()),
        });
        let champion = self.champion_mut(champion_id)?;
        champion.last_command_id = Some(command_id.to_string());

        Ok(ChampionMagicReceipt {
            command_id: command_id.to_string(),
            champion_id: champion_id.to_string(),
            action: "learn_champion_spell".to_string(),
            skill_key: None,
            spell_slug: Some(spell_slug.to_string()),
            mana_after: champion.mana,
            movement_remaining_after: champion.movement_remaining,
            status_keys: Vec::new(),
        })
    }

    pub fn cast_adventure_spell(
        &mut self,
        champion_id: &str,
        manifest: &ContentManifest,
        spell_slug: &str,
        current_turn: u32,
        command_id: &str,
    ) -> Result<ChampionMagicReceipt, ChampionError> {
        let spell = learned_spell(self, champion_id, manifest, spell_slug)?;
        if spell.target_type != "self_champion" {
            return Err(ChampionError::InvalidSpellTarget {
                spell_slug: spell_slug.to_string(),
                target_type: spell.target_type.clone(),
            });
        }
        self.spend_mana(champion_id, spell.mana_cost, current_turn, command_id)?;
        let champion = self.champion_mut(champion_id)?;
        if spell.effect_key == "spell:spite_march_movement_30" {
            let available = if champion.movement_turn == current_turn {
                champion.movement_remaining
            } else {
                champion.movement_max
            };
            champion.movement_turn = current_turn;
            champion.movement_remaining = available.saturating_add(30).min(champion.movement_max);
        }
        champion.last_command_id = Some(command_id.to_string());
        Ok(ChampionMagicReceipt {
            command_id: command_id.to_string(),
            champion_id: champion_id.to_string(),
            action: "cast_adventure_spell".to_string(),
            skill_key: None,
            spell_slug: Some(spell_slug.to_string()),
            mana_after: champion.mana,
            movement_remaining_after: champion.movement_remaining,
            status_keys: Vec::new(),
        })
    }

    pub fn apply_battle_spell(
        &mut self,
        battle_state: &mut BattleState,
        battle_id: &str,
        caster_champion_id: &str,
        caster_stack_id: &str,
        target_stack_id: &str,
        spell_slug: &str,
        command_id: &str,
        roll_index: u32,
    ) -> Result<ChampionMagicReceipt, ChampionError> {
        let spell = learned_spell(
            self,
            caster_champion_id,
            &crate::first_playable_content_manifest(),
            spell_slug,
        )?;
        if spell.target_type != "enemy_battle_stack" {
            return Err(ChampionError::InvalidSpellTarget {
                spell_slug: spell_slug.to_string(),
                target_type: spell.target_type.clone(),
            });
        }
        let round = u32::from(
            battle_state
                .battle(battle_id)
                .map_err(champion_battle_error)?
                .current_round,
        );
        let caster = battle_state
            .stack(caster_stack_id)
            .map_err(champion_battle_error)?;
        if caster
            .cast_round
            .saturating_add(CHAMPION_BATTLE_CASTS_PER_ROUND)
            > u16::try_from(round).unwrap_or(u16::MAX)
        {
            return Err(ChampionError::InvalidSpellTarget {
                spell_slug: spell_slug.to_string(),
                target_type: "caster_already_cast_this_round".to_string(),
            });
        }
        self.spend_mana(caster_champion_id, spell.mana_cost, round, command_id)?;
        let caster_wisdom = self.champion(caster_champion_id)?.wisdom.max(0) as u64;
        let roll = RollKey::new(
            battle_state.session_seed.clone(),
            "battle_spell_damage",
            round,
            command_id,
            caster_stack_id,
            target_stack_id,
            roll_index,
        )
        .roll_between_inclusive(12, 18)
        .map_err(|error| ChampionError::InvalidSpellTarget {
            spell_slug: spell_slug.to_string(),
            target_type: format!("rng_error:{error}"),
        })?;
        let damage = (roll.value + caster_wisdom).min(u64::from(u32::MAX)) as u32;
        apply_damage_to_stack(battle_state, target_stack_id, damage, command_id)
            .map_err(champion_battle_error)?;
        let battle_round = u16::try_from(round).unwrap_or(u16::MAX);
        let status_key = format!(
            "hexed_until_round:{}",
            battle_round.saturating_add(u16::from(spell.duration_rounds))
        );
        {
            let target = battle_state
                .stack_mut(target_stack_id)
                .map_err(champion_battle_error)?;
            if !target.status_keys.iter().any(|key| key == &status_key) {
                target.status_keys.push(status_key.clone());
                target.status_keys.sort();
            }
            validate_status_keys(&target.status_keys).map_err(|error| {
                ChampionError::InvalidSpellTarget {
                    spell_slug: spell_slug.to_string(),
                    target_type: format!("status_error:{error}"),
                }
            })?;
        }
        {
            let caster = battle_state
                .stack_mut(caster_stack_id)
                .map_err(champion_battle_error)?;
            caster.cast_round = battle_round;
            caster.acted_round = battle_round;
            caster.last_command_id = Some(command_id.to_string());
        }
        let champion = self.champion(caster_champion_id)?;
        Ok(ChampionMagicReceipt {
            command_id: command_id.to_string(),
            champion_id: caster_champion_id.to_string(),
            action: "cast_battle_spell".to_string(),
            skill_key: None,
            spell_slug: Some(spell_slug.to_string()),
            mana_after: champion.mana,
            movement_remaining_after: champion.movement_remaining,
            status_keys: vec![status_key],
        })
    }

    pub fn effective_mana(
        &self,
        champion_id: &str,
        current_turn: u32,
    ) -> Result<u16, ChampionError> {
        let champion = self.champion(champion_id)?;
        if champion.mana_turn == current_turn {
            Ok(champion.mana)
        } else {
            Ok(champion.mana_max)
        }
    }

    pub fn spend_mana(
        &mut self,
        champion_id: &str,
        cost: u16,
        current_turn: u32,
        command_id: &str,
    ) -> Result<u16, ChampionError> {
        let available = self.effective_mana(champion_id, current_turn)?;
        if cost > available {
            return Err(ChampionError::InsufficientMana {
                champion_id: champion_id.to_string(),
                cost,
                available,
            });
        }
        let champion = self.champion_mut(champion_id)?;
        champion.mana_turn = current_turn;
        champion.mana = available - cost;
        champion.last_command_id = Some(command_id.to_string());
        Ok(champion.mana)
    }

    pub fn learned_spell_slugs(&self, champion_id: &str) -> Vec<String> {
        let mut spells = self
            .champion_spells
            .iter()
            .filter(|known| known.champion_id == champion_id)
            .map(|known| known.spell_slug.clone())
            .collect::<Vec<_>>();
        spells.sort();
        spells
    }
}

fn learned_spell(
    state: &ChampionState,
    champion_id: &str,
    manifest: &ContentManifest,
    spell_slug: &str,
) -> Result<SpellContent, ChampionError> {
    let spell = manifest
        .spell(spell_slug)
        .ok_or_else(|| ChampionError::SpellNotFound {
            spell_slug: spell_slug.to_string(),
        })?;
    if !state
        .champion_spells
        .iter()
        .any(|known| known.champion_id == champion_id && known.spell_slug == spell_slug)
    {
        return Err(ChampionError::SpellNotFound {
            spell_slug: spell_slug.to_string(),
        });
    }
    Ok(spell.clone())
}

fn champion_battle_error(error: BattleError) -> ChampionError {
    ChampionError::InvalidSpellTarget {
        spell_slug: "battle_spell".to_string(),
        target_type: error.to_string(),
    }
}
