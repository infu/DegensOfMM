use canic_cdk::{query, update};

use crate::dto::public::{ApiError, ChampionProgressionView, CommandResponse};

#[query]
fn preview_champion_progression(
    session_id: String,
    champion_id: String,
) -> Result<ChampionProgressionView, ApiError> {
    crate::services::champion_magic::preview_champion_progression(
        canic_cdk::api::msg_caller(),
        session_id,
        champion_id,
    )
}

#[update]
fn select_champion_level_up(
    session_id: String,
    champion_id: String,
    skill_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("select_champion_level_up", || {
        crate::services::champion_magic::select_champion_level_up(
            canic_cdk::api::msg_caller(),
            session_id,
            champion_id,
            skill_key,
            client_nonce,
        )
    })
}

#[update]
fn learn_champion_spell(
    session_id: String,
    champion_id: String,
    spell_slug: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("learn_champion_spell", || {
        crate::services::champion_magic::learn_champion_spell(
            canic_cdk::api::msg_caller(),
            session_id,
            champion_id,
            spell_slug,
            client_nonce,
        )
    })
}

#[update]
fn cast_adventure_spell(
    session_id: String,
    champion_id: String,
    spell_slug: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("cast_adventure_spell", || {
        crate::services::champion_magic::cast_adventure_spell(
            canic_cdk::api::msg_caller(),
            session_id,
            champion_id,
            spell_slug,
            client_nonce,
        )
    })
}
