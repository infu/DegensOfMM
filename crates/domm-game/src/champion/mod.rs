mod actions;
mod build;
#[cfg(test)]
mod tests;
mod types;
mod view;

pub use build::build_first_playable_champion_state;
pub use types::{
    ArtifactCaptureResult, ArtifactEquipmentRecord, ArtifactInstanceRecord, ArtifactView,
    CHAMPION_LEVEL_CAP, ChampionArmyStackRecord, ChampionError, ChampionProgressionResult,
    ChampionRecord, ChampionState, ChampionView, ChampionViewResult,
};
