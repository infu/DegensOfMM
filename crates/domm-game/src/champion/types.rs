use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CHAMPION_STACK_CAP: u32 = 99_999;
pub const CHAMPION_LEVEL_CAP: u16 = 10;
pub const CHAMPION_SKILL_CAP: usize = 8;
pub const CHAMPION_SPELLBOOK_CAP: usize = 8;
pub const CHAMPION_SKILL_OPTIONS_PER_LEVEL: usize = 3;
pub const CHAMPION_BATTLE_CASTS_PER_ROUND: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionRecord {
    pub champion_id: String,
    pub session_id: String,
    pub participant_id: String,
    pub class_def_id: String,
    pub name: String,
    pub class_key: String,
    pub status: String,
    pub x: u16,
    pub y: u16,
    pub level: u16,
    pub experience: u64,
    pub might: i16,
    pub guard: i16,
    pub wisdom: i16,
    pub command: i16,
    pub mana: u16,
    pub mana_max: u16,
    pub mana_turn: u32,
    pub skill_points: u16,
    pub skill_keys: Vec<String>,
    pub movement_max: u16,
    pub movement_remaining: u16,
    pub movement_turn: u32,
    pub vision_radius: u8,
    pub defeated_turn: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionSpellRecord {
    pub champion_spell_id: String,
    pub session_id: String,
    pub champion_id: String,
    pub spell_slug: String,
    pub learned_turn: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionArmyStackRecord {
    pub stack_id: String,
    pub session_id: String,
    pub champion_id: String,
    pub unit_slug: String,
    pub slot_index: u8,
    pub quantity: u32,
    pub front_hp: u16,
    pub status: String,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArtifactInstanceRecord {
    pub artifact_id: String,
    pub session_id: String,
    pub artifact_def_id: String,
    pub owner_champion_id: Option<String>,
    pub slot: Option<String>,
    pub x: u16,
    pub y: u16,
    pub state: String,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArtifactEquipmentRecord {
    pub equipment_id: String,
    pub session_id: String,
    pub champion_id: String,
    pub artifact_id: String,
    pub slot: String,
    pub equipped_turn: u32,
    pub last_command_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArtifactView {
    pub artifact_id: String,
    pub artifact_def_id: String,
    pub slot: String,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionView {
    pub champion_id: String,
    pub owner_participant_id: String,
    pub name: Option<String>,
    pub class_def_id: String,
    pub class_key: String,
    pub status: String,
    pub x: u16,
    pub y: u16,
    pub effective_movement: u16,
    pub movement_max: u16,
    pub mana: u16,
    pub mana_max: u16,
    pub skill_points: u16,
    pub skill_keys: Vec<String>,
    pub spell_slugs: Vec<String>,
    pub vision_radius: u8,
    pub strength_label: String,
    pub army_stacks: Vec<ChampionArmyStackRecord>,
    pub artifacts: Vec<ArtifactView>,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionSkillChoiceView {
    pub skill_key: String,
    pub name: String,
    pub description: String,
    pub rank: u8,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionProgressionView {
    pub champion_id: String,
    pub level: u16,
    pub experience: u64,
    pub skill_points: u16,
    pub skill_keys: Vec<String>,
    pub mana: u16,
    pub mana_max: u16,
    pub mana_turn: u32,
    pub learned_spell_slugs: Vec<String>,
    pub level_up_choices: Vec<ChampionSkillChoiceView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionMagicReceipt {
    pub command_id: String,
    pub champion_id: String,
    pub action: String,
    pub skill_key: Option<String>,
    pub spell_slug: Option<String>,
    pub mana_after: u16,
    pub movement_remaining_after: u16,
    pub status_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum ChampionViewResult {
    Visible(ChampionView),
    Hidden {
        champion_id: String,
        visibility: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArtifactCaptureResult {
    pub victor_champion_id: String,
    pub defeated_champion_id: String,
    pub captured_artifact_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionProgressionResult {
    pub champion_id: String,
    pub experience_before: u64,
    pub experience_after: u64,
    pub level_before: u16,
    pub level_after: u16,
    pub skill_points_after: u16,
    pub skill_choice_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionState {
    pub session_id: String,
    pub champions: Vec<ChampionRecord>,
    pub champion_spells: Vec<ChampionSpellRecord>,
    pub army_stacks: Vec<ChampionArmyStackRecord>,
    pub artifact_instances: Vec<ArtifactInstanceRecord>,
    pub artifact_equipment: Vec<ArtifactEquipmentRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ChampionError {
    #[error("champion not found: {champion_id}")]
    ChampionNotFound { champion_id: String },
    #[error("artifact not found: {artifact_id}")]
    ArtifactNotFound { artifact_id: String },
    #[error("stack cap exceeded for {stack_id}: {attempted}")]
    StackCapExceeded { stack_id: String, attempted: u32 },
    #[error("equipment slot {slot} is already occupied for {champion_id}")]
    EquipmentSlotOccupied { champion_id: String, slot: String },
    #[error("artifact is already equipped: {artifact_id}")]
    ArtifactAlreadyEquipped { artifact_id: String },
    #[error("status is not valid for v1: {status}")]
    InvalidStatus { status: String },
    #[error("movement cost {cost} exceeds available movement {available}")]
    InsufficientMovement { cost: u16, available: u16 },
    #[error("champion {champion_id} has no pending skill point")]
    NoPendingSkillPoint { champion_id: String },
    #[error("skill choice is not legal for champion {champion_id}: {skill_key}")]
    InvalidSkillChoice {
        champion_id: String,
        skill_key: String,
    },
    #[error("skill already selected for champion {champion_id}: {skill_key}")]
    SkillAlreadySelected {
        champion_id: String,
        skill_key: String,
    },
    #[error("skill cap exceeded for champion {champion_id}: {attempted}")]
    SkillCapExceeded {
        champion_id: String,
        attempted: usize,
    },
    #[error("spell not found: {spell_slug}")]
    SpellNotFound { spell_slug: String },
    #[error("spell already learned by champion {champion_id}: {spell_slug}")]
    SpellAlreadyLearned {
        champion_id: String,
        spell_slug: String,
    },
    #[error("spellbook cap exceeded for champion {champion_id}: {attempted}")]
    SpellbookCapExceeded {
        champion_id: String,
        attempted: usize,
    },
    #[error("spell prerequisite is missing for champion {champion_id}: {required_skill_key}")]
    SpellPrerequisiteMissing {
        champion_id: String,
        required_skill_key: String,
    },
    #[error("champion {champion_id} has insufficient mana: cost {cost}, available {available}")]
    InsufficientMana {
        champion_id: String,
        cost: u16,
        available: u16,
    },
    #[error("spell {spell_slug} cannot target {target_type}")]
    InvalidSpellTarget {
        spell_slug: String,
        target_type: String,
    },
}
