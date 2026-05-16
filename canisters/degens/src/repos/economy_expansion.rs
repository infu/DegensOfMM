//! Repository boundary for checkpoint 23 taverns, market trades, and external dwellings.

use domm_degens_schema::schema::{
    Champion, ChampionClassDefinition, ChampionHire, DwellingPool, DwellingRecruitment,
    GameCommand, GameParticipant, GameSession, MarketTrade, TavernOffer, Town, UnitDefinition,
    WorldObject,
};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const TAVERN_OFFERS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy_expansion.tavern_offers_by_town_week",
    entity: "TavernOffer",
    indexed_fields: &["session_id", "town_id", "week_number"],
    bounded_limit: Some(domm_game::TAVERN_OFFERS_PER_WEEK as u32),
};

pub(crate) const TAVERN_OFFER_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy_expansion.tavern_offer_by_key",
    entity: "TavernOffer",
    indexed_fields: &["offer_key"],
    bounded_limit: Some(1),
};

pub(crate) const MARKET_TRADE_COMMAND_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy_expansion.market_trade_by_command",
    entity: "MarketTrade",
    indexed_fields: &["command_id"],
    bounded_limit: Some(1),
};

pub(crate) const DWELLING_POOL_OBJECT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy_expansion.dwelling_pool_by_object",
    entity: "DwellingPool",
    indexed_fields: &["session_id", "object_id"],
    bounded_limit: Some(1),
};

pub(crate) const DWELLING_RECRUIT_COMMAND_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy_expansion.dwelling_recruit_by_command",
    entity: "DwellingRecruitment",
    indexed_fields: &["command_id"],
    bounded_limit: Some(1),
};

pub(crate) fn page_tavern_offers(
    session_id: Id<GameSession>,
    town_id: Id<Town>,
    week_number: u32,
) -> RepoResult<RepositoryPage<TavernOffer>> {
    foundation::execute_page(
        TAVERN_OFFERS_LOOKUP.name,
        crate::db()
            .load::<TavernOffer>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("town_id").eq(town_id.key()))
            .filter(FieldRef::new("week_number").eq(week_number))
            .order_asc("offer_slot")
            .order_asc("id"),
        domm_game::TAVERN_OFFERS_PER_WEEK as u32,
        None,
    )
}

