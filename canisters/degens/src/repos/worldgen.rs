//! Repository boundary for checkpoint 25 skirmish, procedural, naval, and siege rows.

use domm_degens_schema::schema::{
    GameCommand, GameSession, NavalRouteState, ProceduralMapState, SiegeRuleState,
    SkirmishSettingsState,
};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const SKIRMISH_SETTINGS_BY_SESSION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.skirmish_settings_by_session",
    entity: "SkirmishSettingsState",
    indexed_fields: &["session_id"],
    bounded_limit: Some(1),
};

pub(crate) const PROCEDURAL_MAP_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.procedural_map_by_key",
    entity: "ProceduralMapState",
    indexed_fields: &["session_id", "generation_key"],
    bounded_limit: Some(1),
};

pub(crate) const PROCEDURAL_MAPS_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.procedural_maps_by_status",
    entity: "ProceduralMapState",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_GENERATED_CHUNKS_PER_UPDATE),
};

pub(crate) const NAVAL_ROUTE_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.naval_route_by_key",
    entity: "NavalRouteState",
    indexed_fields: &["session_id", "route_key"],
    bounded_limit: Some(1),
};

pub(crate) const NAVAL_ROUTES_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.naval_routes_by_status",
    entity: "NavalRouteState",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_NAVAL_ROUTE_ROWS_PER_SESSION),
};

pub(crate) const SIEGE_RULE_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.siege_rule_by_key",
    entity: "SiegeRuleState",
    indexed_fields: &["session_id", "rule_key"],
    bounded_limit: Some(1),
};

pub(crate) const SIEGE_RULES_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "worldgen.siege_rules_by_status",
    entity: "SiegeRuleState",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_SIEGE_RULE_ROWS_PER_SESSION),
};

