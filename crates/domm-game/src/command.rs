use std::fmt::Write as _;

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const RECOVERY_INSPECT_LIMIT: usize = 25;
pub const RECOVERY_ADVANCE_LIMIT: usize = 8;
pub const RECOVERY_COMMAND_EFFECT_LIMIT: usize = 32;
pub const RECOVERY_GAME_EVENT_LIMIT: usize = 32;
pub const RECOVERY_GAMEPLAY_ROW_LIMIT: usize = 160;
pub const COMMAND_EFFECT_LIMIT: usize = 16;
pub const COMMAND_EVENT_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum ActorKind {
    Player,
    System,
    Ai,
}

impl ActorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::System => "system",
            Self::Ai => "ai",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum CommandStatus {
    Pending,
    Applying,
    Applied,
    Failed,
    Cancelled,
    Superseded,
    AppliedNoop,
}

impl CommandStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applying => "applying",
            Self::Applied => "applied",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::AppliedNoop => "applied_noop",
        }
    }

    #[must_use]
    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::Pending | Self::Applying)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum CommandPhase {
    Created,
    Validated,
    Applying,
    EffectsApplied,
    EventsApplied,
    Recovered,
    Complete,
    Failed,
}

impl CommandPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Validated => "validated",
            Self::Applying => "applying",
            Self::EffectsApplied => "effects_applied",
            Self::EventsApplied => "events_applied",
            Self::Recovered => "recovered",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum EffectStatus {
    Pending,
    Applied,
    Failed,
    Cancelled,
}

impl EffectStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandActor {
    pub kind: ActorKind,
    pub actor_id_text: String,
    pub actor_player_id: Option<String>,
    pub actor_participant_id: Option<String>,
    pub champion_id: Option<String>,
}

impl CommandActor {
    #[must_use]
    pub fn player(
        actor_player_id: impl Into<String>,
        actor_participant_id: impl Into<String>,
        champion_id: Option<String>,
    ) -> Self {
        let actor_player_id = actor_player_id.into();
        Self {
            kind: ActorKind::Player,
            actor_id_text: actor_player_id.clone(),
            actor_player_id: Some(actor_player_id),
            actor_participant_id: Some(actor_participant_id.into()),
            champion_id,
        }
    }

    #[must_use]
    pub fn system(actor_id_text: impl Into<String>) -> Self {
        Self {
            kind: ActorKind::System,
            actor_id_text: actor_id_text.into(),
            actor_player_id: None,
            actor_participant_id: None,
            champion_id: None,
        }
    }