pub(crate) fn find_tavern_offer_by_key(offer_key: &str) -> RepoResult<Option<TavernOffer>> {
    foundation::storage_result(
        TAVERN_OFFER_KEY_LOOKUP.name,
        crate::db()
            .load::<TavernOffer>()
            .filter(FieldRef::new("offer_key").eq(offer_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_tavern_offer(
    session_id: Id<GameSession>,
    town_id: Id<Town>,
    participant_id: Id<GameParticipant>,
    week_number: u32,
    offer_slot: u8,
    offer_key: String,
    champion_class_id: Id<ChampionClassDefinition>,
    champion_class_slug: String,
    candidate_name: String,
    cost_gold: u32,
) -> RepoResult<TavernOffer> {
    let input: Create<TavernOffer> = Create::<TavernOffer> {
        session_id: Some(session_id.key()),
        town_id: Some(town_id.key()),
        participant_id: Some(participant_id.key()),
        week_number: Some(week_number),
        offer_slot: Some(offer_slot),
        offer_key: Some(offer_key),
        champion_class_id: Some(champion_class_id.key()),
        champion_class_slug: Some(champion_class_slug),
        candidate_name: Some(candidate_name),
        cost_gold: Some(cost_gold),
        status: Some("available".to_string()),
        hired_champion_id: Some(None),
        hired_command_id: Some(None),
    };

    foundation::create("economy_expansion.create_tavern_offer", input)
}

pub(crate) fn update_tavern_offer(offer: TavernOffer) -> RepoResult<TavernOffer> {
    foundation::update("economy_expansion.update_tavern_offer", offer)
}

pub(crate) fn find_champion_hire_by_command(
    command_id: Id<GameCommand>,
) -> RepoResult<Option<ChampionHire>> {
    foundation::storage_result(
        "economy_expansion.champion_hire_by_command",
        crate::db()
            .load::<ChampionHire>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn update_champion_hire(hire: ChampionHire) -> RepoResult<ChampionHire> {
    foundation::update("economy_expansion.update_champion_hire", hire)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_champion_hire(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    town_id: Id<Town>,
    offer_id: Id<TavernOffer>,
    command_id: Id<GameCommand>,
    champion_id: Option<Id<Champion>>,
    cost_gold: u32,
    hired_turn: u32,
) -> RepoResult<ChampionHire> {
    let input: Create<ChampionHire> = Create::<ChampionHire> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        town_id: Some(town_id.key()),
        offer_id: Some(offer_id.key()),
        command_id: Some(command_id.key()),
        champion_id: Some(champion_id.map(|id| id.key())),
        cost_gold: Some(cost_gold),
        hired_turn: Some(hired_turn),
        status: Some("applied".to_string()),
    };

    foundation::create("economy_expansion.create_champion_hire", input)
}

pub(crate) fn find_market_trade_by_command(
    command_id: Id<GameCommand>,
) -> RepoResult<Option<MarketTrade>> {
    foundation::storage_result(
        MARKET_TRADE_COMMAND_LOOKUP.name,
        crate::db()
            .load::<MarketTrade>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_market_trade(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    command_id: Id<GameCommand>,
    turn_number: u32,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
    amount_out: u64,
    rate_key: String,
) -> RepoResult<MarketTrade> {
    let input: Create<MarketTrade> = Create::<MarketTrade> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        command_id: Some(command_id.key()),
        turn_number: Some(turn_number),
        from_resource: Some(from_resource),
        to_resource: Some(to_resource),
        amount_in: Some(amount_in),
        amount_out: Some(amount_out),
        rate_key: Some(rate_key),
        status: Some("applied".to_string()),
    };

    foundation::create("economy_expansion.create_market_trade", input)
}

pub(crate) fn find_dwelling_pool_by_object(
    session_id: Id<GameSession>,
    object_id: Id<WorldObject>,
) -> RepoResult<Option<DwellingPool>> {
    foundation::storage_result(
        DWELLING_POOL_OBJECT_LOOKUP.name,
        crate::db()
            .load::<DwellingPool>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("object_id").eq(object_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_dwelling_pool(
    session_id: Id<GameSession>,
    object_id: Id<WorldObject>,
    participant_id: Option<Id<GameParticipant>>,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    available: u32,
    last_growth_week: u32,
    growth_per_week: u16,
    direct_recruit: bool,
) -> RepoResult<DwellingPool> {
    let input: Create<DwellingPool> = Create::<DwellingPool> {
        session_id: Some(session_id.key()),
        object_id: Some(object_id.key()),
        participant_id: Some(participant_id.map(|id| id.key())),
        unit_id: Some(unit_id.key()),
        unit_slug: Some(unit_slug),
        available: Some(available),
        last_growth_week: Some(last_growth_week),
        growth_per_week: Some(growth_per_week),
        direct_recruit: Some(direct_recruit),
        last_command_id: Some(None),
    };

    foundation::create("economy_expansion.create_dwelling_pool", input)
}

pub(crate) fn update_dwelling_pool(pool: DwellingPool) -> RepoResult<DwellingPool> {
    foundation::update("economy_expansion.update_dwelling_pool", pool)
}

pub(crate) fn find_dwelling_recruitment_by_command(
    command_id: Id<GameCommand>,
) -> RepoResult<Option<DwellingRecruitment>> {
    foundation::storage_result(
        DWELLING_RECRUIT_COMMAND_LOOKUP.name,
        crate::db()
            .load::<DwellingRecruitment>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_dwelling_recruitment(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    object_id: Id<WorldObject>,
    pool_id: Id<DwellingPool>,
    champion_id: Id<Champion>,
    unit_id: Id<UnitDefinition>,
    unit_slug: String,
    command_id: Id<GameCommand>,
    quantity: u32,
    recruited_turn: u32,
) -> RepoResult<DwellingRecruitment> {
    let input: Create<DwellingRecruitment> = Create::<DwellingRecruitment> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        object_id: Some(object_id.key()),
        pool_id: Some(pool_id.key()),
        champion_id: Some(champion_id.key()),
        unit_id: Some(unit_id.key()),
        unit_slug: Some(unit_slug),
        command_id: Some(command_id.key()),
        quantity: Some(quantity),
        recruited_turn: Some(recruited_turn),
        status: Some("applied".to_string()),
    };

    foundation::create("economy_expansion.create_dwelling_recruitment", input)
}
