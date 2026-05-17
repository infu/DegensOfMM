use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

use crate::battle::{BattleActionReceipt, BattleCoord, BattleSyncOutcome, BattleView};
use crate::champion::ChampionMagicReceipt;
use crate::champion::ChampionView;
use crate::command::{CommandPhase, CommandStatus, CommandStatusView};
use crate::content::ContentManifest;
use crate::driver::{PlayerView, SessionView};
use crate::economy::ExpandedEconomyReceipt;
use crate::economy::ResourceBalances;
use crate::lifecycle::{MatchHistoryEntry, ParticipantView};
use crate::map::{MapChunkView, ObjectView, Viewport};
use crate::movement::{MovementPreview, MovementSyncOutcome};
use crate::scenario::AdvancedScenarioReceipt;
use crate::strategic::StrategicCommandReceipt;
use crate::town::{
    ArmyStackRecord, BuildPreview, RecruitPreview, TownBuildingRecord, TownRecord,
    TownRecruitPoolRecord,
};
use crate::worldgen::WorldGenerationReceipt;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub details_json: Option<String>,
}

impl ApiError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
            details_json: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details_json: impl Into<String>) -> Self {
        self.details_json = Some(details_json.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PageInfo {
    pub next_cursor: Option<u32>,
    pub has_more: bool,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EventPageInfo {
    pub next_event_seq: Option<u64>,
    pub has_more: bool,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameViewRequest {
    pub viewport: Viewport,
    pub chunk_cursor: Option<u32>,
    pub chunk_limit: u32,
    pub object_cursor: Option<u32>,
    pub object_limit: u32,
    pub events_after_seq: u64,
    pub event_limit: u32,
    pub include_battle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RenderTimeMeta {
    pub server_now_ms: u64,
    pub turn_started_at_ms: u64,
    pub turn_duration_ms: u64,
    pub sync_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub state: String,
    pub participant_ids: Vec<String>,
    pub current_turn: u32,
}

impl SessionSummary {
    #[must_use]
    pub fn from_session(session: SessionView, current_turn: u32) -> Self {
        Self {
            session_id: session.session_id,
            state: session.state,
            participant_ids: session.participant_ids,
            current_turn,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ParticipantSummary {
    pub participant_id: String,
    pub player_id: String,
    pub faction_slug: String,
    pub slot_index: u8,
    pub status: String,
    pub ready: bool,
    pub resources: ResourceBalances,
}

impl From<ParticipantView> for ParticipantSummary {
    fn from(participant: ParticipantView) -> Self {
        Self {
            participant_id: participant.participant_id,
            player_id: participant.player_id,
            faction_slug: participant.faction_slug,
            slot_index: participant.slot_index,
            status: participant.status,
            ready: participant.ready,
            resources: ResourceBalances::from_cost(&participant.resources),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ApiTownView {
    pub town: TownRecord,
    pub buildings: Vec<TownBuildingRecord>,
    pub recruit_pools: Vec<TownRecruitPoolRecord>,
    pub garrison_stacks: Vec<ArmyStackRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleSummary {
    pub battle_id: String,
    pub state: String,
    pub battle_type: String,
    pub current_round: u16,
    pub active_stack_id: Option<String>,
    pub active_participant_id: Option<String>,
}

impl From<&BattleView> for BattleSummary {
    fn from(view: &BattleView) -> Self {
        Self {
            battle_id: view.battle_id.clone(),
            state: view.state.clone(),
            battle_type: view.battle_type.clone(),
            current_round: view.current_round,
            active_stack_id: view.active_stack_id.clone(),
            active_participant_id: view.active_participant_id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ActionAffordance {
    pub action: String,
    pub enabled: bool,
    pub target_id: Option<String>,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ApiEventView {
    pub session_id: String,
    pub event_seq: u64,
    pub event_key: String,
    pub audience_key: String,
    pub turn_number: u32,
    pub event_type: String,
    pub subject_kind: Option<String>,
    pub subject_id_text: Option<String>,
    pub payload: Option<String>,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ApiEventPage {
    pub events: Vec<ApiEventView>,
    pub page_info: EventPageInfo,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameView {
    pub session: SessionSummary,
    pub participant: ParticipantSummary,
    pub viewport: Viewport,
    pub map_chunks: Vec<MapChunkView>,
    pub map_page_info: PageInfo,
    pub objects: Vec<ObjectView>,
    pub object_page_info: PageInfo,
    pub champions: Vec<ChampionView>,
    pub towns: Vec<ApiTownView>,
    pub battle: Option<BattleView>,
    pub battle_summary: Option<BattleSummary>,
    pub events: Vec<ApiEventView>,
    pub event_page_info: EventPageInfo,
    pub content_manifest_hash: String,
    pub render_time: RenderTimeMeta,
    pub action_affordances: Vec<ActionAffordance>,
    pub omitted_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChangedSubject {
    pub subject_kind: String,
    pub subject_id_text: String,
    pub operation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum LobbyCommandResult {
    None,
    Player(PlayerView),
    Session(SessionView),
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum CommandResult {
    None,
    StrategicReceipt(StrategicCommandReceipt),
    MovementSync(MovementSyncOutcome),
    BuildPreview(BuildPreview),
    RecruitPreview(RecruitPreview),
    MovementPreview(MovementPreview),
    BattleAction(BattleActionReceipt),
    BattleSync(BattleSyncOutcome),
    ChampionMagic(ChampionMagicReceipt),
    ExpandedEconomy(ExpandedEconomyReceipt),
    AdvancedScenario(AdvancedScenarioReceipt),
    WorldGeneration(WorldGenerationReceipt),
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LobbyCommandResponse {
    pub command_id: String,
    pub command_type: String,
    pub actor_principal: Principal,
    pub client_nonce: String,
    pub payload_hash: String,
    pub status: CommandStatus,
    pub phase: CommandPhase,
    pub retryable: bool,
    pub effective_turn: u32,
    pub durable_turn: u32,
    pub events: Vec<ApiEventView>,
    pub changed_subjects: Vec<ChangedSubject>,
    pub result: LobbyCommandResult,
    pub error: Option<ApiError>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandResponse {
    pub command_id: String,
    pub command_type: String,
    pub actor_principal: Principal,
    pub actor_participant_id: Option<String>,
    pub client_nonce: String,
    pub payload_hash: String,
    pub status: CommandStatus,
    pub phase: CommandPhase,
    pub retryable: bool,
    pub effective_turn: u32,
    pub durable_turn: u32,
    pub events: Vec<ApiEventView>,
    pub changed_subjects: Vec<ChangedSubject>,
    pub result: CommandResult,
    pub error: Option<ApiError>,
}

impl CommandResponse {
    #[must_use]
    pub fn status_view(&self) -> CommandStatusView {
        CommandStatusView {
            command_id: self.command_id.clone(),
            status: self.status,
            phase: self.phase,
            retryable: self.retryable,
            error_code: self.error.as_ref().map(|error| error.code.clone()),
            error_message: self.error.as_ref().map(|error| error.message.clone()),
            result_json: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BattleActionInput {
    pub battle_id: String,
    pub battle_stack_id: String,
    pub action: String,
    pub ability_key: Option<String>,
    pub target_stack_id: Option<String>,
    pub destination: Option<BattleCoord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ApiMetrics {
    pub command_response_count: u32,
    pub lobby_response_count: u32,
    pub api_event_count: u32,
    pub strategic_command_count: u32,
    pub strategic_event_count: u32,
    pub strategic_query_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ContentManifestResponse {
    pub manifest: ContentManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MatchHistoryPage {
    pub entries: Vec<MatchHistoryEntry>,
    pub page_info: PageInfo,
}
