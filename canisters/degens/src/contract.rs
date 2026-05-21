use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum EndpointKind {
    Query,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndpointSpec {
    pub name: &'static str,
    pub kind: EndpointKind,
    pub group: &'static str,
    pub fixture_mapping: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CanisterEndpointView {
    pub name: String,
    pub kind: EndpointKind,
    pub group: String,
    pub fixture_mapping: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticRowCount {
    pub entity: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticStorageSnapshot {
    pub row_counts: Vec<DiagnosticRowCount>,
    pub total_rows: u32,
    pub stable_memory_pages: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticBenchmarkRepoOpView {
    pub operation: String,
    pub calls: u64,
    pub instruction_delta: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticBenchmarkCallView {
    pub sequence: u64,
    pub method: String,
    pub kind: String,
    pub ok: bool,
    pub error_code: Option<String>,
    pub instruction_delta: u64,
    pub stable_memory_pages_before: u64,
    pub stable_memory_pages_after: u64,
    pub repo_ops: Vec<DiagnosticBenchmarkRepoOpView>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticBenchmarkCallPage {
    pub calls: Vec<DiagnosticBenchmarkCallView>,
    pub next_cursor: Option<u64>,
    pub total_recorded: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticSystemJobView {
    pub job_key: String,
    pub job_kind: String,
    pub session_id: String,
    pub battle_id: Option<String>,
    pub turn_number: Option<u32>,
    pub due_at_ms: u64,
    pub status: String,
    pub lease_owner: Option<String>,
    pub lease_expires_at_ms: Option<u64>,
    pub attempt_count: u32,
    pub command_id: Option<String>,
    pub cursor_json: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DiagnosticSystemJobPage {
    pub jobs: Vec<DiagnosticSystemJobView>,
    pub next_cursor: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeferredEndpointDecision {
    pub name: &'static str,
    pub decision: &'static str,
}

pub const REQUIRED_GAME_ENDPOINTS: &[EndpointSpec] = &[
    EndpointSpec {
        name: "register_player",
        kind: EndpointKind::Update,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::register_player",
    },
    EndpointSpec {
        name: "get_my_player",
        kind: EndpointKind::Query,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::get_my_player",
    },
    EndpointSpec {
        name: "create_session",
        kind: EndpointKind::Update,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::create_session",
    },
    EndpointSpec {
        name: "join_session",
        kind: EndpointKind::Update,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::join_session",
    },
    EndpointSpec {
        name: "mark_ready",
        kind: EndpointKind::Update,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::mark_ready",
    },
    EndpointSpec {
        name: "start_session",
        kind: EndpointKind::Update,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::start_session",
    },
    EndpointSpec {
        name: "get_session",
        kind: EndpointKind::Query,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::get_session",
    },
    EndpointSpec {
        name: "get_setup_progress",
        kind: EndpointKind::Query,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::get_setup_progress",
    },
    EndpointSpec {
        name: "get_my_participant",
        kind: EndpointKind::Query,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::get_my_participant",
    },
    EndpointSpec {
        name: "get_match_history",
        kind: EndpointKind::Query,
        group: "account_lobby_session",
        fixture_mapping: "FixtureApiBackend::get_match_history",
    },
    EndpointSpec {
        name: "get_game_view",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_game_view",
    },
    EndpointSpec {
        name: "get_visible_map_chunks",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_visible_map_chunks",
    },
    EndpointSpec {
        name: "get_visible_objects",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_visible_objects",
    },
    EndpointSpec {
        name: "get_object_view",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_object_view",
    },
    EndpointSpec {
        name: "get_my_champions",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_my_champions",
    },
    EndpointSpec {
        name: "get_champion_view",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_champion_view",
    },
    EndpointSpec {
        name: "preview_champion_progression",
        kind: EndpointKind::Query,
        group: "champion_magic",
        fixture_mapping: "FixtureApiBackend::preview_champion_progression",
    },
    EndpointSpec {
        name: "select_champion_level_up",
        kind: EndpointKind::Update,
        group: "champion_magic",
        fixture_mapping: "FixtureApiBackend::select_champion_level_up",
    },
    EndpointSpec {
        name: "learn_champion_spell",
        kind: EndpointKind::Update,
        group: "champion_magic",
        fixture_mapping: "FixtureApiBackend::learn_champion_spell",
    },
    EndpointSpec {
        name: "cast_adventure_spell",
        kind: EndpointKind::Update,
        group: "champion_magic",
        fixture_mapping: "FixtureApiBackend::cast_adventure_spell",
    },
    EndpointSpec {
        name: "get_tavern_offers",
        kind: EndpointKind::Query,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::get_tavern_offers",
    },
    EndpointSpec {
        name: "preview_hire_champion",
        kind: EndpointKind::Query,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::preview_hire_champion",
    },
    EndpointSpec {
        name: "hire_tavern_champion",
        kind: EndpointKind::Update,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::hire_tavern_champion",
    },
    EndpointSpec {
        name: "preview_market_trade",
        kind: EndpointKind::Query,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::preview_market_trade",
    },
    EndpointSpec {
        name: "submit_market_trade",
        kind: EndpointKind::Update,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::submit_market_trade",
    },
    EndpointSpec {
        name: "get_dwelling_pool",
        kind: EndpointKind::Query,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::get_dwelling_pool",
    },
    EndpointSpec {
        name: "preview_dwelling_recruit",
        kind: EndpointKind::Query,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::preview_dwelling_recruit",
    },
    EndpointSpec {
        name: "submit_dwelling_recruit",
        kind: EndpointKind::Update,
        group: "economy_expansion",
        fixture_mapping: "FixtureApiBackend::submit_dwelling_recruit",
    },
    EndpointSpec {
        name: "get_objective_progress",
        kind: EndpointKind::Query,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::get_objective_progress",
    },
    EndpointSpec {
        name: "get_scenario_rules",
        kind: EndpointKind::Query,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::get_scenario_rules",
    },
    EndpointSpec {
        name: "get_world_events",
        kind: EndpointKind::Query,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::get_world_events",
    },
    EndpointSpec {
        name: "preview_quest",
        kind: EndpointKind::Query,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::preview_quest",
    },
    EndpointSpec {
        name: "accept_quest",
        kind: EndpointKind::Update,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::accept_quest",
    },
    EndpointSpec {
        name: "claim_quest_reward",
        kind: EndpointKind::Update,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::claim_quest_reward",
    },
    EndpointSpec {
        name: "sync_objectives",
        kind: EndpointKind::Update,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::sync_objectives",
    },
    EndpointSpec {
        name: "sync_world_events",
        kind: EndpointKind::Update,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::sync_world_events",
    },
    EndpointSpec {
        name: "sync_advanced_victory",
        kind: EndpointKind::Update,
        group: "scenario_progress",
        fixture_mapping: "FixtureApiBackend::sync_advanced_victory",
    },
    EndpointSpec {
        name: "get_skirmish_settings",
        kind: EndpointKind::Query,
        group: "worldgen",
        fixture_mapping: "FixtureApiBackend::get_skirmish_settings",
    },
    EndpointSpec {
        name: "get_procedural_map_state",
        kind: EndpointKind::Query,
        group: "worldgen",
        fixture_mapping: "FixtureApiBackend::get_procedural_map_state",
    },
    EndpointSpec {
        name: "get_naval_routes",
        kind: EndpointKind::Query,
        group: "worldgen",
        fixture_mapping: "FixtureApiBackend::get_naval_routes",
    },
    EndpointSpec {
        name: "get_siege_rules",
        kind: EndpointKind::Query,
        group: "worldgen",
        fixture_mapping: "FixtureApiBackend::get_siege_rules",
    },
    EndpointSpec {
        name: "sync_world_generation",
        kind: EndpointKind::Update,
        group: "worldgen",
        fixture_mapping: "FixtureApiBackend::sync_world_generation",
    },
    EndpointSpec {
        name: "get_town_view",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_town_view",
    },
    EndpointSpec {
        name: "get_battle_state",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_battle_state",
    },
    EndpointSpec {
        name: "get_content_manifest",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_content_manifest",
    },
    EndpointSpec {
        name: "get_events_after",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_events_after",
    },
    EndpointSpec {
        name: "get_command_status",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_command_status",
    },
    EndpointSpec {
        name: "get_command_status_by_nonce",
        kind: EndpointKind::Query,
        group: "render_query",
        fixture_mapping: "FixtureApiBackend::get_command_status_by_nonce",
    },
    EndpointSpec {
        name: "preview_move_path",
        kind: EndpointKind::Query,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::preview_move",
    },
    EndpointSpec {
        name: "preview_build_town_structure",
        kind: EndpointKind::Query,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::preview_build_town_structure",
    },
    EndpointSpec {
        name: "preview_recruit_units",
        kind: EndpointKind::Query,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::preview_recruit_units",
    },
    EndpointSpec {
        name: "submit_move_intent",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::submit_move_intent",
    },
    EndpointSpec {
        name: "end_turn",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::end_turn",
    },
    EndpointSpec {
        name: "sync_session_turn",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::sync_session_turn",
    },
    EndpointSpec {
        name: "submit_build_town_structure",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::submit_build_town_structure",
    },
    EndpointSpec {
        name: "submit_recruit_units",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::submit_recruit_units",
    },
    EndpointSpec {
        name: "sync_battle",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::sync_battle",
    },
    EndpointSpec {
        name: "end_battle_turn",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::end_battle_turn",
    },
    EndpointSpec {
        name: "submit_battle_action",
        kind: EndpointKind::Update,
        group: "preview_update",
        fixture_mapping: "FixtureApiBackend::submit_battle_action",
    },
];

pub const DEFERRED_ENDPOINT_DECISIONS: &[DeferredEndpointDecision] = &[
    DeferredEndpointDecision {
        name: "leave_session",
        decision: "deferred until lobby cancellation/leave semantics are promoted into the canister API; return typed unavailable behavior if surfaced earlier",
    },
    DeferredEndpointDecision {
        name: "cancel_session",
        decision: "deferred until lobby cancellation/leave semantics are promoted into the canister API; return typed unavailable behavior if surfaced earlier",
    },
    DeferredEndpointDecision {
        name: "surrender",
        decision: "explicitly disabled in v1 alongside retreat/surrender policy until Part 2 expands the command, event, and victory contract",
    },
    DeferredEndpointDecision {
        name: "retreat",
        decision: "explicitly disabled in v1 alongside retreat/surrender policy until Part 2 expands the battle aftermath contract",
    },
    DeferredEndpointDecision {
        name: "request_rematch",
        decision: "client affordance only for v1; durable rematch creation remains deferred to multiplayer meta expansion",
    },
];

pub fn required_endpoint_views() -> Vec<CanisterEndpointView> {
    REQUIRED_GAME_ENDPOINTS
        .iter()
        .map(|endpoint| CanisterEndpointView {
            name: endpoint.name.to_string(),
            kind: endpoint.kind,
            group: endpoint.group.to_string(),
            fixture_mapping: endpoint.fixture_mapping.to_string(),
        })
        .collect()
}

pub const fn deferred_endpoint_decisions() -> &'static [DeferredEndpointDecision] {
    DEFERRED_ENDPOINT_DECISIONS
}
