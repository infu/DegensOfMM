pub mod rules;
#[cfg(test)]
mod tests;
pub mod types;

pub use rules::{
    deterministic_procedural_map, first_playable_naval_route, first_playable_siege_rule,
    first_playable_skirmish_settings, validate_boat_movement, validate_generation_caps,
};
pub use types::{
    MAX_GENERATED_CHUNKS_PER_UPDATE, MAX_GENERATED_MAP_HEIGHT, MAX_GENERATED_MAP_WIDTH,
    MAX_NAVAL_ROUTE_ROWS_PER_SESSION, MAX_SIEGE_RULE_ROWS_PER_SESSION, NAVAL_ROUTE_KEY,
    NavalRouteRecord, NavalRoutesView, PROCEDURAL_GENERATION_KEY, ProceduralMapRecord,
    ProceduralMapView, SIEGE_RULE_KEY, SKIRMISH_PROFILE_KEY, SiegeRuleRecord, SiegeRulesView,
    SkirmishSettingsRecord, SkirmishSettingsView, WorldGenerationReceipt, WorldgenError,
};
