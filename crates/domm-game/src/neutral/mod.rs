mod actions;
mod build;
mod smoke;
#[cfg(test)]
mod tests;
mod types;
mod view;

pub use actions::{
    apply_neutral_encounters_from_movement, defeat_neutral_army, materialize_neutral_growth,
    start_guarded_object_encounter, start_neutral_encounter,
};
pub use build::build_first_playable_neutral_state;
pub use smoke::run_first_playable_neutral_smoke;
pub use types::{
    NeutralArmyEncounterRecord, NeutralArmyRecord, NeutralArmyStackRecord, NeutralArmyView,
    NeutralArmyViewResult, NeutralBehaviorPolicy, NeutralError, NeutralGrowthOutcome,
    NeutralSmokeView, NeutralState, strength_label_for_quantity,
};
