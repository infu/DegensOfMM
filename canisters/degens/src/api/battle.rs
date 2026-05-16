use canic_cdk::{query, update};

use crate::dto::public::{ApiError, BattleActionInput, BattleView, CommandResponse};

#[query]
fn get_battle_state(
    _session_id: String,
    _battle_id: String,
    _now_ms: u64,
) -> Result<BattleView, ApiError> {
    crate::services::battle::unavailable("get_battle_state")
}

#[update]
fn sync_battle(
    _session_id: String,
    _battle_id: String,
    _now_ms: u64,
    _client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::services::battle::unavailable("sync_battle")
}

#[update]
fn submit_battle_action(
    _session_id: String,
    _input: BattleActionInput,
    _client_nonce: String,
    _now_ms: u64,
) -> Result<CommandResponse, ApiError> {
    crate::services::battle::unavailable("submit_battle_action")
}
