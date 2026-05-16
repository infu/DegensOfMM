mod actions;
mod build;
mod recruitment;
#[cfg(test)]
mod tests;
mod types;

pub use actions::run_first_playable_town_smoke;
pub use build::build_first_playable_town_state;
pub use types::{
    ArmyStackRecord, BuildPreview, ChampionTownRecord, MAX_ARMY_SLOTS, RecruitPreview,
    RecruitTarget, TownBuildingRecord, TownError, TownRecord, TownRecruitPoolRecord, TownSmokeView,
    TownState,
};
