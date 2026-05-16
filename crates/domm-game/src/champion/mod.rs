mod actions;
mod build;
mod magic;
#[cfg(test)]
mod tests;
mod types;
mod view;

pub use build::build_first_playable_champion_state;
pub use types::{
    ArtifactCaptureResult, ArtifactEquipmentRecord, ArtifactInstanceRecord, ArtifactView,
    CHAMPION_BATTLE_CASTS_PER_ROUND, CHAMPION_LEVEL_CAP, CHAMPION_SKILL_CAP,
    CHAMPION_SKILL_OPTIONS_PER_LEVEL, CHAMPION_SPELLBOOK_CAP, ChampionArmyStackRecord,
    ChampionError, ChampionMagicReceipt, ChampionProgressionResult, ChampionProgressionView,
    ChampionRecord, ChampionSkillChoiceView, ChampionSpellRecord, ChampionState, ChampionView,
    ChampionViewResult,
};
