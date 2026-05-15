//! Pure deterministic harness code for Degens of Misery & Mayhem.
//!
//! This crate deliberately has no IcyDB dependency. It owns fixture constants,
//! public DTO shapes used by tests, and the headless driver contract so gameplay
//! rules can be tested before and alongside persistence work.

pub mod command;
pub mod content;
pub mod driver;
pub mod fixtures;
pub mod lifecycle;
pub mod map;
pub mod rng;

pub use command::{
    ActorKind, CommandActor, CommandCoreError, CommandEffectRecord, CommandPhase, CommandStatus,
    CommandStatusView, CommandSubmitOutcome, EffectStatus, EventAppendOutcome, EventAudience,
    EventView, GameCommandPayload, GameCommandRecord, GameEventDraft, GameEventRecord,
    GameEventTurnSummaryRecord, LobbyCommandJournal, LobbyCommandPayload, LobbyCommandRecord,
    LobbyCommandSubmitOutcome, PendingEffectDraft, PendingEffectRecord, RecoveryBudget,
    RecoveryOutcome, SessionCommandJournal, recovery_effect_key, recovery_event_key,
};
pub use content::{
    ArmyStackSeed, ArtifactContent, BuildingContent, ChampionClassContent, ContentManifest,
    FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH,
    FIRST_PLAYABLE_MAX_TURNS, FIRST_PLAYABLE_PLAYER_COUNT, FIRST_PLAYABLE_RULESET_ID,
    FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION, FactionContent,
    FirstPlayableScenario, FirstPlayableWalkthrough, HandAuthoredMap, MapObjectContent,
    NeutralArmySeed, ObjectSeed, PlayerStart, ResourceCost, ResourcePileSeed, RoadPath,
    RulesetContent, SpellContent, StartingState, TerrainContent, TerrainPatch, TileCoord,
    UnitContent, WalkthroughStep, first_playable_content_manifest, first_playable_scenario,
    get_content_manifest,
};
pub use driver::{
    ActiveMatchView, DriverCall, DriverError, HeadlessBackend, HeadlessGameDriver, PlayerView,
    ScriptedBackend, SessionView,
};
pub use fixtures::{
    CommandNonces, FIRST_PLAYABLE_SCENARIO_SEED, FixtureClock, FixtureIds, FixturePrincipals,
    ScenarioFixture, TURN_DURATION_MS, first_playable_fixture,
};
pub use lifecycle::{
    LifecycleBackend, LifecycleError, MatchHistoryEntry, ParticipantView, SetupProjection,
};
pub use map::{
    FirstPlayableMapState, MAP_FLAG_BLOCKING_TERRAIN, MAP_FLAG_PASSABLE, MAP_FLAG_ROAD,
    MapChunkPage, MapChunkRecord, MapChunkView, MapError, MapOccupancyRecord, MapSubjectRecord,
    ObjectView, ObjectViewPage, OpeningViewportSnapshot, ParticipantKnownObjectRecord,
    SubjectViewResult, Viewport, VisibilityChunkRecord, WorldObjectRecord,
    build_first_playable_map_state, build_first_playable_map_state_for_ids, empty_visibility_blob,
    read_visibility_bit, set_visibility_bit,
};
pub use rng::{
    BoundedRoll, DeterministicRoll, RngError, RollAudit, RollKey, hash64, roll_below,
    roll_between_inclusive,
};
