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
