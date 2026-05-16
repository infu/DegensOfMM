use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::{
    Champion, GameCommand, GameParticipant, GameSession, Town, UnitDefinition, WorldObject,
};
use domm_game::{
    ApiError, ChampionHirePreview, CommandResponse, CommandResult, DwellingPoolView,
    DwellingRecruitPreview, ExpandedEconomyReceipt, MarketTradePreview, ResourceBalances,
    TavernOfferView, TavernOffersView,
};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{
    champions_artifacts, content, economy, economy_expansion, map_visibility_occupancy, sessions,
    towns,
};

use super::{
    command_response::{self, GameCommandAction},
    session_context::{self, public_error},
};

pub(crate) fn get_tavern_offers(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
) -> Result<TavernOffersView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return Err(public_error(
            "not_owner",
            "caller does not own this town",
            false,
        ));
    }
    let week_number = domm_game::week_for_turn(context.session.current_turn);
    let offers =
        economy_expansion::page_tavern_offers(context.session.id(), town.id(), week_number)?
            .items
            .into_iter()
            .map(tavern_offer_view)
            .collect();
    Ok(TavernOffersView {
        town_id: town.id().to_string(),
        week_number,
        offers,
    })
}

pub(crate) fn preview_hire_champion(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    offer_key: String,
) -> Result<ChampionHirePreview, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let offer = load_offer_for_town(&context, town.id(), &offer_key)?;
    let cost = ResourceBalances {
        gold: u64::from(offer.cost_gold),
        ..ResourceBalances::zero()
    };
    let disabled_reason = if town.owner_participant_id != Some(context.participant.id().key()) {
        Some("not_owner".to_string())
    } else if offer.status != "available" {
        Some("offer_not_available".to_string())
    } else if !can_afford(&context.participant, &cost) {
        Some("insufficient_resources".to_string())
    } else {
        None
    };
    Ok(ChampionHirePreview {
        allowed: disabled_reason.is_none(),
        disabled_reason,
        town_id: town.id().to_string(),
        offer_key,
        champion_class_slug: offer.champion_class_slug,
        candidate_name: offer.candidate_name,
        cost,
    })
}

pub(crate) fn hire_tavern_champion(
    caller: CandidPrincipal,
    session_id: String,
    town_id: String,
    offer_key: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let town = resolve_town(&context.session, &town_id)?;
    let payload_json = format!(
        r#"{{"town_id":"{}","offer_key":"{}"}}"#,
        command_response::escape_json(&town_id),
        command_response::escape_json(&offer_key)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "hire_tavern_champion",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    apply_hire_command(
        caller,
        &mut context,
        town,
        offer_key,
        command,
        &client_nonce,
    )
}

pub(crate) fn preview_market_trade(
    caller: CandidPrincipal,
    session_id: String,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
) -> Result<MarketTradePreview, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let mut quote = domm_game::market_trade_quote(&from_resource, &to_resource, amount_in)
        .map_err(|error| ApiError::new("invalid_market_trade", error.to_string(), false))?;
    if !has_resource(&context.participant, &from_resource, amount_in)? {
        quote.allowed = false;
        quote.disabled_reason = Some("insufficient_resources".to_string());
    }
    Ok(quote)
}

