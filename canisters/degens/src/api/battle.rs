use canic_cdk::{query, update};

use crate::{
    dto::public::{ApiError, BattleActionInput, BattleView, CommandResponse},
    services::clock,
};

#[query]
fn get_battle_state(session_id: String, battle_id: String) -> Result<BattleView, ApiError> {
    crate::services::battle::get_battle_state(
        canic_cdk::api::msg_caller(),
        session_id,
        battle_id,
        clock::now_ms(),
    )
}

#[update]
fn sync_battle(
    session_id: String,
    battle_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("sync_battle", || {
        crate::services::battle::sync_battle(
            canic_cdk::api::msg_caller(),
            session_id,
            battle_id,
            clock::now_ms(),
            client_nonce,
        )
    })
}

#[update]
fn end_battle_turn(
    session_id: String,
    battle_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("end_battle_turn", || {
        crate::services::battle::end_battle_turn(
            canic_cdk::api::msg_caller(),
            session_id,
            battle_id,
            client_nonce,
        )
    })
}

#[update]
fn submit_battle_action(
    session_id: String,
    input: BattleActionInput,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_battle_action", || {
        crate::services::battle::submit_battle_action(
            canic_cdk::api::msg_caller(),
            session_id,
            input,
            client_nonce,
            clock::now_ms(),
        )
    })
}
