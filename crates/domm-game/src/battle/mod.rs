mod actions;
mod build;
mod initiative;
mod occupancy;
mod smoke;
#[cfg(test)]
mod tests;
mod types;
mod view;

pub use actions::{
    apply_damage_to_stack, apply_stack_attack, damage_preview, legal_actions_for_stack,
    reachable_tiles, v1_morale_luck_policy, validate_battle_stack_status_keys,
};
pub use build::build_first_playable_battle_state;
pub use initiative::{initiative_order, select_active_stack_id};
pub use occupancy::{
    adjacent_coords, occupant_at, repair_stack_position_from_occupancy, validate_battle_occupancy,
};
pub use smoke::run_first_playable_battle_smoke;
pub use types::{
    BATTLE_ACTION_DEADLINE_MS, BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_MAX_ROUNDS,
    BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER, BattleCoord, BattleDamageOutcome, BattleError,
    BattleEventView, BattleGridView, BattleInitiativeEntry, BattleMoraleLuckPolicy,
    BattleObstacleRecord, BattleObstacleView, BattleOccupancyRecord, BattleRecord, BattleSmokeView,
    BattleStackRecord, BattleStackView, BattleState, BattleView, DamagePreview, LegalBattleAction,
};
pub use view::battle_view_for_participant;
