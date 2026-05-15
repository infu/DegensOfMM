mod actions;
mod build;
mod smoke;
#[cfg(test)]
mod tests;
mod types;

pub use actions::{
    apply_battle_aftermath, check_and_finalize_victory, finalize_stalemate,
    require_retreat_or_surrender_enabled, resolve_neutral_battle_for_fixture,
    retreat_surrender_policy,
};
pub use build::build_first_playable_aftermath_state;
pub use smoke::{
    run_first_playable_aftermath_smoke, seed_resolved_champion_defeat_battle,
    seed_resolved_town_capture_battle,
};
pub use types::{
    AftermathError, AftermathEventRecord, AftermathSmokeView, AftermathState,
    BattleAftermathReport, MatchSessionRecord, PlayerMatchSummaryRecord, RetreatSurrenderPolicy,
    VictoryCheck, VictoryScore,
};
