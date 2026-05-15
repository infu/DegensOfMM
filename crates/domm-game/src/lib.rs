//! Pure deterministic harness code for Degens of Misery & Mayhem.
//!
//! This crate deliberately has no IcyDB dependency. It owns fixture constants,
//! public DTO shapes used by tests, and the headless driver contract so gameplay
//! rules can be tested before and alongside persistence work.

pub mod command;
pub mod driver;
pub mod fixtures;

pub use command::{
    ActorKind, CommandActor, CommandCoreError, CommandEffectRecord, CommandPhase, CommandStatus,
    CommandStatusView, CommandSubmitOutcome, EffectStatus, EventAppendOutcome, EventAudience,
    EventView, GameCommandPayload, GameCommandRecord, GameEventDraft, GameEventRecord,
    GameEventTurnSummaryRecord, LobbyCommandJournal, LobbyCommandPayload, LobbyCommandRecord,
    LobbyCommandSubmitOutcome, PendingEffectDraft, PendingEffectRecord, RecoveryBudget,
    RecoveryOutcome, SessionCommandJournal, recovery_effect_key, recovery_event_key,
};
pub use driver::{
    ActiveMatchView, DriverCall, DriverError, HeadlessBackend, HeadlessGameDriver, PlayerView,
    ScriptedBackend, SessionView,
};
pub use fixtures::{
    CommandNonces, FIRST_PLAYABLE_SCENARIO_SEED, FixtureClock, FixtureIds, FixturePrincipals,
    ScenarioFixture, TURN_DURATION_MS, first_playable_fixture,
};