    #[must_use]
    pub fn ai(actor_id_text: impl Into<String>) -> Self {
        Self {
            kind: ActorKind::Ai,
            actor_id_text: actor_id_text.into(),
            actor_player_id: None,
            actor_participant_id: None,
            champion_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameCommandPayload {
    pub session_id: String,
    pub actor: CommandActor,
    pub turn_number: u32,
    pub client_nonce: u64,
    pub command_type: String,
    pub typed_payload_bytes: Vec<u8>,
    pub payload_json: String,
}

impl GameCommandPayload {
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        actor: CommandActor,
        turn_number: u32,
        client_nonce: u64,
        command_type: impl Into<String>,
        typed_payload_bytes: Vec<u8>,
        payload_json: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            actor,
            turn_number,
            client_nonce,
            command_type: command_type.into(),
            typed_payload_bytes,
            payload_json: payload_json.into(),
        }
    }

    #[must_use]
    pub fn from_json(
        session_id: impl Into<String>,
        actor: CommandActor,
        turn_number: u32,
        client_nonce: u64,
        command_type: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Self {
        let payload_json = payload_json.into();
        Self::new(
            session_id,
            actor,
            turn_number,
            client_nonce,
            command_type,
            payload_json.as_bytes().to_vec(),
            payload_json,
        )
    }

    #[must_use]
    pub fn payload_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hash_text(&mut hasher, "command_type", &self.command_type);
        hash_text(&mut hasher, "session_id", &self.session_id);
        hash_text(&mut hasher, "actor_kind", self.actor.kind.as_str());
        hash_text(&mut hasher, "actor_id_text", &self.actor.actor_id_text);
        hash_optional_text(
            &mut hasher,
            "actor_player_id",
            self.actor.actor_player_id.as_deref(),
        );
        hash_optional_text(
            &mut hasher,
            "actor_participant_id",
            self.actor.actor_participant_id.as_deref(),
        );
        hash_optional_text(
            &mut hasher,
            "champion_id",
            self.actor.champion_id.as_deref(),
        );
        hash_text(&mut hasher, "turn_number", &self.turn_number.to_string());
        hash_bytes(
            &mut hasher,
            "typed_payload_bytes",
            &self.typed_payload_bytes,
        );
        to_hex(&hasher.finalize())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameCommandRecord {
    pub id: String,
    pub session_id: String,
    pub actor_kind: ActorKind,
    pub actor_id_text: String,
    pub actor_player_id: Option<String>,
    pub actor_participant_id: Option<String>,
    pub champion_id: Option<String>,
    pub turn_number: u32,
    pub client_nonce: u64,
    pub command_type: String,
    pub status: CommandStatus,
    pub phase: CommandPhase,
    pub payload_hash: String,
    pub payload_json: String,
    pub result_json: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_details_json: Option<String>,
    pub retryable: bool,
    pub created_at_ms: u64,
    pub applied_at_ms: Option<u64>,
    pub failed_at_ms: Option<u64>,
}

impl GameCommandRecord {
    #[must_use]
    pub fn status_view(&self) -> CommandStatusView {
        CommandStatusView {
            command_id: self.id.clone(),
            status: self.status,
            phase: self.phase,
            retryable: self.retryable,
            error_code: self.error_code.clone(),
            error_message: self.error_message.clone(),
            result_json: self.result_json.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LobbyCommandPayload {
    pub actor_principal: Principal,
    pub actor_player_id: Option<String>,
    pub client_nonce: u64,
    pub command_type: String,
    pub typed_payload_bytes: Vec<u8>,
    pub payload_json: String,
}

impl LobbyCommandPayload {
    #[must_use]
    pub fn from_json(
        actor_principal: Principal,
        actor_player_id: Option<String>,
        client_nonce: u64,
        command_type: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Self {
        let payload_json = payload_json.into();
        Self {
            actor_principal,
            actor_player_id,
            client_nonce,
            command_type: command_type.into(),
            typed_payload_bytes: payload_json.as_bytes().to_vec(),
            payload_json,
        }
    }

    #[must_use]
    pub fn payload_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hash_text(&mut hasher, "command_type", &self.command_type);
        hash_text(
            &mut hasher,
            "actor_principal",
            &self.actor_principal.to_text(),
        );
        hash_optional_text(
            &mut hasher,
            "actor_player_id",
            self.actor_player_id.as_deref(),
        );
        hash_bytes(
            &mut hasher,
            "typed_payload_bytes",
            &self.typed_payload_bytes,
        );
        to_hex(&hasher.finalize())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LobbyCommandRecord {
    pub id: String,
    pub actor_principal: Principal,
    pub actor_player_id: Option<String>,
    pub client_nonce: u64,
    pub payload_hash: String,
    pub command_type: String,
    pub status: CommandStatus,
    pub phase: CommandPhase,
    pub payload_json: String,
    pub result_json: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_details_json: Option<String>,
    pub retryable: bool,
    pub created_at_ms: u64,
    pub applied_at_ms: Option<u64>,
    pub failed_at_ms: Option<u64>,
}

impl LobbyCommandRecord {
    #[must_use]
    pub fn status_view(&self) -> CommandStatusView {
        CommandStatusView {
            command_id: self.id.clone(),
            status: self.status,
            phase: self.phase,
            retryable: self.retryable,
            error_code: self.error_code.clone(),
            error_message: self.error_message.clone(),
            result_json: self.result_json.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandEffectRecord {
    pub id: String,
    pub session_id: String,
    pub command_id: String,
    pub effect_key: String,
    pub effect_type: String,
    pub target_kind: String,
    pub target_id_text: String,
    pub status: EffectStatus,
    pub payload_json: String,
    pub applied_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PendingEffectRecord {
    pub id: String,
    pub session_id: String,
    pub source_command_id: Option<String>,
    pub target_participant_id: Option<String>,
    pub target_champion_id: Option<String>,
    pub effect_key: String,
    pub due_turn: u32,
    pub effect_type: String,
    pub status: EffectStatus,
    pub payload_json: String,
    pub applied_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum EventAudience {
    Public,
    Participant {
        participant_id: String,
    },
    BattleParticipant {
        battle_id: String,
        participant_id: String,
    },
}

impl EventAudience {
    #[must_use]
    pub fn public() -> Self {
        Self::Public
    }

    #[must_use]
    pub fn participant(participant_id: impl Into<String>) -> Self {
        Self::Participant {
            participant_id: participant_id.into(),
        }
    }

    #[must_use]
    pub fn battle_participant(
        battle_id: impl Into<String>,
        participant_id: impl Into<String>,
    ) -> Self {
        Self::BattleParticipant {
            battle_id: battle_id.into(),
            participant_id: participant_id.into(),
        }
    }

    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Public => "public".to_string(),
            Self::Participant { participant_id } => format!("participant:{participant_id}"),
            Self::BattleParticipant {
                battle_id,
                participant_id,
            } => format!("battle:{battle_id}:participant:{participant_id}"),
        }
    }

    #[must_use]
    pub fn can_read_key(&self, audience_key: &str) -> bool {
        audience_key == "public" || audience_key == self.key()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameEventDraft {
    pub session_id: String,
    pub command_id: Option<String>,
    pub actor_participant_id: Option<String>,
    pub turn_number: u32,
    pub event_key: String,
    pub audience: EventAudience,
    pub event_type: String,
    pub subject_kind: Option<String>,
    pub subject_id_text: Option<String>,
    pub payload_json: String,
}

impl GameEventDraft {
    #[must_use]
    pub fn public(
        session_id: impl Into<String>,
        command_id: Option<String>,
        turn_number: u32,
        event_key: impl Into<String>,
        event_type: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            command_id,
            actor_participant_id: None,
            turn_number,
            event_key: event_key.into(),
            audience: EventAudience::Public,
            event_type: event_type.into(),
            subject_kind: None,
            subject_id_text: None,
            payload_json: payload_json.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameEventRecord {
    pub id: String,
    pub session_id: String,
    pub command_id: Option<String>,
    pub actor_participant_id: Option<String>,
    pub turn_number: u32,
    pub event_seq: u64,
    pub event_key: String,
    pub audience_key: String,
    pub event_type: String,
    pub subject_kind: Option<String>,
    pub subject_id_text: Option<String>,
    pub payload_json: String,
}

impl GameEventRecord {
    #[must_use]
    pub fn view_for(&self, audience: Option<&EventAudience>) -> EventView {
        let allowed = self.audience_key == "public"
            || audience.is_some_and(|audience| audience.can_read_key(&self.audience_key));
        EventView {
            session_id: self.session_id.clone(),
            event_seq: self.event_seq,
            event_key: self.event_key.clone(),
            audience_key: self.audience_key.clone(),
            turn_number: self.turn_number,
            event_type: self.event_type.clone(),
            subject_kind: self.subject_kind.clone(),
            subject_id_text: self.subject_id_text.clone(),
            payload_json: allowed.then(|| self.payload_json.clone()),
            redacted: !allowed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EventView {
    pub session_id: String,
    pub event_seq: u64,
    pub event_key: String,
    pub audience_key: String,
    pub turn_number: u32,
    pub event_type: String,
    pub subject_kind: Option<String>,
    pub subject_id_text: Option<String>,
    pub payload_json: Option<String>,
    pub redacted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<EventView>,
    pub next_event_seq: Option<u64>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct GameEventTurnSummaryRecord {
    pub session_id: String,
    pub audience_key: String,
    pub turn_number: u32,
    pub first_event_seq: u64,
    pub last_event_seq: u64,
    pub event_count: u32,
    pub summary_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandStatusView {
    pub command_id: String,
    pub status: CommandStatus,
    pub phase: CommandPhase,
    pub retryable: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub result_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandSubmitOutcome {
    pub command: GameCommandRecord,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LobbyCommandSubmitOutcome {
    pub command: LobbyCommandRecord,
    pub duplicate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EventAppendOutcome {
    pub event: GameEventRecord,
    pub duplicate: bool,
    pub appended: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RecoveryBudget {
    pub inspect_commands: usize,
    pub advance_commands: usize,
    pub command_effects: usize,
    pub game_events: usize,
    pub gameplay_rows: usize,
}

impl Default for RecoveryBudget {
    fn default() -> Self {
        Self {
            inspect_commands: RECOVERY_INSPECT_LIMIT,
            advance_commands: RECOVERY_ADVANCE_LIMIT,
            command_effects: RECOVERY_COMMAND_EFFECT_LIMIT,
            game_events: RECOVERY_GAME_EVENT_LIMIT,
            gameplay_rows: RECOVERY_GAMEPLAY_ROW_LIMIT,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RecoveryOutcome {
    pub inspected_commands: usize,
    pub advanced_commands: usize,
    pub effects_applied: usize,
    pub events_appended: usize,
    pub rows_mutated: usize,
    pub pending_after: usize,
    pub budget_exhausted: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandCoreError {
    #[error("command session mismatch: expected {expected}, got {actual}")]
    SessionMismatch { expected: String, actual: String },
    #[error("duplicate nonce {client_nonce} has different payload hash")]
    DuplicateNoncePayloadMismatch {
        client_nonce: u64,
        existing_payload_hash: String,
        submitted_payload_hash: String,
    },
    #[error("lobby duplicate nonce {client_nonce} has different payload hash")]
    LobbyDuplicateNoncePayloadMismatch {
        client_nonce: u64,
        existing_payload_hash: String,
        submitted_payload_hash: String,
    },
    #[error("command not found: {command_id}")]
    CommandNotFound { command_id: String },
    #[error("command {command_id} effect limit exceeded")]
    CommandEffectLimitExceeded { command_id: String },
    #[error("command {command_id} event limit exceeded")]
    CommandEventLimitExceeded { command_id: String },
    #[error("command effect mismatch for {command_id}/{effect_key}")]
    CommandEffectMismatch {
        command_id: String,
        effect_key: String,
    },
    #[error("pending effect mismatch for {session_id}/{effect_key}")]
    PendingEffectMismatch {
        session_id: String,
        effect_key: String,
    },
}

#[derive(Clone, Debug)]
pub struct SessionCommandJournal {
    session_id: String,
    next_command_ordinal: u64,
    next_effect_ordinal: u64,
    next_pending_effect_ordinal: u64,
    next_event_ordinal: u64,
    next_event_seq: u64,
    clock_start_ms: u64,
    commands: Vec<GameCommandRecord>,
    effects: Vec<CommandEffectRecord>,
    pending_effects: Vec<PendingEffectRecord>,
    events: Vec<GameEventRecord>,
}

impl SessionCommandJournal {
    #[must_use]
    pub fn new(session_id: impl Into<String>, clock_start_ms: u64) -> Self {
        Self::with_next_event_seq(session_id, clock_start_ms, 1)
    }

    #[must_use]
    pub fn with_next_event_seq(
        session_id: impl Into<String>,
        clock_start_ms: u64,
        next_event_seq: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            next_command_ordinal: 1,
            next_effect_ordinal: 1,
            next_pending_effect_ordinal: 1,
            next_event_ordinal: 1,
            next_event_seq,
            clock_start_ms,
            commands: Vec::new(),
            effects: Vec::new(),
            pending_effects: Vec::new(),
            events: Vec::new(),
        }
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn next_event_seq(&self) -> u64 {
        self.next_event_seq
    }

    #[must_use]
    pub fn commands(&self) -> &[GameCommandRecord] {
        &self.commands
    }

    #[must_use]
    pub fn effects(&self) -> &[CommandEffectRecord] {
        &self.effects
    }

    #[must_use]
    pub fn pending_effects(&self) -> &[PendingEffectRecord] {
        &self.pending_effects
    }

    #[must_use]
    pub fn events(&self) -> &[GameEventRecord] {
        &self.events
    }

    pub fn submit_command(
        &mut self,
        payload: GameCommandPayload,
    ) -> Result<CommandSubmitOutcome, CommandCoreError> {
        self.ensure_session(&payload.session_id)?;
        let submitted_payload_hash = payload.payload_hash();

        if let Some(existing) = self.commands.iter().find(|command| {
            command.session_id == payload.session_id
                && command.actor_kind == payload.actor.kind
                && command.actor_id_text == payload.actor.actor_id_text
                && command.client_nonce == payload.client_nonce
        }) {
            if existing.payload_hash != submitted_payload_hash {
                return Err(CommandCoreError::DuplicateNoncePayloadMismatch {
                    client_nonce: payload.client_nonce,
                    existing_payload_hash: existing.payload_hash.clone(),
                    submitted_payload_hash,
                });
            }

            return Ok(CommandSubmitOutcome {
                command: existing.clone(),
                duplicate: true,
            });
        }

        let id = self.allocate_command_id();
        let command = GameCommandRecord {
            id,
            session_id: payload.session_id,
            actor_kind: payload.actor.kind,
            actor_id_text: payload.actor.actor_id_text,
            actor_player_id: payload.actor.actor_player_id,
            actor_participant_id: payload.actor.actor_participant_id,
            champion_id: payload.actor.champion_id,
            turn_number: payload.turn_number,
            client_nonce: payload.client_nonce,
            command_type: payload.command_type,
            status: CommandStatus::Pending,
            phase: CommandPhase::Created,
            payload_hash: submitted_payload_hash,
            payload_json: payload.payload_json,
            result_json: None,
            error_code: None,
            error_message: None,
            error_details_json: None,
            retryable: false,
            created_at_ms: self.synthetic_time_ms(),
            applied_at_ms: None,
            failed_at_ms: None,
        };
        self.commands.push(command.clone());

        Ok(CommandSubmitOutcome {
            command,
            duplicate: false,
        })
    }

    pub fn begin_apply(&mut self, command_id: &str) -> Result<CommandStatusView, CommandCoreError> {
        self.set_command_applying(command_id, CommandPhase::Applying)
    }

    pub fn mark_command_applied(
        &mut self,
        command_id: &str,
        result_json: Option<String>,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let applied_at_ms = self.synthetic_time_ms();
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::Applied;
        command.phase = CommandPhase::Complete;
        command.result_json = result_json;
        command.applied_at_ms = Some(applied_at_ms);
        command.retryable = false;
        Ok(command.status_view())
    }

    pub fn mark_command_failed(
        &mut self,
        command_id: &str,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        retryable: bool,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let failed_at_ms = self.synthetic_time_ms();
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::Failed;
        command.phase = CommandPhase::Failed;
        command.error_code = Some(error_code.into());
        command.error_message = Some(error_message.into());
        command.retryable = retryable;
        command.failed_at_ms = Some(failed_at_ms);
        Ok(command.status_view())
    }

    pub fn mark_command_superseded(
        &mut self,
        command_id: &str,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::Superseded;
        command.phase = CommandPhase::Complete;
        Ok(command.status_view())
    }

    pub fn mark_command_applied_noop(
        &mut self,
        command_id: &str,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let applied_at_ms = self.synthetic_time_ms();
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::AppliedNoop;
        command.phase = CommandPhase::Complete;
        command.applied_at_ms = Some(applied_at_ms);
        Ok(command.status_view())
    }

    pub fn command_status(&self, command_id: &str) -> Result<CommandStatusView, CommandCoreError> {
        Ok(self.command(command_id)?.status_view())
    }

    pub fn ensure_command_effect(
        &mut self,
        command_id: &str,
        effect_key: impl Into<String>,
        effect_type: impl Into<String>,
        target_kind: impl Into<String>,
        target_id_text: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Result<CommandEffectRecord, CommandCoreError> {
        let session_id = self.command(command_id)?.session_id.clone();
        let effect_key = effect_key.into();
        let effect_type = effect_type.into();
        let target_kind = target_kind.into();
        let target_id_text = target_id_text.into();
        let payload_json = payload_json.into();

        if let Some(existing) = self
            .effects
            .iter()
            .find(|effect| effect.command_id == command_id && effect.effect_key == effect_key)
        {
            if existing.effect_type != effect_type
                || existing.target_kind != target_kind
                || existing.target_id_text != target_id_text
                || existing.payload_json != payload_json
            {
                return Err(CommandCoreError::CommandEffectMismatch {
                    command_id: command_id.to_string(),
                    effect_key,
                });
            }

            return Ok(existing.clone());
        }

        if self
            .effects
            .iter()
            .filter(|effect| effect.command_id == command_id)
            .count()
            >= COMMAND_EFFECT_LIMIT
        {
            return Err(CommandCoreError::CommandEffectLimitExceeded {
                command_id: command_id.to_string(),
            });
        }

        let effect = CommandEffectRecord {
            id: self.allocate_effect_id(),
            session_id,
            command_id: command_id.to_string(),
            effect_key,
            effect_type,
            target_kind,
            target_id_text,
            status: EffectStatus::Pending,
            payload_json,
            applied_at_ms: None,
        };
        self.effects.push(effect.clone());
        Ok(effect)
    }

    pub fn mark_command_effect_applied(
        &mut self,
        command_id: &str,
        effect_key: &str,
    ) -> Result<CommandEffectRecord, CommandCoreError> {
        let applied_at_ms = self.synthetic_time_ms();
        let effect = self
            .effects
            .iter_mut()
            .find(|effect| effect.command_id == command_id && effect.effect_key == effect_key)
            .ok_or_else(|| CommandCoreError::CommandEffectMismatch {
                command_id: command_id.to_string(),
                effect_key: effect_key.to_string(),
            })?;
        effect.status = EffectStatus::Applied;
        effect.applied_at_ms = Some(applied_at_ms);
        Ok(effect.clone())
    }

    pub fn ensure_pending_effect(
        &mut self,
        draft: PendingEffectDraft,
    ) -> Result<PendingEffectRecord, CommandCoreError> {
        self.ensure_session(&draft.session_id)?;

        if let Some(existing) = self.pending_effects.iter().find(|effect| {
            effect.session_id == draft.session_id && effect.effect_key == draft.effect_key
        }) {
            if existing.source_command_id != draft.source_command_id
                || existing.target_participant_id != draft.target_participant_id
                || existing.target_champion_id != draft.target_champion_id
                || existing.due_turn != draft.due_turn
                || existing.effect_type != draft.effect_type
                || existing.payload_json != draft.payload_json
            {
                return Err(CommandCoreError::PendingEffectMismatch {
                    session_id: draft.session_id,
                    effect_key: draft.effect_key,
                });
            }

            return Ok(existing.clone());
        }

        let pending = PendingEffectRecord {
            id: self.allocate_pending_effect_id(),
            session_id: draft.session_id,
            source_command_id: draft.source_command_id,
            target_participant_id: draft.target_participant_id,
            target_champion_id: draft.target_champion_id,
            effect_key: draft.effect_key,
            due_turn: draft.due_turn,
            effect_type: draft.effect_type,
            status: EffectStatus::Pending,
            payload_json: draft.payload_json,
            applied_at_ms: None,
        };
        self.pending_effects.push(pending.clone());
        Ok(pending)
    }

    pub fn append_event(
        &mut self,
        draft: GameEventDraft,
    ) -> Result<EventAppendOutcome, CommandCoreError> {
        self.ensure_session(&draft.session_id)?;

        if let Some(existing) = self
            .events
            .iter()
            .find(|event| {
                event.session_id == draft.session_id && event.event_key == draft.event_key
            })
            .cloned()
        {
            self.bump_event_cursor_past(existing.event_seq);
            return Ok(EventAppendOutcome {
                event: existing,
                duplicate: true,
                appended: false,
            });
        }

        if let Some(command_id) = draft.command_id.as_deref() {
            self.command(command_id)?;
            if self
                .events
                .iter()
                .filter(|event| event.command_id.as_deref() == Some(command_id))
                .count()
                >= COMMAND_EVENT_LIMIT
            {
                return Err(CommandCoreError::CommandEventLimitExceeded {
                    command_id: command_id.to_string(),
                });
            }
        }

        self.skip_colliding_event_sequences();
        let event_seq = self.next_event_seq;
        self.next_event_seq += 1;

        let event = GameEventRecord {
            id: self.allocate_event_id(),
            session_id: draft.session_id,
            command_id: draft.command_id,
            actor_participant_id: draft.actor_participant_id,
            turn_number: draft.turn_number,
            event_seq,
            event_key: draft.event_key,
            audience_key: draft.audience.key(),
            event_type: draft.event_type,
            subject_kind: draft.subject_kind,
            subject_id_text: draft.subject_id_text,
            payload_json: draft.payload_json,
        };
        self.events.push(event.clone());

        Ok(EventAppendOutcome {
            event,
            duplicate: false,
            appended: true,
        })
    }

    #[must_use]
    pub fn event_views_after_seq(
        &self,
        events_after_seq: u64,
        audience: Option<&EventAudience>,
        limit: usize,
    ) -> Vec<EventView> {
        self.event_page_after_seq(events_after_seq, audience, limit)
            .events
    }

    #[must_use]
    pub fn event_page_after_seq(
        &self,
        events_after_seq: u64,
        audience: Option<&EventAudience>,
        limit: usize,
    ) -> EventPage {
        let limit = limit.max(1);
        let mut events = self
            .events
            .iter()
            .filter(|event| event.event_seq > events_after_seq)
            .collect::<Vec<_>>();
        events.sort_by_key(|event| event.event_seq);
        let has_more = events.len() > limit;

        let events = events
            .into_iter()
            .take(limit)
            .map(|event| event.view_for(audience))
            .collect::<Vec<_>>();
        let next_event_seq = has_more
            .then(|| events.last().map(|event| event.event_seq))
            .flatten();

        EventPage {
            events,
            next_event_seq,
            has_more,
        }
    }

    #[must_use]
    pub fn summarize_turn(
        &self,
        turn_number: u32,
        audience: &EventAudience,
    ) -> Option<GameEventTurnSummaryRecord> {
        let audience_key = audience.key();
        let mut visible = self
            .events
            .iter()
            .filter(|event| {
                event.turn_number == turn_number
                    && (event.audience_key == "public" || event.audience_key == audience_key)
            })
            .collect::<Vec<_>>();
        visible.sort_by_key(|event| event.event_seq);

        let first = visible.first()?;
        let last = visible.last()?;
        let event_types = visible
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>()
            .join(",");

        Some(GameEventTurnSummaryRecord {
            session_id: self.session_id.clone(),
            audience_key,
            turn_number,
            first_event_seq: first.event_seq,
            last_event_seq: last.event_seq,
            event_count: visible.len() as u32,
            summary_json: format!(
                "{{\"event_count\":{},\"event_types\":\"{}\"}}",
                visible.len(),
                escape_json(&event_types)
            ),
        })
    }

    pub fn recover_pending_or_applying(
        &mut self,
        budget: RecoveryBudget,
    ) -> Result<RecoveryOutcome, CommandCoreError> {
        let recoverable_count = self.recoverable_command_count();
        let recoverable_ids = self
            .commands
            .iter()
            .filter(|command| command.status.is_recoverable())
            .take(budget.inspect_commands)
            .map(|command| command.id.clone())
            .collect::<Vec<_>>();

        let mut outcome = RecoveryOutcome {
            inspected_commands: recoverable_ids.len(),
            budget_exhausted: recoverable_count > recoverable_ids.len(),
            ..RecoveryOutcome::default()
        };

        for command_id in recoverable_ids.into_iter().take(budget.advance_commands) {
            if outcome.advanced_commands >= budget.advance_commands {
                outcome.budget_exhausted = true;
                break;
            }

            let command = self.command(&command_id)?.clone();
            if !command.status.is_recoverable() {
                continue;
            }

            let effect_key = recovery_effect_key(&command_id);
            let event_key = recovery_event_key(&command_id);
            let needs_effect = self
                .effects
                .iter()
                .find(|effect| effect.command_id == command_id && effect.effect_key == effect_key)
                .map_or(true, |effect| effect.status != EffectStatus::Applied);
            let needs_event = !self.events.iter().any(|event| {
                event.session_id == command.session_id && event.event_key == event_key
            });

            if needs_effect && outcome.effects_applied >= budget.command_effects {
                outcome.budget_exhausted = true;
                break;
            }
            if needs_event && outcome.events_appended >= budget.game_events {
                outcome.budget_exhausted = true;
                break;
            }
            if outcome.rows_mutated >= budget.gameplay_rows {
                outcome.budget_exhausted = true;
                break;
            }

            self.set_command_applying(&command_id, CommandPhase::Recovered)?;
            outcome.rows_mutated += 1;

            let effect = self.ensure_command_effect(
                &command_id,
                effect_key.clone(),
                "command_recovery",
                "command",
                command_id.clone(),
                "{}",
            )?;
            if effect.status != EffectStatus::Applied {
                self.mark_command_effect_applied(&command_id, &effect_key)?;
                outcome.effects_applied += 1;
                outcome.rows_mutated += 1;
            }

            let event = self.append_event(GameEventDraft::public(
                command.session_id,
                Some(command_id.clone()),
                command.turn_number,
                event_key,
                "command_recovered",
                format!("{{\"command_id\":\"{}\"}}", escape_json(&command_id)),
            ))?;
            if event.appended {
                outcome.events_appended += 1;
                outcome.rows_mutated += 1;
            }

            self.mark_command_applied(&command_id, Some("{\"recovered\":true}".to_string()))?;
            outcome.rows_mutated += 1;
            outcome.advanced_commands += 1;
        }

        outcome.pending_after = self.recoverable_command_count();
        if outcome.pending_after > 0 {
            outcome.budget_exhausted = true;
        }
        Ok(outcome)
    }

    fn ensure_session(&self, actual: &str) -> Result<(), CommandCoreError> {
        if self.session_id == actual {
            Ok(())
        } else {
            Err(CommandCoreError::SessionMismatch {
                expected: self.session_id.clone(),
                actual: actual.to_string(),
            })
        }
    }

    fn set_command_applying(
        &mut self,
        command_id: &str,
        phase: CommandPhase,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let command = self.command_mut(command_id)?;
        if command.status.is_recoverable() {
            command.status = CommandStatus::Applying;
            command.phase = phase;
        }
        Ok(command.status_view())
    }

    fn command(&self, command_id: &str) -> Result<&GameCommandRecord, CommandCoreError> {
        self.commands
            .iter()
            .find(|command| command.id == command_id)
            .ok_or_else(|| CommandCoreError::CommandNotFound {
                command_id: command_id.to_string(),
            })
    }

    fn command_mut(
        &mut self,
        command_id: &str,
    ) -> Result<&mut GameCommandRecord, CommandCoreError> {
        self.commands
            .iter_mut()
            .find(|command| command.id == command_id)
            .ok_or_else(|| CommandCoreError::CommandNotFound {
                command_id: command_id.to_string(),
            })
    }

    fn recoverable_command_count(&self) -> usize {
        self.commands
            .iter()
            .filter(|command| command.status.is_recoverable())
            .count()
    }

    fn skip_colliding_event_sequences(&mut self) {
        while self.events.iter().any(|event| {
            event.session_id == self.session_id && event.event_seq == self.next_event_seq
        }) {
            self.next_event_seq += 1;
        }
    }

    fn bump_event_cursor_past(&mut self, event_seq: u64) {
        self.next_event_seq = self.next_event_seq.max(event_seq.saturating_add(1));
    }

    fn allocate_command_id(&mut self) -> String {
        let id = format!("cmd:{:06}", self.next_command_ordinal);
        self.next_command_ordinal += 1;
        id
    }

    fn allocate_effect_id(&mut self) -> String {
        let id = format!("effect:{:06}", self.next_effect_ordinal);
        self.next_effect_ordinal += 1;
        id
    }

    fn allocate_pending_effect_id(&mut self) -> String {
        let id = format!("pending-effect:{:06}", self.next_pending_effect_ordinal);
        self.next_pending_effect_ordinal += 1;
        id
    }

    fn allocate_event_id(&mut self) -> String {
        let id = format!("event:{:06}", self.next_event_ordinal);
        self.next_event_ordinal += 1;
        id
    }

    fn synthetic_time_ms(&self) -> u64 {
        self.clock_start_ms
            + self.commands.len() as u64
            + self.effects.len() as u64
            + self.pending_effects.len() as u64
            + self.events.len() as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PendingEffectDraft {
    pub session_id: String,
    pub source_command_id: Option<String>,
    pub target_participant_id: Option<String>,
    pub target_champion_id: Option<String>,
    pub effect_key: String,
    pub due_turn: u32,
    pub effect_type: String,
    pub payload_json: String,
}

impl PendingEffectDraft {
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        effect_key: impl Into<String>,
        due_turn: u32,
        effect_type: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            source_command_id: None,
            target_participant_id: None,
            target_champion_id: None,
            effect_key: effect_key.into(),
            due_turn,
            effect_type: effect_type.into(),
            payload_json: payload_json.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LobbyCommandJournal {
    next_command_ordinal: u64,
    clock_start_ms: u64,
    commands: Vec<LobbyCommandRecord>,
}

impl LobbyCommandJournal {
    #[must_use]
    pub fn new(clock_start_ms: u64) -> Self {
        Self {
            next_command_ordinal: 1,
            clock_start_ms,
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn commands(&self) -> &[LobbyCommandRecord] {
        &self.commands
    }

    pub fn submit_command(
        &mut self,
        payload: LobbyCommandPayload,
    ) -> Result<LobbyCommandSubmitOutcome, CommandCoreError> {
        let submitted_payload_hash = payload.payload_hash();
        if let Some(existing) = self.commands.iter().find(|command| {
            command.actor_principal == payload.actor_principal
                && command.client_nonce == payload.client_nonce
        }) {
            if existing.payload_hash != submitted_payload_hash {
                return Err(CommandCoreError::LobbyDuplicateNoncePayloadMismatch {
                    client_nonce: payload.client_nonce,
                    existing_payload_hash: existing.payload_hash.clone(),
                    submitted_payload_hash,
                });
            }

            return Ok(LobbyCommandSubmitOutcome {
                command: existing.clone(),
                duplicate: true,
            });
        }

        let command = LobbyCommandRecord {
            id: self.allocate_command_id(),
            actor_principal: payload.actor_principal,
            actor_player_id: payload.actor_player_id,
            client_nonce: payload.client_nonce,
            payload_hash: submitted_payload_hash,
            command_type: payload.command_type,
            status: CommandStatus::Pending,
            phase: CommandPhase::Created,
            payload_json: payload.payload_json,
            result_json: None,
            error_code: None,
            error_message: None,
            error_details_json: None,
            retryable: false,
            created_at_ms: self.synthetic_time_ms(),
            applied_at_ms: None,
            failed_at_ms: None,
        };
        self.commands.push(command.clone());
        Ok(LobbyCommandSubmitOutcome {
            command,
            duplicate: false,
        })
    }

    pub fn mark_command_applied(
        &mut self,
        command_id: &str,
        result_json: Option<String>,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let applied_at_ms = self.synthetic_time_ms();
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::Applied;
        command.phase = CommandPhase::Complete;
        command.result_json = result_json;
        command.retryable = false;
        command.applied_at_ms = Some(applied_at_ms);
        Ok(command.status_view())
    }

    pub fn mark_command_failed(
        &mut self,
        command_id: &str,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        retryable: bool,
    ) -> Result<CommandStatusView, CommandCoreError> {
        let failed_at_ms = self.synthetic_time_ms();
        let command = self.command_mut(command_id)?;
        command.status = CommandStatus::Failed;
        command.phase = CommandPhase::Failed;
        command.error_code = Some(error_code.into());
        command.error_message = Some(error_message.into());
        command.retryable = retryable;
        command.failed_at_ms = Some(failed_at_ms);
        Ok(command.status_view())
    }

    pub fn command_status(&self, command_id: &str) -> Result<CommandStatusView, CommandCoreError> {
        Ok(self.command(command_id)?.status_view())
    }

    fn command(&self, command_id: &str) -> Result<&LobbyCommandRecord, CommandCoreError> {
        self.commands
            .iter()
            .find(|command| command.id == command_id)
            .ok_or_else(|| CommandCoreError::CommandNotFound {
                command_id: command_id.to_string(),
            })
    }

    fn command_mut(
        &mut self,
        command_id: &str,
    ) -> Result<&mut LobbyCommandRecord, CommandCoreError> {
        self.commands
            .iter_mut()
            .find(|command| command.id == command_id)
            .ok_or_else(|| CommandCoreError::CommandNotFound {
                command_id: command_id.to_string(),
            })
    }

    fn allocate_command_id(&mut self) -> String {
        let id = format!("lobby-cmd:{:06}", self.next_command_ordinal);
        self.next_command_ordinal += 1;
        id
    }

    fn synthetic_time_ms(&self) -> u64 {
        self.clock_start_ms + self.commands.len() as u64
    }
}

#[must_use]
pub fn recovery_effect_key(command_id: &str) -> String {
    format!("recover:{command_id}")
}

#[must_use]
pub fn recovery_event_key(command_id: &str) -> String {
    format!("{command_id}:recovered")
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hash_bytes(hasher, label, value.as_bytes());
}

fn hash_optional_text(hasher: &mut Sha256, label: &str, value: Option<&str>) {
    match value {
        Some(value) => hash_text(hasher, label, value),
        None => hash_text(hasher, label, "<none>"),
    }
}

fn hash_bytes(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update(b":");
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value);
    hasher.update(b"\n");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        CommandActor, CommandCoreError, CommandPhase, CommandStatus, EffectStatus, EventAudience,
        GameCommandPayload, GameEventDraft, GameEventRecord, LobbyCommandJournal,
        LobbyCommandPayload, PendingEffectDraft, RecoveryBudget, SessionCommandJournal,
        recovery_effect_key, recovery_event_key,
    };
    use crate::fixtures::first_playable_fixture;

    fn journal() -> SessionCommandJournal {
        let fixture = first_playable_fixture();
        SessionCommandJournal::new(fixture.ids.session_id, fixture.clock.start_timestamp_ms)
    }

    fn player_actor() -> CommandActor {
        let fixture = first_playable_fixture();
        CommandActor::player(
            fixture.ids.player_one_id,
            fixture.ids.participant_one_id,
            Some("fixture-champion-one".to_string()),
        )
    }

    fn payload(nonce: u64, body: &str) -> GameCommandPayload {
        let fixture = first_playable_fixture();
        GameCommandPayload::from_json(
            fixture.ids.session_id,
            player_actor(),
            1,
            nonce,
            "submit_move_intent",
            body,
        )
    }

    #[test]
    fn game_command_dedupe_returns_existing_command_for_exact_retry() {
        let mut journal = journal();
        let first = journal
            .submit_command(payload(7, "{\"path\":[[1,1],[1,2]]}"))
            .expect("first command should insert");
        let retry = journal
            .submit_command(payload(7, "{\"path\":[[1,1],[1,2]]}"))
            .expect("exact retry should load existing command");

        assert!(!first.duplicate);
        assert!(retry.duplicate);
        assert_eq!(first.command.id, retry.command.id);
        assert_eq!(first.command.payload_hash, retry.command.payload_hash);
        assert_eq!(journal.commands().len(), 1);
        assert_eq!(first.command.payload_hash.len(), 64);
    }

    #[test]
    fn game_command_rejects_same_nonce_with_different_payload_hash() {
        let mut journal = journal();
        journal
            .submit_command(payload(7, "{\"path\":[[1,1],[1,2]]}"))
            .expect("first command should insert");

        let error = journal
            .submit_command(payload(7, "{\"path\":[[1,1],[2,2]]}"))
            .expect_err("same nonce with different typed payload should fail");

        assert!(matches!(
            error,
            CommandCoreError::DuplicateNoncePayloadMismatch {
                client_nonce: 7,
                ..
            }
        ));
        assert_eq!(journal.commands().len(), 1);
    }

    #[test]
    fn lobby_command_uses_principal_nonce_idempotency() {
        let fixture = first_playable_fixture();
        let mut journal = LobbyCommandJournal::new(fixture.clock.start_timestamp_ms);
        let payload = LobbyCommandPayload::from_json(
            fixture.principals.player_one,
            None,
            11,
            "create_session",
            "{\"scenario\":\"first\"}",
        );

        let first = journal
            .submit_command(payload.clone())
            .expect("first lobby command should insert");
        let retry = journal
            .submit_command(payload)
            .expect("identical lobby retry should load existing command");
        let mismatch = LobbyCommandPayload::from_json(
            fixture.principals.player_one,
            None,
            11,
            "create_session",
            "{\"scenario\":\"other\"}",
        );

        assert!(!first.duplicate);
        assert!(retry.duplicate);
        assert_eq!(first.command.id, retry.command.id);
        assert!(matches!(
            journal.submit_command(mismatch),
            Err(CommandCoreError::LobbyDuplicateNoncePayloadMismatch {
                client_nonce: 11,
                ..
            })
        ));
        assert_eq!(journal.commands().len(), 1);
    }

    #[test]
    fn recovery_finishes_pending_effect_event_and_marks_command_applied() {
        let mut journal = journal();
        let command = journal
            .submit_command(payload(7, "{\"path\":[[1,1],[1,2]]}"))
            .expect("command should insert")
            .command;
        journal
            .begin_apply(&command.id)
            .expect("command can enter applying");
        journal
            .ensure_command_effect(
                &command.id,
                recovery_effect_key(&command.id),
                "command_recovery",
                "command",
                command.id.clone(),
                "{}",
            )
            .expect("pending recovery effect can exist before recovery");

        let outcome = journal
            .recover_pending_or_applying(RecoveryBudget::default())
            .expect("recovery should complete");
        let status = journal
            .command_status(&command.id)
            .expect("status should be readable");
        let effect = journal
            .effects()
            .iter()
            .find(|effect| {
                effect.command_id == command.id
                    && effect.effect_key == recovery_effect_key(&command.id)
            })
            .expect("recovery effect should exist");
        let event = journal
            .events()
            .iter()
            .find(|event| event.event_key == recovery_event_key(&command.id))
            .expect("recovery event should exist");

        assert_eq!(outcome.advanced_commands, 1);
        assert_eq!(outcome.effects_applied, 1);
        assert_eq!(outcome.events_appended, 1);
        assert!(!outcome.budget_exhausted);
        assert_eq!(status.status, CommandStatus::Applied);
        assert_eq!(status.phase, CommandPhase::Complete);
        assert_eq!(effect.status, EffectStatus::Applied);
        assert_eq!(event.event_seq, 1);
    }

    #[test]
    fn event_key_idempotency_reuses_existing_sequence() {
        let mut journal = journal();
        let fixture = first_playable_fixture();
        let first = journal
            .append_event(GameEventDraft::public(
                fixture.ids.session_id.clone(),
                None,
                1,
                "session_started",
                "session_started",
                "{\"state\":\"active\"}",
            ))
            .expect("first event should append");
        let duplicate = journal
            .append_event(GameEventDraft::public(
                fixture.ids.session_id,
                None,
                1,
                "session_started",
                "session_started",
                "{\"state\":\"active\"}",
            ))
            .expect("duplicate event key should return existing row");

        assert_eq!(first.event.event_seq, 1);
        assert!(first.appended);
        assert!(duplicate.duplicate);
        assert!(!duplicate.appended);
        assert_eq!(duplicate.event.event_seq, first.event.event_seq);
        assert_eq!(journal.events().len(), 1);
        assert_eq!(journal.next_event_seq(), 2);
    }

    #[test]
    fn event_sequence_allocator_skips_collisions_and_allows_gaps() {
        let mut journal = journal();
        let fixture = first_playable_fixture();
        journal.events.push(GameEventRecord {
            id: "manual-event".to_string(),
            session_id: fixture.ids.session_id.clone(),
            command_id: None,
            actor_participant_id: None,
            turn_number: 1,
            event_seq: 1,
            event_key: "manual-prior-event".to_string(),
            audience_key: "public".to_string(),
            event_type: "manual".to_string(),
            subject_kind: None,
            subject_id_text: None,
            payload_json: "{}".to_string(),
        });
        journal.next_event_seq = 1;

        let repaired = journal
            .append_event(GameEventDraft::public(
                fixture.ids.session_id.clone(),
                None,
                1,
                "after-collision",
                "repair",
                "{}",
            ))
            .expect("allocator should skip occupied event seq");
        journal.next_event_seq = 5;
        let after_gap = journal
            .append_event(GameEventDraft::public(
                fixture.ids.session_id,
                None,
                1,
                "after-gap",
                "gap",
                "{}",
            ))
            .expect("allocator may leave gaps after recovery");

        let mut sequences = journal
            .events()
            .iter()
            .map(|event| event.event_seq)
            .collect::<Vec<_>>();
        sequences.sort_unstable();
        sequences.dedup();

        assert_eq!(repaired.event.event_seq, 2);
        assert_eq!(after_gap.event.event_seq, 5);
        assert_eq!(sequences.len(), journal.events().len());
        assert_eq!(journal.next_event_seq(), 6);
    }

    #[test]
    fn recovery_budget_exhaustion_persists_progress_and_reports_pending_work() {
        let mut journal = journal();
        for nonce in 1..=9 {
            journal
                .submit_command(payload(nonce, &format!("{{\"nonce\":{nonce}}}")))
                .expect("command should insert");
        }

        let outcome = journal
            .recover_pending_or_applying(RecoveryBudget::default())
            .expect("recovery should make bounded progress");
        let applied = journal
            .commands()
            .iter()
            .filter(|command| command.status == CommandStatus::Applied)
            .count();

        assert_eq!(outcome.inspected_commands, 9);
        assert_eq!(outcome.advanced_commands, 8);
        assert_eq!(outcome.pending_after, 1);
        assert!(outcome.budget_exhausted);
        assert_eq!(applied, 8);
    }

    #[test]
    fn audience_redaction_hides_private_payloads_from_wrong_reader() {
        let mut journal = journal();
        let fixture = first_playable_fixture();
        let mut private_event = GameEventDraft::public(
            fixture.ids.session_id.clone(),
            None,
            1,
            "private-scouting",
            "object_spotted",
            "{\"hidden\":\"enemy-town\"}",
        );
        private_event.audience = EventAudience::participant(fixture.ids.participant_one_id.clone());
        private_event.subject_kind = Some("world_object".to_string());
        private_event.subject_id_text = Some("hidden-object".to_string());
        journal
            .append_event(private_event)
            .expect("private event should append");

        let event = &journal.events()[0];
        let wrong = event.view_for(Some(&EventAudience::participant(
            fixture.ids.participant_two_id.clone(),
        )));
        let right = event.view_for(Some(&EventAudience::participant(
            fixture.ids.participant_one_id.clone(),
        )));
        let summary = journal
            .summarize_turn(
                1,
                &EventAudience::participant(fixture.ids.participant_one_id),
            )
            .expect("participant summary should include visible private event");

        assert!(wrong.redacted);
        assert_eq!(wrong.payload_json, None);
        assert!(!right.redacted);
        assert_eq!(
            right.payload_json,
            Some("{\"hidden\":\"enemy-town\"}".to_string())
        );
        assert_eq!(summary.first_event_seq, 1);
        assert_eq!(summary.last_event_seq, 1);
        assert_eq!(summary.event_count, 1);
    }

    #[test]
    fn status_reads_expose_retryable_errors_without_recovery_writes() {
        let mut journal = journal();
        let command = journal
            .submit_command(payload(7, "{\"path\":[[1,1],[1,2]]}"))
            .expect("command should insert")
            .command;
        journal
            .mark_command_failed(&command.id, "transient_busy", "retry later", true)
            .expect("command can fail retryably");

        let before_effects = journal.effects().len();
        let before_events = journal.events().len();
        let status = journal
            .command_status(&command.id)
            .expect("status query should not mutate journal");
        let recovery = journal
            .recover_pending_or_applying(RecoveryBudget::default())
            .expect("failed commands should not be recovered");

        assert_eq!(status.status, CommandStatus::Failed);
        assert!(status.retryable);
        assert_eq!(status.error_code, Some("transient_busy".to_string()));
        assert_eq!(journal.effects().len(), before_effects);
        assert_eq!(journal.events().len(), before_events);
        assert_eq!(recovery.advanced_commands, 0);
    }

    #[test]
    fn pending_effects_are_idempotent_by_session_effect_key() {
        let mut journal = journal();
        let fixture = first_playable_fixture();
        let mut draft = PendingEffectDraft::new(
            fixture.ids.session_id.clone(),
            "income:turn:2:participant-one",
            2,
            "income_tick",
            "{\"gold\":1000}",
        );
        draft.target_participant_id = Some(fixture.ids.participant_one_id);

        let first = journal
            .ensure_pending_effect(draft.clone())
            .expect("pending effect should insert");
        let retry = journal
            .ensure_pending_effect(draft)
            .expect("identical pending effect should reuse existing row");

        assert_eq!(first.id, retry.id);
        assert_eq!(journal.pending_effects().len(), 1);
    }
}