pub(crate) fn find_skirmish_settings(
    session_id: Id<GameSession>,
) -> RepoResult<Option<SkirmishSettingsState>> {
    foundation::storage_result(
        SKIRMISH_SETTINGS_BY_SESSION_LOOKUP.name,
        crate::db()
            .load::<SkirmishSettingsState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_skirmish_settings(
    session_id: Id<GameSession>,
    record: &domm_game::SkirmishSettingsRecord,
) -> RepoResult<SkirmishSettingsState> {
    let input: Create<SkirmishSettingsState> = Create::<SkirmishSettingsState> {
        session_id: Some(session_id.key()),
        profile_key: Some(record.profile_key.clone()),
        status: Some(record.status.clone()),
        map_seed: Some(record.map_seed),
        map_width: Some(record.map_width),
        map_height: Some(record.map_height),
        chunk_size: Some(record.chunk_size),
        player_count: Some(record.player_count),
        fog_enabled: Some(record.fog_enabled),
        neutral_difficulty: Some(record.neutral_difficulty.clone()),
        victory_condition: Some(record.victory_condition.clone()),
        generation_key: Some(record.generation_key.clone()),
        naval_enabled: Some(record.naval_enabled),
        siege_enabled: Some(record.siege_enabled),
        larger_map_enabled: Some(record.larger_map_enabled),
        last_command_id: Some(None),
    };
    foundation::create("worldgen.create_skirmish_settings", input)
}

pub(crate) fn update_skirmish_settings(
    row: SkirmishSettingsState,
) -> RepoResult<SkirmishSettingsState> {
    foundation::update("worldgen.update_skirmish_settings", row)
}

pub(crate) fn find_procedural_map_by_key(
    session_id: Id<GameSession>,
    generation_key: &str,
) -> RepoResult<Option<ProceduralMapState>> {
    foundation::storage_result(
        PROCEDURAL_MAP_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<ProceduralMapState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("generation_key").eq(generation_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_procedural_maps_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<ProceduralMapState>> {
    foundation::execute_page(
        PROCEDURAL_MAPS_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<ProceduralMapState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("generation_key")
            .order_asc("id"),
        domm_game::MAX_GENERATED_CHUNKS_PER_UPDATE,
        None,
    )
}

pub(crate) fn create_procedural_map(
    session_id: Id<GameSession>,
    record: &domm_game::ProceduralMapRecord,
    command_id: Option<Id<GameCommand>>,
) -> RepoResult<ProceduralMapState> {
    let input: Create<ProceduralMapState> = Create::<ProceduralMapState> {
        session_id: Some(session_id.key()),
        generation_key: Some(record.generation_key.clone()),
        status: Some(record.status.clone()),
        map_seed: Some(record.map_seed),
        map_width: Some(record.map_width),
        map_height: Some(record.map_height),
        chunk_size: Some(record.chunk_size),
        chunk_count: Some(record.chunk_count),
        land_tile_count: Some(record.land_tile_count),
        water_tile_count: Some(record.water_tile_count),
        road_tile_count: Some(record.road_tile_count),
        town_count: Some(record.town_count),
        mine_count: Some(record.mine_count),
        scenario_hash: Some(record.scenario_hash.clone()),
        generated_turn: Some(record.generated_turn),
        last_command_id: Some(command_id.map(|id| id.key())),
    };
    foundation::create("worldgen.create_procedural_map", input)
}

pub(crate) fn update_procedural_map(row: ProceduralMapState) -> RepoResult<ProceduralMapState> {
    foundation::update("worldgen.update_procedural_map", row)
}

pub(crate) fn find_naval_route_by_key(
    session_id: Id<GameSession>,
    route_key: &str,
) -> RepoResult<Option<NavalRouteState>> {
    foundation::storage_result(
        NAVAL_ROUTE_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<NavalRouteState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("route_key").eq(route_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_naval_routes_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<NavalRouteState>> {
    foundation::execute_page(
        NAVAL_ROUTES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<NavalRouteState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("route_key")
            .order_asc("id"),
        domm_game::MAX_NAVAL_ROUTE_ROWS_PER_SESSION,
        None,
    )
}

pub(crate) fn create_naval_route(
    session_id: Id<GameSession>,
    record: &domm_game::NavalRouteRecord,
) -> RepoResult<NavalRouteState> {
    let input: Create<NavalRouteState> = Create::<NavalRouteState> {
        session_id: Some(session_id.key()),
        route_key: Some(record.route_key.clone()),
        status: Some(record.status.clone()),
        from_x: Some(record.from_x),
        from_y: Some(record.from_y),
        to_x: Some(record.to_x),
        to_y: Some(record.to_y),
        water_crossings: Some(record.water_crossings),
        boat_required: Some(record.boat_required),
        disabled_reason: Some(record.disabled_reason.clone()),
        last_command_id: Some(None),
    };
    foundation::create("worldgen.create_naval_route", input)
}

pub(crate) fn update_naval_route(row: NavalRouteState) -> RepoResult<NavalRouteState> {
    foundation::update("worldgen.update_naval_route", row)
}

pub(crate) fn find_siege_rule_by_key(
    session_id: Id<GameSession>,
    rule_key: &str,
) -> RepoResult<Option<SiegeRuleState>> {
    foundation::storage_result(
        SIEGE_RULE_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<SiegeRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("rule_key").eq(rule_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_siege_rules_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<SiegeRuleState>> {
    foundation::execute_page(
        SIEGE_RULES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<SiegeRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("rule_key")
            .order_asc("id"),
        domm_game::MAX_SIEGE_RULE_ROWS_PER_SESSION,
        None,
    )
}

pub(crate) fn create_siege_rule(
    session_id: Id<GameSession>,
    record: &domm_game::SiegeRuleRecord,
) -> RepoResult<SiegeRuleState> {
    let input: Create<SiegeRuleState> = Create::<SiegeRuleState> {
        session_id: Some(session_id.key()),
        rule_key: Some(record.rule_key.clone()),
        status: Some(record.status.clone()),
        fortification_level: Some(record.fortification_level.clone()),
        wall_segments: Some(record.wall_segments),
        gate_count: Some(record.gate_count),
        tower_count: Some(record.tower_count),
        siege_engine_slots: Some(record.siege_engine_slots),
        battle_obstacle_cap: Some(record.battle_obstacle_cap),
        disabled_reason: Some(record.disabled_reason.clone()),
        last_command_id: Some(None),
    };
    foundation::create("worldgen.create_siege_rule", input)
}

pub(crate) fn update_siege_rule(row: SiegeRuleState) -> RepoResult<SiegeRuleState> {
    foundation::update("worldgen.update_siege_rule", row)
}

#[cfg(test)]
#[cfg(test)]
pub(crate) fn skirmish_settings_plan_text(session_id: Id<GameSession>) -> RepoResult<String> {
    foundation::explain_text(
        SKIRMISH_SETTINGS_BY_SESSION_LOOKUP.name,
        crate::db()
            .load::<SkirmishSettingsState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn procedural_map_plan_text(
    session_id: Id<GameSession>,
    generation_key: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        PROCEDURAL_MAP_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<ProceduralMapState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("generation_key").eq(generation_key))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn naval_routes_plan_text(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        NAVAL_ROUTES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<NavalRouteState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("route_key")
            .order_asc("id")
            .limit(domm_game::MAX_NAVAL_ROUTE_ROWS_PER_SESSION),
    )
}

#[cfg(test)]
pub(crate) fn siege_rules_plan_text(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        SIEGE_RULES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<SiegeRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("rule_key")
            .order_asc("id")
            .limit(domm_game::MAX_SIEGE_RULE_ROWS_PER_SESSION),
    )
}
