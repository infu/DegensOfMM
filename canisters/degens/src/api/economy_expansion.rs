use canic_cdk::{query, update};

use crate::dto::public::{
    ApiError, ChampionHirePreview, CommandResponse, DwellingPoolView, DwellingRecruitPreview,
    MarketTradePreview, TavernOffersView,
};

#[query]
fn get_tavern_offers(session_id: String, town_id: String) -> Result<TavernOffersView, ApiError> {
    crate::services::economy_expansion::get_tavern_offers(
        canic_cdk::api::msg_caller(),
        session_id,
        town_id,
    )
}

#[query]
fn preview_hire_champion(
    session_id: String,
    town_id: String,
    offer_key: String,
) -> Result<ChampionHirePreview, ApiError> {
    crate::services::economy_expansion::preview_hire_champion(
        canic_cdk::api::msg_caller(),
        session_id,
        town_id,
        offer_key,
    )
}

#[update]
fn hire_tavern_champion(
    session_id: String,
    town_id: String,
    offer_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("hire_tavern_champion", || {
        crate::services::economy_expansion::hire_tavern_champion(
            canic_cdk::api::msg_caller(),
            session_id,
            town_id,
            offer_key,
            client_nonce,
        )
    })
}

#[query]
fn preview_market_trade(
    session_id: String,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
) -> Result<MarketTradePreview, ApiError> {
    crate::services::economy_expansion::preview_market_trade(
        canic_cdk::api::msg_caller(),
        session_id,
        from_resource,
        to_resource,
        amount_in,
    )
}

#[update]
fn submit_market_trade(
    session_id: String,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_market_trade", || {
        crate::services::economy_expansion::submit_market_trade(
            canic_cdk::api::msg_caller(),
            session_id,
            from_resource,
            to_resource,
            amount_in,
            client_nonce,
        )
    })
}

#[query]
fn get_dwelling_pool(session_id: String, object_id: String) -> Result<DwellingPoolView, ApiError> {
    crate::services::economy_expansion::get_dwelling_pool(
        canic_cdk::api::msg_caller(),
        session_id,
        object_id,
    )
}

#[query]
fn preview_dwelling_recruit(
    session_id: String,
    object_id: String,
    unit_slug: String,
    quantity: u32,
    champion_id: String,
) -> Result<DwellingRecruitPreview, ApiError> {
    crate::services::economy_expansion::preview_dwelling_recruit(
        canic_cdk::api::msg_caller(),
        session_id,
        object_id,
        unit_slug,
        quantity,
        champion_id,
    )
}

#[update]
fn submit_dwelling_recruit(
    session_id: String,
    object_id: String,
    unit_slug: String,
    quantity: u32,
    champion_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    crate::metrics::benchmark_update("submit_dwelling_recruit", || {
        crate::services::economy_expansion::submit_dwelling_recruit(
            canic_cdk::api::msg_caller(),
            session_id,
            object_id,
            unit_slug,
            quantity,
            champion_id,
            client_nonce,
        )
    })
}
