//! Pure deterministic harness code for Degens of Misery & Mayhem.
//!
//! This crate deliberately has no IcyDB dependency. It owns fixture constants,
//! public DTO shapes used by tests, and the headless driver contract so gameplay
//! rules can be tested before and alongside persistence work.

pub mod aftermath;
pub mod ai;
pub mod api;
pub mod battle;
pub mod champion;
pub mod cleanup;
pub mod command;
pub mod content;
pub mod driver;
pub mod e2e;
pub mod economy;
pub mod effects;
pub mod fixtures;
pub mod lifecycle;
pub mod limits;
pub mod map;
pub mod movement;
pub mod neutral;
pub mod playable;
pub mod rng;
pub mod strategic;
pub mod town;
pub mod world_object;

pub use aftermath::{
    AftermathError, AftermathEventRecord, AftermathSmokeView, AftermathState,
    BattleAftermathReport, MatchSessionRecord, PlayerMatchSummaryRecord, RetreatSurrenderPolicy,
    VictoryCheck, VictoryScore, apply_battle_aftermath, build_first_playable_aftermath_state,
    check_and_finalize_victory, finalize_stalemate, require_retreat_or_surrender_enabled,
    resolve_neutral_battle_for_fixture, retreat_surrender_policy,
    run_first_playable_aftermath_smoke, seed_resolved_champion_defeat_battle,
    seed_resolved_town_capture_battle,
};
pub use ai::{
    AI_MAX_ACTORS_PER_UPDATE, AI_MAX_CANDIDATES_PER_ACTOR, AI_MAX_CHUNKS_LOADED_PER_ACTOR,
    AI_MAX_EMITTED_COMMANDS_PER_UPDATE, AI_MAX_PATH_NODES_PER_ACTOR, AiActorStateRecord,
    AiCommandDraft, AiDecisionInput, AiError, AiUpdateReport, decide_for_actor, run_ai_update,
};
pub use api::{
    ActionAffordance, ApiError, ApiEventPage, ApiEventView, ApiMetrics, ApiTownView,
    BattleActionInput, BattleSummary, ChangedSubject, CommandResponse, CommandResult,
    ContentManifestResponse, DEFAULT_CHUNK_LIMIT, DEFAULT_EVENT_LIMIT, DEFAULT_OBJECT_LIMIT,
    EventPageInfo, FixtureApiBackend, GameView, GameViewRequest, LobbyCommandResponse,
    LobbyCommandResult, MAX_CHUNK_LIMIT, MAX_EVENT_LIMIT, MAX_OBJECT_LIMIT, MAX_VIEWPORT_TILES,
    MatchHistoryPage, PageInfo, ParticipantSummary, RenderTimeMeta, SessionSummary,
    opening_viewport_for_slot,
};
pub use battle::{
    BATTLE_ACTION_DEADLINE_MS, BATTLE_GRID_HEIGHT, BATTLE_GRID_WIDTH, BATTLE_MAX_ROUNDS,
    BATTLE_SIDE_ATTACKER, BATTLE_SIDE_DEFENDER, BattleActionReceipt, BattleCommandBudget,
    BattleCommandRecord, BattleCoord, BattleDamageOutcome, BattleError, BattleEventRecord,
    BattleEventView, BattleGridView, BattleInitiativeEntry, BattleMoraleLuckPolicy,
    BattleObstacleRecord, BattleObstacleView, BattleOccupancyRecord, BattleRecord, BattleSmokeView,
    BattleStackRecord, BattleStackView, BattleState, BattleSyncOutcome, BattleView, DamagePreview,
    LegalBattleAction, adjacent_coords, append_battle_event, apply_battle_command_by_id,
    apply_damage_to_stack, apply_stack_attack, battle_action_payload_hash,
    battle_view_for_participant, build_first_playable_battle_state, damage_preview,
    initiative_order, legal_actions_for_stack, occupant_at, reachable_tiles,
    recover_applying_battle_commands, repair_stack_position_from_occupancy,
    run_first_playable_battle_smoke, select_active_stack_id, submit_battle_action, sync_battle,
    v1_morale_luck_policy, validate_battle_occupancy, validate_battle_stack_status_keys,
};
pub use champion::{
    ArtifactCaptureResult, ArtifactEquipmentRecord, ArtifactInstanceRecord, ArtifactView,
    CHAMPION_LEVEL_CAP, ChampionArmyStackRecord, ChampionError, ChampionProgressionResult,
    ChampionRecord, ChampionState, ChampionView, ChampionViewResult,
    build_first_playable_champion_state,
};
pub use cleanup::{
    ACTIVE_SESSION_LIMIT, CLEANUP_MAX_FINISHED_SESSIONS_PER_UPDATE, CLEANUP_MAX_ROWS_PER_UPDATE,
    CleanupBudget, CleanupCanisterSnapshot, CleanupError, CleanupPolicy, CleanupReport,
    CleanupTarget, RAW_FINISHED_LOG_RETENTION_MS, RAW_FINISHED_SESSION_LIMIT,
    assert_active_session_capacity, compact_finished_session, should_compact_raw_finished_logs,
};
pub use command::{
    ActorKind, CommandActor, CommandCoreError, CommandEffectRecord, CommandPhase, CommandStatus,
    CommandStatusView, CommandSubmitOutcome, EffectStatus, EventAppendOutcome, EventAudience,
    EventPage, EventView, GameCommandPayload, GameCommandRecord, GameEventDraft, GameEventRecord,
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
pub use e2e::{
    EndToEndCoverage, EndToEndError, EndToEndFirstPlayableReport, EndToEndMeasurements,
    ManualSmokeCommand, MovementConflictReport, SpecAuditRow, SpecAuditStatus, part_two_spec_audit,
    run_e2e_movement_conflict_probe, run_first_playable_e2e_fixture,
};
pub use economy::{
    BASE_TOWN_GOLD_INCOME, EconomyError, EconomyParticipantRecord, EconomySmokeView, EconomyState,
    IncomeSourceRecord, ResourceApplyBudget, ResourceApplyOutcome, ResourceBalances,
    ResourceCapMode, ResourceDelta, ResourceLedgerEntryRecord, ResourceLedgerTurnSummaryRecord,
    ResourcePileEconomyRecord, build_first_playable_economy_state,
    run_first_playable_economy_smoke,
};
pub use effects::{
    EffectDomain, EffectError, EffectRequest, EffectResolution, LegalEffectAction, dispatch_effect,
    legal_effect_action, resolve_chance_effect, validate_status_keys,
};
pub use fixtures::{
    CommandNonces, FIRST_PLAYABLE_SCENARIO_SEED, FixtureClock, FixtureIds, FixturePrincipals,
    ScenarioFixture, TURN_DURATION_MS, first_playable_fixture,
};
pub use lifecycle::{
    LifecycleBackend, LifecycleError, MatchHistoryEntry, ParticipantView, SetupProjection,
};
pub use limits::{
    DEFAULT_LIST_LIMIT, GAME_COMMAND_EFFECT_CAP, GAME_COMMAND_EVENT_CAP,
    MAX_ACTIVE_BATTLES_PER_SESSION, MAX_ACTIVE_SESSIONS_PER_CANISTER, MAX_AI_ACTORS_PER_UPDATE,
    MAX_AI_CANDIDATES_PER_ACTOR, MAX_AI_CHUNKS_LOADED_PER_ACTOR,
    MAX_AI_EMITTED_COMMANDS_PER_UPDATE, MAX_AI_PATH_NODES_PER_ACTOR, MAX_BATTLE_OBSTACLES,
    MAX_BATTLE_ROUNDS as LIMIT_MAX_BATTLE_ROUNDS, MAX_BATTLE_STARTS_FROM_MOVEMENT,
    MAX_BATTLE_TIMEOUT_ACTIONS_PER_UPDATE, MAX_CHAMPIONS_PER_PARTICIPANT,
    MAX_CLEANUP_ROWS_PER_UPDATE as LIMIT_MAX_CLEANUP_ROWS_PER_UPDATE,
    MAX_COMMAND_PAYLOAD_JSON_BYTES, MAX_COMMAND_RESULT_JSON_BYTES,
    MAX_COMMANDS_RETAINED_PER_ACTIVE_SESSION, MAX_DYNAMIC_OBJECTS_PER_SESSION,
    MAX_EVENT_PAYLOAD_JSON_BYTES, MAX_EVENTS_PER_TURN, MAX_EVENTS_RETAINED_PER_ACTIVE_SESSION,
    MAX_FINISHED_SESSIONS_CLEANED_PER_UPDATE as LIMIT_MAX_FINISHED_SESSIONS_CLEANED_PER_UPDATE,
    MAX_LIST_LIMIT, MAX_MAP_CHUNKS_PER_SESSION, MAX_MAP_HEIGHT, MAX_MAP_WIDTH,
    MAX_MOVE_CHUNKS_TOUCHED_LIMIT, MAX_MOVE_PATH_STEPS_LIMIT, MAX_MOVEMENT_INTENT_PATH_JSON_BYTES,
    MAX_MOVEMENT_MICROSTEPS_PER_SYNC, MAX_OBJECT_INTERACTIONS_FROM_MOVEMENT,
    MAX_PARTICIPANTS_PER_SESSION, MAX_RECENT_EVENTS_IN_GAME_VIEW,
    MAX_RESOURCE_LEDGER_ROWS_RETAINED_PER_ACTIVE_SESSION, MAX_STACKS_PER_BATTLE_SIDE,
    MAX_TOWNS_PER_SESSION, MAX_UNRESOLVED_MOVEMENT_INTENTS_PER_TURN,
    MAX_VIEWPORT_CHUNKS_PER_REQUEST, PerformanceBudgetError, PerformanceBudgetReport,
    RECOVERY_COMMAND_EFFECTS_PER_UPDATE, RECOVERY_COMMANDS_ADVANCED_PER_UPDATE,
    RECOVERY_COMMANDS_INSPECTED_PER_UPDATE, RECOVERY_GAME_EVENTS_PER_UPDATE,
    RECOVERY_GAMEPLAY_ROWS_PER_UPDATE, TURN_CATCHUP_CAP, measure_first_playable_performance,
};
pub use map::{
    FirstPlayableMapState, MAP_FLAG_BLOCKING_TERRAIN, MAP_FLAG_PASSABLE, MAP_FLAG_ROAD,
    MapChunkPage, MapChunkRecord, MapChunkView, MapError, MapOccupancyRecord, MapSubjectRecord,
    OPENING_VIEWPORT_EAST_X, OPENING_VIEWPORT_EAST_Y, OPENING_VIEWPORT_HEIGHT,
    OPENING_VIEWPORT_WEST_X, OPENING_VIEWPORT_WEST_Y, OPENING_VIEWPORT_WIDTH, ObjectView,
    ObjectViewPage, OpeningViewportSnapshot, ParticipantKnownObjectRecord, SubjectViewResult,
    Viewport, VisibilityChunkRecord, WorldObjectRecord, build_first_playable_map_state,
    build_first_playable_map_state_for_ids, empty_visibility_blob, read_visibility_bit,
    set_visibility_bit,
};
pub use movement::{
    BattleStartDraft, MoveCoord, MovementError, MovementIntentRecord, MovementIntentSubmitOutcome,
    MovementPathStop, MovementPreview, MovementResolutionCursor, MovementSmokeView,
    MovementSnapshotRecord, MovementState, MovementSyncBudget, MovementSyncOutcome,
    MovementSystemCommandRecord, MovementTimeView, ObjectStopDraft,
    build_first_playable_movement_state, preview_move_path, run_first_playable_movement_smoke,
    submit_move_intent, sync_session_turn,
};
pub use neutral::{
    NeutralArmyEncounterRecord, NeutralArmyRecord, NeutralArmyStackRecord, NeutralArmyView,
    NeutralArmyViewResult, NeutralBehaviorPolicy, NeutralError, NeutralGrowthOutcome,
    NeutralSmokeView, NeutralState, apply_neutral_encounters_from_movement,
    build_first_playable_neutral_state, defeat_neutral_army, materialize_neutral_growth,
    run_first_playable_neutral_smoke, start_guarded_object_encounter, start_neutral_encounter,
    strength_label_for_quantity,
};
pub use playable::{
    PlayableBattleView, PlayableCall, PlayableCommandReceipt, PlayableError, PlayableEventPage,
    PlayableFixtureBackend, PlayableGateReport, PlayableMatchView, run_first_playable_backend_gate,
};
pub use rng::{
    BoundedRoll, DeterministicRoll, RngError, RollAudit, RollKey, hash64, roll_below,
    roll_between_inclusive,
};
pub use strategic::{
    StrategicBackend, StrategicCall, StrategicCommandReceipt, StrategicError,
    StrategicFixtureBackend, StrategicGameView, StrategicGateReport, StrategicHeadlessDriver,
    StrategicStepView, run_first_playable_strategic_gate,
};
pub use town::{
    ArmyStackRecord, BuildPreview, ChampionTownRecord, RecruitPreview, RecruitTarget,
    TownBuildingRecord, TownError, TownRecord, TownRecruitPoolRecord, TownSmokeView, TownState,
    build_first_playable_town_state, run_first_playable_town_smoke,
};
pub use world_object::{
    ChampionObjectVisitRecord, ObjectCommandEffectRecord, ObjectInteractionCommandRecord,
    ObjectInteractionOutcome, ObjectResourceOutcome, ObjectScoreRecord,
    ParticipantObjectVisitRecord, WorldObjectError, WorldObjectSmokeView, WorldObjectState,
    apply_movement_object_interactions, build_first_playable_world_object_state,
    interact_with_world_object, record_champion_object_visit, record_participant_object_visit,
    run_first_playable_world_object_smoke, world_object_scoreboard,
};
