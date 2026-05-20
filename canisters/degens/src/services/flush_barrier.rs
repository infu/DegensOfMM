//! Durable projection barriers for heap-owned runtime state.

#[cfg(not(feature = "benchmark"))]
use domm_game::ApiError;

#[cfg(not(feature = "benchmark"))]
pub(crate) const FLUSH_BARRIER_UPGRADE: &str = "Upgrade";
#[cfg(not(feature = "benchmark"))]
pub(crate) const FLUSH_BARRIER_STRONG_READ: &str = "StrongRead";
#[cfg(not(feature = "benchmark"))]
pub(crate) const FLUSH_BARRIER_TURN_ADVANCE: &str = "TurnAdvance";
#[cfg(not(feature = "benchmark"))]
pub(crate) const FLUSH_BARRIER_BATTLE_HANDOFF: &str = "BattleHandoff";
#[cfg(not(feature = "benchmark"))]
pub(crate) const FLUSH_BARRIER_RUNTIME_EVICTION: &str = "RuntimeEviction";

#[cfg(not(feature = "benchmark"))]
pub(crate) fn flush_barrier(reason: &str) -> Result<usize, ApiError> {
    if reason != FLUSH_BARRIER_UPGRADE
        && reason != FLUSH_BARRIER_STRONG_READ
        && reason != FLUSH_BARRIER_TURN_ADVANCE
        && reason != FLUSH_BARRIER_BATTLE_HANDOFF
        && reason != FLUSH_BARRIER_RUNTIME_EVICTION
    {
        return Err(ApiError::new(
            "unsupported_flush_barrier",
            "flush barrier reason is not supported yet",
            true,
        ));
    }

    let mut flushed = 0_usize;
    flushed = flushed
        .saturating_add(super::account_lobby_session::flush_runtime_lobby_state_for_upgrade()?);
    flushed = flushed
        .saturating_add(super::session_turn_runtime::flush_runtime_projections_for_upgrade()?);
    flushed = flushed.saturating_add(super::town_runtime::flush_all_projections_to_durable()?);
    flushed = flushed.saturating_add(super::battle_runtime::flush_runtime_archives_for_barrier()?);
    if reason == FLUSH_BARRIER_UPGRADE {
        super::battle_runtime::persist_snapshot_for_upgrade()
            .map_err(|message| ApiError::new("battle_runtime_snapshot_failed", message, true))?;
    }
    Ok(flushed)
}