pub(crate) fn submit_market_trade(
    caller: CandidPrincipal,
    session_id: String,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let payload_json = format!(
        r#"{{"from_resource":"{}","to_resource":"{}","amount_in":{}}}"#,
        command_response::escape_json(&from_resource),
        command_response::escape_json(&to_resource),
        amount_in
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "submit_market_trade",
        &client_nonce,
        None,
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    apply_market_trade_command(
        caller,
        &mut context,
        from_resource,
        to_resource,
        amount_in,
        command,
        &client_nonce,
    )
}

pub(crate) fn get_dwelling_pool(
    caller: CandidPrincipal,
    session_id: String,
    object_id: String,
) -> Result<DwellingPoolView, ApiError> {
    let context = session_context::require_session_caller(caller, &session_id)?;
    let object = resolve_dwelling_object(&context.session, &object_id)?;
    let pool = economy_expansion::find_dwelling_pool_by_object(context.session.id(), object.id())?
        .ok_or_else(|| public_error("dwelling_pool_not_found", "dwelling pool not found", false))?;
    if pool.participant_id != Some(context.participant.id().key()) {
        return Err(public_error(
            "not_owner",
            "caller does not own this dwelling",
            false,
        ));
    }
    let mut view = dwelling_pool_view(pool);
    view.available = domm_game::dwelling_effective_available(
        view.available,
        view.last_growth_week,
        domm_game::week_for_turn(context.session.current_turn),
        view.growth_per_week,
    );
    Ok(view)
}

pub(crate) fn preview_dwelling_recruit(
    caller: CandidPrincipal,
    session_id: String,
    object_id: String,
    unit_slug: String,
    quantity: u32,
    champion_id: String,
) -> Result<DwellingRecruitPreview, ApiError> {
    let context = session_context::require_active_session_caller(caller, &session_id)?;
    let object = resolve_dwelling_object(&context.session, &object_id)?;
    let pool = economy_expansion::find_dwelling_pool_by_object(context.session.id(), object.id())?
        .ok_or_else(|| public_error("dwelling_pool_not_found", "dwelling pool not found", false))?;
    let champion = resolve_champion(&context.session, &champion_id)?;
    let unit =
        content::find_unit_by_ruleset_slug(Id::from_key(context.session.ruleset_id), &unit_slug)?
            .ok_or_else(|| public_error("unit_not_found", "unit definition was not found", false))?;
    let available = domm_game::dwelling_effective_available(
        pool.available,
        pool.last_growth_week,
        domm_game::week_for_turn(context.session.current_turn),
        pool.growth_per_week,
    );
    let total_cost = domm_game::dwelling_recruit_cost(unit.gold_cost, quantity);
    let disabled_reason = dwelling_recruit_disabled_reason(
        &context.participant,
        &pool,
        &champion,
        &unit_slug,
        quantity,
        available,
        &total_cost,
    );
    Ok(DwellingRecruitPreview {
        allowed: disabled_reason.is_none(),
        disabled_reason,
        object_id: object.id().to_string(),
        unit_slug,
        quantity,
        target_champion_id: champion.id().to_string(),
        total_cost,
        available,
    })
}

pub(crate) fn submit_dwelling_recruit(
    caller: CandidPrincipal,
    session_id: String,
    object_id: String,
    unit_slug: String,
    quantity: u32,
    champion_id: String,
    client_nonce: String,
) -> Result<CommandResponse, ApiError> {
    let mut context = session_context::require_active_session_caller(caller, &session_id)?;
    let object = resolve_dwelling_object(&context.session, &object_id)?;
    let champion = resolve_champion(&context.session, &champion_id)?;
    let payload_json = format!(
        r#"{{"object_id":"{}","unit_slug":"{}","quantity":{},"champion_id":"{}"}}"#,
        command_response::escape_json(&object_id),
        command_response::escape_json(&unit_slug),
        quantity,
        command_response::escape_json(&champion_id)
    );
    let command = match command_response::begin_participant_command(
        caller,
        &context,
        "submit_dwelling_recruit",
        &client_nonce,
        Some(champion.id()),
        payload_json,
    )? {
        GameCommandAction::Apply(command) => command,
        GameCommandAction::Return(response) => return Ok(response),
    };
    apply_dwelling_recruit_command(
        caller,
        &mut context,
        object,
        champion,
        unit_slug,
        quantity,
        command,
        &client_nonce,
    )
}

fn apply_hire_command(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    town: Town,
    offer_key: String,
    command: GameCommand,
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let mut offer = load_offer_for_town(context, town.id(), &offer_key)?;
    if town.owner_participant_id != Some(context.participant.id().key()) {
        return command_response::fail_command(
            caller,
            context,
            command,
            client_nonce,
            public_error("not_owner", "caller does not own this town", false),
        );
    }
    if offer.status != "available" && offer.hired_command_id != Some(command.id().key()) {
        return command_response::fail_command(
            caller,
            context,
            command,
            client_nonce,
            public_error(
                "offer_not_available",
                "tavern offer is not available",
                false,
            ),
        );
    }
    let cost = ResourceBalances {
        gold: u64::from(offer.cost_gold),
        ..ResourceBalances::zero()
    };
    if !can_afford(&context.participant, &cost) {
        return command_response::fail_command(
            caller,
            context,
            command,
            client_nonce,
            public_error("insufficient_resources", "not enough resources", false),
        );
    }

    spend_resources(
        context.session.id(),
        &mut context.participant,
        command.id(),
        &format!("hire:{offer_key}"),
        context.session.current_turn,
        &cost,
        "hire_champion",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    context.participant = sessions::update_participant(context.participant.clone())?;

    let mut hire = match economy_expansion::find_champion_hire_by_command(command.id())? {
        Some(row) => row,
        None => economy_expansion::create_champion_hire(
            context.session.id(),
            context.participant.id(),
            town.id(),
            offer.id(),
            command.id(),
            None,
            offer.cost_gold,
            context.session.current_turn,
        )?,
    };
    let champion = match hire.champion_id {
        Some(id) => champions_artifacts::load_champion(Id::<Champion>::from_key(id))?
            .ok_or_else(|| public_error("champion_not_found", "hired champion missing", true))?,
        None => {
            let mut champion = champions_artifacts::create_champion(
                context.session.id(),
                context.participant.id(),
                Id::from_key(offer.champion_class_id),
                offer.candidate_name.clone(),
                offer.champion_class_slug.clone(),
                "active".to_string(),
                town.x,
                town.y,
                town.chunk_x,
                town.chunk_y,
                1,
                0,
                1,
                1,
                1,
                1,
                10,
                10,
                context.session.current_turn,
                0,
                Vec::new(),
                240,
                240,
                context.session.current_turn,
                5,
                0,
            )?;
            champion.last_command_id = Some(command.id().key());
            champions_artifacts::update_champion(champion)?
        }
    };
    hire.champion_id = Some(champion.id().key());
    economy_expansion::update_champion_hire(hire)?;
    offer.status = "hired".to_string();
    offer.hired_champion_id = Some(champion.id().key());
    offer.hired_command_id = Some(command.id().key());
    economy_expansion::update_tavern_offer(offer)?;
    ensure_champion_occupancy(context.session.id(), command.id(), &champion)?;

    let receipt = receipt(
        command.id().to_string(),
        "hire_tavern_champion",
        Some(town.id().to_string()),
        None,
        Some(champion.id().to_string()),
        Some(offer_key.clone()),
        None,
        None,
        0,
        0,
        None,
        0,
        balances_from_participant(&context.participant),
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("hire:{offer_key}:{}", command.id()),
        "champion_hired".to_string(),
        Some("champion".to_string()),
        Some(champion.id().to_string()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("hire:{offer_key}"),
        "champion_hire".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        context,
        command,
        client_nonce,
        result_json,
        vec![event],
        vec![
            command_response::changed("champion", &champion.id().to_string(), "created"),
            command_response::changed("tavern_offer", &offer_key, "updated"),
            command_response::changed(
                "participant",
                &context.participant.id().to_string(),
                "updated",
            ),
        ],
        CommandResult::ExpandedEconomy(receipt),
    )
}

fn apply_market_trade_command(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    from_resource: String,
    to_resource: String,
    amount_in: u64,
    command: GameCommand,
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let quote = domm_game::market_trade_quote(&from_resource, &to_resource, amount_in)
        .map_err(|error| ApiError::new("invalid_market_trade", error.to_string(), false))?;
    if !has_resource(&context.participant, &from_resource, quote.amount_in)? {
        return command_response::fail_command(
            caller,
            context,
            command,
            client_nonce,
            public_error("insufficient_resources", "not enough resources", false),
        );
    }
    apply_resource_delta(
        context.session.id(),
        &mut context.participant,
        command.id(),
        format!("market:{}:{}:spend", quote.from_resource, quote.to_resource),
        context.session.current_turn,
        &quote.from_resource,
        -i64::try_from(quote.amount_in).unwrap_or(i64::MAX),
        "market_trade",
    )?;
    apply_resource_delta(
        context.session.id(),
        &mut context.participant,
        command.id(),
        format!("market:{}:{}:gain", quote.from_resource, quote.to_resource),
        context.session.current_turn,
        &quote.to_resource,
        i64::try_from(quote.amount_out).unwrap_or(i64::MAX),
        "market_trade",
    )?;
    context.participant.last_resource_command_id = Some(command.id().key());
    context.participant.last_action_turn = context.session.current_turn;
    context.participant = sessions::update_participant(context.participant.clone())?;
    if economy_expansion::find_market_trade_by_command(command.id())?.is_none() {
        economy_expansion::create_market_trade(
            context.session.id(),
            context.participant.id(),
            command.id(),
            context.session.current_turn,
            quote.from_resource.clone(),
            quote.to_resource.clone(),
            quote.amount_in,
            quote.amount_out,
            quote.rate_key.clone(),
        )?;
    }
    let receipt = receipt(
        command.id().to_string(),
        "submit_market_trade",
        None,
        None,
        None,
        None,
        Some(quote.from_resource.clone()),
        Some(quote.to_resource.clone()),
        quote.amount_in,
        quote.amount_out,
        None,
        0,
        balances_from_participant(&context.participant),
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("market:{}:{}", quote.rate_key, command.id()),
        "market_trade".to_string(),
        Some("participant".to_string()),
        Some(context.participant.id().to_string()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("market:{}:{}", quote.from_resource, quote.to_resource),
        "market_trade".to_string(),
        "participant".to_string(),
        context.participant.id().to_string(),
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        context,
        command,
        client_nonce,
        result_json,
        vec![event],
        vec![command_response::changed(
            "participant",
            &context.participant.id().to_string(),
            "updated",
        )],
        CommandResult::ExpandedEconomy(receipt),
    )
}

fn apply_dwelling_recruit_command(
    caller: CandidPrincipal,
    context: &mut session_context::SessionCallerContext,
    object: WorldObject,
    champion: Champion,
    unit_slug: String,
    quantity: u32,
    command: GameCommand,
    client_nonce: &str,
) -> Result<CommandResponse, ApiError> {
    let mut pool =
        economy_expansion::find_dwelling_pool_by_object(context.session.id(), object.id())?
            .ok_or_else(|| {
                public_error("dwelling_pool_not_found", "dwelling pool not found", false)
            })?;
    let unit =
        content::find_unit_by_ruleset_slug(Id::from_key(context.session.ruleset_id), &unit_slug)?
            .ok_or_else(|| public_error("unit_not_found", "unit definition was not found", false))?;
    let current_week = domm_game::week_for_turn(context.session.current_turn);
    pool.available = domm_game::dwelling_effective_available(
        pool.available,
        pool.last_growth_week,
        current_week,
        pool.growth_per_week,
    );
    pool.last_growth_week = current_week;
    let total_cost = domm_game::dwelling_recruit_cost(unit.gold_cost, quantity);
    if let Some(reason) = dwelling_recruit_disabled_reason(
        &context.participant,
        &pool,
        &champion,
        &unit_slug,
        quantity,
        pool.available,
        &total_cost,
    ) {
        return command_response::fail_command(
            caller,
            context,
            command,
            client_nonce,
            public_error(
                reason.clone(),
                format!("dwelling recruit disabled: {reason}"),
                false,
            ),
        );
    }
    spend_resources(
        context.session.id(),
        &mut context.participant,
        command.id(),
        &format!("dwelling:{}:{unit_slug}", object.id()),
        context.session.current_turn,
        &total_cost,
        "dwelling_recruit",
    )?;
    context.participant.last_action_turn = context.session.current_turn;
    context.participant = sessions::update_participant(context.participant.clone())?;
    pool.available = pool.available.saturating_sub(quantity);
    pool.last_command_id = Some(command.id().key());
    pool = economy_expansion::update_dwelling_pool(pool)?;
    recruit_to_champion(
        context.session.id(),
        champion.id(),
        unit.id(),
        unit.max_hp,
        quantity,
        command.id(),
    )?;
    if economy_expansion::find_dwelling_recruitment_by_command(command.id())?.is_none() {
        economy_expansion::create_dwelling_recruitment(
            context.session.id(),
            context.participant.id(),
            object.id(),
            pool.id(),
            champion.id(),
            unit.id(),
            unit_slug.clone(),
            command.id(),
            quantity,
            context.session.current_turn,
        )?;
    }
    let receipt = receipt(
        command.id().to_string(),
        "submit_dwelling_recruit",
        None,
        Some(object.id().to_string()),
        Some(champion.id().to_string()),
        None,
        None,
        None,
        0,
        0,
        Some(unit_slug.clone()),
        quantity,
        balances_from_participant(&context.participant),
    );
    let result_json = receipt_json(&receipt);
    let event = command_response::append_public_event(
        &mut context.session,
        command.id(),
        format!("dwelling_recruit:{}:{}", object.id(), command.id()),
        "dwelling_recruit".to_string(),
        Some("world_object".to_string()),
        Some(object.id().to_string()),
        result_json.clone(),
    )?;
    command_response::ensure_command_effect(
        context.session.id(),
        command.id(),
        format!("dwelling_recruit:{}:{unit_slug}", object.id()),
        "dwelling_recruit".to_string(),
        "champion".to_string(),
        champion.id().to_string(),
        result_json.clone(),
    )?;
    command_response::apply_command_with_result(
        caller,
        context,
        command,
        client_nonce,
        result_json,
        vec![event],
        vec![
            command_response::changed("dwelling_pool", &pool.id().to_string(), "updated"),
            command_response::changed("champion", &champion.id().to_string(), "updated"),
            command_response::changed(
                "participant",
                &context.participant.id().to_string(),
                "updated",
            ),
        ],
        CommandResult::ExpandedEconomy(receipt),
    )
}

fn load_offer_for_town(
    context: &session_context::SessionCallerContext,
    town_id: Id<Town>,
    offer_key: &str,
) -> Result<domm_degens_schema::schema::TavernOffer, ApiError> {
    let offer = economy_expansion::find_tavern_offer_by_key(offer_key)?
        .ok_or_else(|| public_error("offer_not_found", "tavern offer not found", false))?;
    if offer.session_id != context.session.id().key() || offer.town_id != town_id.key() {
        return Err(public_error(
            "offer_not_found",
            "tavern offer not found",
            false,
        ));
    }
    Ok(offer)
}

fn resolve_town(session: &GameSession, town_id: &str) -> Result<Town, ApiError> {
    if let Ok(id) = session_context::parse_id::<Town>(town_id, "town_id") {
        return towns::load_town(id)?
            .ok_or_else(|| public_error("not_found", "town not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.town_key == town_id)
        .ok_or_else(|| public_error("not_found", "town not found", false))?;
    towns::find_town_by_session_xy(session.id(), start.town_x, start.town_y)?
        .ok_or_else(|| public_error("not_found", "town not found", false))
}

fn resolve_dwelling_object(
    session: &GameSession,
    object_id: &str,
) -> Result<WorldObject, ApiError> {
    if let Ok(id) = session_context::parse_id::<WorldObject>(object_id, "object_id") {
        return map_visibility_occupancy::load_world_object(id)?
            .ok_or_else(|| public_error("not_found", "dwelling object not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let object = scenario
        .external_dwellings
        .iter()
        .find(|object| object.key == object_id)
        .ok_or_else(|| public_error("not_found", "dwelling object not found", false))?;
    map_visibility_occupancy::find_world_object_by_session_xy(session.id(), object.x, object.y)?
        .ok_or_else(|| public_error("not_found", "dwelling object not found", false))
}

fn resolve_champion(session: &GameSession, champion_id: &str) -> Result<Champion, ApiError> {
    if let Ok(id) = session_context::parse_id::<Champion>(champion_id, "champion_id") {
        return champions_artifacts::load_champion(id)?
            .ok_or_else(|| public_error("champion_not_found", "champion not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.champion_key == champion_id)
        .ok_or_else(|| public_error("champion_not_found", "champion not found", false))?;
    champions_artifacts::find_champion_by_session_xy(
        session.id(),
        start.champion_x,
        start.champion_y,
    )?
    .ok_or_else(|| public_error("champion_not_found", "champion not found", false))
}

fn tavern_offer_view(offer: domm_degens_schema::schema::TavernOffer) -> TavernOfferView {
    TavernOfferView {
        offer_key: offer.offer_key,
        town_id: Id::<Town>::from_key(offer.town_id).to_string(),
        week_number: offer.week_number,
        offer_slot: offer.offer_slot,
        champion_class_slug: offer.champion_class_slug,
        candidate_name: offer.candidate_name,
        cost_gold: offer.cost_gold,
        status: offer.status,
        hired_champion_id: offer
            .hired_champion_id
            .map(|id| Id::<Champion>::from_key(id).to_string()),
    }
}

fn dwelling_pool_view(pool: domm_degens_schema::schema::DwellingPool) -> DwellingPoolView {
    DwellingPoolView {
        object_id: Id::<WorldObject>::from_key(pool.object_id).to_string(),
        owner_participant_id: pool
            .participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
        unit_slug: pool.unit_slug,
        available: pool.available,
        last_growth_week: pool.last_growth_week,
        growth_per_week: pool.growth_per_week,
        direct_recruit: pool.direct_recruit,
    }
}

fn dwelling_recruit_disabled_reason(
    participant: &GameParticipant,
    pool: &domm_degens_schema::schema::DwellingPool,
    champion: &Champion,
    unit_slug: &str,
    quantity: u32,
    available: u32,
    total_cost: &ResourceBalances,
) -> Option<String> {
    if pool.participant_id != Some(participant.id().key()) {
        Some("not_owner".to_string())
    } else if !pool.direct_recruit {
        Some("direct_recruit_disabled".to_string())
    } else if champion.participant_id != participant.id().key() {
        Some("champion_not_owned".to_string())
    } else if pool.unit_slug != unit_slug {
        Some("unit_not_available".to_string())
    } else if quantity == 0 || quantity > domm_game::DWELLING_RECRUIT_MAX_QUANTITY {
        Some("invalid_quantity".to_string())
    } else if available < quantity {
        Some("dwelling_pool_empty".to_string())
    } else if !can_afford(participant, total_cost) {
        Some("insufficient_resources".to_string())
    } else {
        None
    }
}

fn recruit_to_champion(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    unit_id: Id<UnitDefinition>,
    front_hp: u16,
    quantity: u32,
    command_id: Id<GameCommand>,
) -> Result<(), ApiError> {
    for slot in 0..domm_game::MAX_ARMY_SLOTS {
        if let Some(mut stack) = champions_artifacts::find_champion_army_stack(champion_id, slot)? {
            if stack.unit_id == unit_id.key() {
                stack.quantity = stack.quantity.saturating_add(quantity);
                stack.last_command_id = Some(command_id.key());
                champions_artifacts::update_champion_army_stack(stack)?;
                return Ok(());
            }
            continue;
        }
        let mut stack = champions_artifacts::create_champion_army_stack(
            session_id,
            champion_id,
            unit_id,
            slot,
            quantity,
            front_hp,
            "active".to_string(),
        )?;
        stack.last_command_id = Some(command_id.key());
        champions_artifacts::update_champion_army_stack(stack)?;
        return Ok(());
    }
    Err(public_error(
        "recruit_target_full",
        "champion army is full",
        false,
    ))
}

fn ensure_champion_occupancy(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    champion: &Champion,
) -> Result<(), ApiError> {
    let occupant_id = champion.id().to_string();
    if map_visibility_occupancy::find_occupancy_by_occupant(
        session_id,
        "champion",
        &occupant_id,
        0,
    )?
    .is_some()
    {
        return Ok(());
    }
    let mut occupancy = map_visibility_occupancy::create_occupancy_cell(
        session_id,
        champion.x,
        champion.y,
        champion.chunk_x,
        champion.chunk_y,
        "champion".to_string(),
        "champion".to_string(),
        occupant_id,
        0,
        true,
    )?;
    occupancy.last_command_id = Some(command_id.key());
    map_visibility_occupancy::update_occupancy_cell(occupancy)?;
    Ok(())
}

fn spend_resources(
    session_id: Id<GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<GameCommand>,
    ledger_prefix: &str,
    turn_number: u32,
    cost: &ResourceBalances,
    reason: &str,
) -> Result<(), ApiError> {
    for (resource_key, amount) in [
        ("gold", i64::try_from(cost.gold).unwrap_or(i64::MAX)),
        ("wood", i64::from(cost.wood)),
        ("stone", i64::from(cost.stone)),
        ("iron", i64::from(cost.iron)),
        ("crystal", i64::from(cost.crystal)),
        ("ember", i64::from(cost.ember)),
        ("aether", i64::from(cost.aether)),
    ] {
        if amount == 0 {
            continue;
        }
        apply_resource_delta(
            session_id,
            participant,
            command_id,
            format!("{ledger_prefix}:{resource_key}"),
            turn_number,
            resource_key,
            -amount,
            reason,
        )?;
    }
    participant.last_resource_command_id = Some(command_id.key());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_resource_delta(
    session_id: Id<GameSession>,
    participant: &mut GameParticipant,
    command_id: Id<GameCommand>,
    ledger_key: String,
    turn_number: u32,
    resource_key: &str,
    delta: i64,
    reason: &str,
) -> Result<(), ApiError> {
    if economy::find_resource_ledger_entry(command_id, &ledger_key)?.is_some() {
        return Ok(());
    }
    let balance_after = match resource_key {
        "gold" => {
            participant.gold = apply_u64_delta(participant.gold, delta)?;
            participant.gold
        }
        "wood" => {
            participant.wood = apply_u32_delta(participant.wood, delta)?;
            u64::from(participant.wood)
        }
        "stone" => {
            participant.stone = apply_u32_delta(participant.stone, delta)?;
            u64::from(participant.stone)
        }
        "iron" => {
            participant.iron = apply_u32_delta(participant.iron, delta)?;
            u64::from(participant.iron)
        }
        "crystal" => {
            participant.crystal = apply_u32_delta(participant.crystal, delta)?;
            u64::from(participant.crystal)
        }
        "ember" => {
            participant.ember = apply_u32_delta(participant.ember, delta)?;
            u64::from(participant.ember)
        }
        "aether" => {
            participant.aether = apply_u32_delta(participant.aether, delta)?;
            u64::from(participant.aether)
        }
        _ => {
            return Err(public_error(
                "unknown_resource",
                "unknown resource key",
                false,
            ));
        }
    };
    economy::create_resource_ledger_entry(
        session_id,
        participant.id(),
        command_id,
        ledger_key,
        turn_number,
        resource_key.to_string(),
        delta,
        balance_after,
        reason.to_string(),
        "applied".to_string(),
    )?;
    Ok(())
}

fn apply_u64_delta(value: u64, delta: i64) -> Result<u64, ApiError> {
    if delta.is_negative() {
        value
            .checked_sub(delta.unsigned_abs())
            .ok_or_else(|| public_error("insufficient_resources", "not enough resources", false))
    } else {
        Ok(value.saturating_add(delta as u64))
    }
}

fn apply_u32_delta(value: u32, delta: i64) -> Result<u32, ApiError> {
    let value = apply_u64_delta(u64::from(value), delta)?;
    u32::try_from(value)
        .map_err(|_| public_error("resource_cap_exceeded", "resource cap exceeded", false))
}

fn can_afford(participant: &GameParticipant, cost: &ResourceBalances) -> bool {
    participant.gold >= cost.gold
        && participant.wood >= cost.wood
        && participant.stone >= cost.stone
        && participant.iron >= cost.iron
        && participant.crystal >= cost.crystal
        && participant.ember >= cost.ember
        && participant.aether >= cost.aether
}

fn has_resource(
    participant: &GameParticipant,
    resource_key: &str,
    amount: u64,
) -> Result<bool, ApiError> {
    let available = match resource_key {
        "gold" => participant.gold,
        "wood" => u64::from(participant.wood),
        "stone" => u64::from(participant.stone),
        "iron" => u64::from(participant.iron),
        "crystal" => u64::from(participant.crystal),
        "ember" => u64::from(participant.ember),
        "aether" => u64::from(participant.aether),
        _ => {
            return Err(public_error(
                "unknown_resource",
                "unknown resource key",
                false,
            ));
        }
    };
    Ok(available >= amount)
}

fn balances_from_participant(participant: &GameParticipant) -> ResourceBalances {
    ResourceBalances {
        gold: participant.gold,
        wood: participant.wood,
        stone: participant.stone,
        iron: participant.iron,
        crystal: participant.crystal,
        ember: participant.ember,
        aether: participant.aether,
    }
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    command_id: String,
    action: &str,
    town_id: Option<String>,
    object_id: Option<String>,
    champion_id: Option<String>,
    offer_key: Option<String>,
    from_resource: Option<String>,
    to_resource: Option<String>,
    amount_in: u64,
    amount_out: u64,
    unit_slug: Option<String>,
    quantity: u32,
    resources_after: ResourceBalances,
) -> ExpandedEconomyReceipt {
    ExpandedEconomyReceipt {
        command_id,
        action: action.to_string(),
        town_id,
        object_id,
        champion_id,
        offer_key,
        from_resource,
        to_resource,
        amount_in,
        amount_out,
        unit_slug,
        quantity,
        resources_after,
    }
}

fn receipt_json(receipt: &ExpandedEconomyReceipt) -> String {
    format!(
        "{{\"action\":\"{}\",\"town_id\":{},\"object_id\":{},\"champion_id\":{},\"offer_key\":{},\"from_resource\":{},\"to_resource\":{},\"amount_in\":{},\"amount_out\":{},\"unit_slug\":{},\"quantity\":{},\"gold_after\":{},\"wood_after\":{},\"stone_after\":{},\"iron_after\":{},\"crystal_after\":{},\"ember_after\":{},\"aether_after\":{}}}",
        command_response::escape_json(&receipt.action),
        option_json(receipt.town_id.as_deref()),
        option_json(receipt.object_id.as_deref()),
        option_json(receipt.champion_id.as_deref()),
        option_json(receipt.offer_key.as_deref()),
        option_json(receipt.from_resource.as_deref()),
        option_json(receipt.to_resource.as_deref()),
        receipt.amount_in,
        receipt.amount_out,
        option_json(receipt.unit_slug.as_deref()),
        receipt.quantity,
        receipt.resources_after.gold,
        receipt.resources_after.wood,
        receipt.resources_after.stone,
        receipt.resources_after.iron,
        receipt.resources_after.crystal,
        receipt.resources_after.ember,
        receipt.resources_after.aether
    )
}

fn option_json(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |value| format!("\"{}\"", command_response::escape_json(value)),
    )
}
