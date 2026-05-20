//! Heap-resident active battle runtime state.
//!
//! This module is intentionally structural first. Endpoint code will move onto
//! it in the next checkpoints; until then these types define the aggregate that
//! active battle commands will mutate instead of tactical child rows.

#![allow(dead_code)]

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use canic_cdk::structures::{
    Cell as StableCell, DefaultMemoryImpl, Storable, memory::VirtualMemory, storable::Bound,
};
use domm_degens_schema::schema::{
    Battle, BattleObstacle, BattleOccupancy, BattleStack, GameSession,
};
#[cfg(not(feature = "benchmark"))]
use domm_degens_schema::schema::{GameCommand, GameParticipant};
use domm_game::{ApiError, ApiEventView, BattleState, CommandResponse, CommandStatusView};
#[cfg(not(feature = "benchmark"))]
use icydb::types::Timestamp;
use icydb::{
    traits::{EntityKey, EntityValue},
    types::{Id, Ulid},
};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "benchmark"))]
use crate::repos::commands_events_effects;
use crate::repos::{battles as battle_repo, sessions as session_repo};

use super::battle_rows;

pub(crate) const BATTLE_RUNTIME_MEMORY_ID: u8 = 23;
pub(crate) const BATTLE_RUNTIME_EVENT_SEQ_BLOCK_SIZE: u64 = 4_096;
const MAX_BATTLE_RUNTIME_SNAPSHOT_BYTES: u32 = 32 * 1024 * 1024;
const UPGRADE_REFS_MAGIC: &str = "DOMM_BATTLE_RUNTIME_REFS_V1";

struct BattleRuntimeStableCell;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RawBattleRuntimeSnapshot(Vec<u8>);

impl RawBattleRuntimeSnapshot {
    const fn empty() -> Self {
        Self(Vec::new())
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl Storable for RawBattleRuntimeSnapshot {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.as_bytes())
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self(bytes.into_owned())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_BATTLE_RUNTIME_SNAPSHOT_BYTES,
        is_fixed_size: false,
    };
}

thread_local! {
    static BATTLE_RUNTIME_SNAPSHOT_CELL: RefCell<
        StableCell<RawBattleRuntimeSnapshot, VirtualMemory<DefaultMemoryImpl>>,
    > = RefCell::new(StableCell::init(
        icydb::__reexports::canic_memory::ic_memory!(
            BattleRuntimeStableCell,
            BATTLE_RUNTIME_MEMORY_ID
        ),
        RawBattleRuntimeSnapshot::empty(),
    ));
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntime {
    pub session_id: String,
    pub battle_id: String,
    pub state: BattleState,
    pub participant_audience_keys: BTreeMap<String, BattleRuntimeAudience>,
    pub command_receipts: Vec<BattleRuntimeCommandReceipt>,
    pub active_events: Vec<BattleRuntimeEvent>,
    pub ready_participants: BTreeSet<BattleRuntimeReadyKey>,
    pub deadline: BattleRuntimeDeadline,
    pub session_event_sequence_cursor: u64,
    pub dirty_generation: u64,
}

impl BattleRuntime {
    pub(crate) fn new(
        session_id: impl Into<String>,
        battle_id: impl Into<String>,
        state: BattleState,
        session_event_sequence_cursor: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            battle_id: battle_id.into(),
            state,
            participant_audience_keys: BTreeMap::new(),
            command_receipts: Vec::new(),
            active_events: Vec::new(),
            ready_participants: BTreeSet::new(),
            deadline: BattleRuntimeDeadline::default(),
            session_event_sequence_cursor,
            dirty_generation: 0,
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty_generation = self.dirty_generation.saturating_add(1);
    }

    pub(crate) fn insert_audience(
        &mut self,
        participant_id: impl Into<String>,
        audience: BattleRuntimeAudience,
    ) {
        self.participant_audience_keys
            .insert(participant_id.into(), audience);
        self.mark_dirty();
    }

    pub(crate) fn insert_command_receipt(&mut self, receipt: BattleRuntimeCommandReceipt) {
        if let Some(existing) = self
            .command_receipts
            .iter_mut()
            .find(|existing| existing.command_id == receipt.command_id)
        {
            *existing = receipt;
        } else {
            self.command_receipts.push(receipt);
        }
        self.mark_dirty();
    }

    pub(crate) fn command_receipt_by_nonce(
        &self,
        actor_participant_id: &str,
        client_nonce: u64,
    ) -> Option<&BattleRuntimeCommandReceipt> {
        self.command_receipts.iter().find(|receipt| {
            receipt.actor_participant_id == actor_participant_id
                && receipt.client_nonce == client_nonce
        })
    }

    pub(crate) fn push_event(&mut self, event: BattleRuntimeEvent) {
        self.active_events.push(event);
        self.mark_dirty();
    }

    pub(crate) fn mark_ready(&mut self, participant_id: impl Into<String>, round_number: u16) {
        self.ready_participants.insert(BattleRuntimeReadyKey {
            participant_id: participant_id.into(),
            round_number,
        });
        self.mark_dirty();
    }
}

pub(crate) fn build_runtime_from_loaded_state(
    session: &GameSession,
    battle: &Battle,
    state: BattleState,
) -> BattleRuntime {
    let deadline_ms = battle
        .action_deadline_at
        .and_then(|deadline| u64::try_from(deadline.as_millis()).ok());
    let mut runtime = BattleRuntime::new(
        session.id().to_string(),
        battle.id().to_string(),
        state,
        session.next_event_seq,
    );
    runtime.deadline = BattleRuntimeDeadline {
        action_deadline_at_ms: deadline_ms,
        timeout_job_key: deadline_ms
            .map(|deadline| format!("battle_timeout:{}:{deadline}", battle.id())),
        round_job_key: None,
    };
    hydrate_participant_audience_keys(&mut runtime);
    runtime
}

pub(crate) fn build_runtime_from_state(
    session: &GameSession,
    state: BattleState,
) -> Result<BattleRuntime, ApiError> {
    let battle = state
        .battles
        .first()
        .ok_or_else(|| ApiError::new("battle_not_found", "battle runtime state is empty", true))?;
    let battle_id = battle.battle_id.clone();
    let deadline_ms = battle.action_deadline_at;
    let mut runtime = BattleRuntime::new(
        session.id().to_string(),
        battle_id.clone(),
        state,
        session.next_event_seq,
    );
    runtime.deadline = BattleRuntimeDeadline {
        action_deadline_at_ms: deadline_ms,
        timeout_job_key: deadline_ms
            .map(|deadline| format!("battle_timeout:{battle_id}:{deadline}")),
        round_job_key: None,
    };
    hydrate_participant_audience_keys(&mut runtime);
    Ok(runtime)
}

pub(crate) fn hydrate_runtime_from_rows(
    session: &GameSession,
    battle: Battle,
) -> Result<BattleRuntime, domm_game::ApiError> {
    let state = battle_rows::load_battle_state_from_row(session, battle.clone())?;
    Ok(build_runtime_from_loaded_state(session, &battle, state))
}

pub(crate) fn adopt_active_battle_from_rows(
    session: &GameSession,
    battle: Battle,
) -> Result<bool, domm_game::ApiError> {
    if battle.state != "active" {
        return Ok(false);
    }
    let battle_id = battle.id().to_string();
    if contains_runtime(&battle_id) {
        return Ok(false);
    }
    let runtime = hydrate_runtime_from_rows(session, battle)?;
    insert_runtime(runtime);
    Ok(true)
}

pub(crate) fn adopt_active_battle_from_rows_with_stacks(
    session: &GameSession,
    battle: Battle,
    stacks: Vec<BattleStack>,
) -> Result<bool, domm_game::ApiError> {
    if battle.state != "active" {
        return Ok(false);
    }
    let battle_id = battle.id().to_string();
    if contains_runtime(&battle_id) {
        return Ok(false);
    }
    let state =
        battle_rows::load_battle_state_from_row_with_stacks(session, battle.clone(), stacks)?;
    let runtime = build_runtime_from_loaded_state(session, &battle, state);
    insert_runtime(runtime);
    Ok(true)
}

pub(crate) fn adopt_active_battle_from_loaded_rows(
    session: &GameSession,
    battle: Battle,
    stacks: Vec<BattleStack>,
    obstacles: Vec<BattleObstacle>,
    occupancy: Vec<BattleOccupancy>,
) -> Result<bool, domm_game::ApiError> {
    if battle.state != "active" {
        return Ok(false);
    }
    let battle_id = battle.id().to_string();
    if contains_runtime(&battle_id) {
        return Ok(false);
    }
    let state = battle_rows::battle_state_from_loaded_tactical_rows(
        session,
        battle.clone(),
        stacks,
        obstacles,
        occupancy,
    );
    let runtime = build_runtime_from_loaded_state(session, &battle, state);
    insert_runtime(runtime);
    Ok(true)
}

pub(crate) fn replace_runtime_from_state(
    session: &GameSession,
    state: BattleState,
) -> Result<bool, ApiError> {
    let Some(battle) = state.battles.first() else {
        return Ok(false);
    };
    let battle_id = battle.battle_id.clone();
    if battle.state != "active" {
        remove_runtime(&battle_id);
        return Ok(false);
    }

    let runtime = build_runtime_from_state(session, state)?;
    insert_runtime(runtime);
    Ok(true)
}

fn hydrate_participant_audience_keys(runtime: &mut BattleRuntime) {
    let participant_ids = runtime
        .state
        .stacks
        .iter()
        .filter_map(|stack| stack.owner_participant_id.clone())
        .collect::<BTreeSet<_>>();
    for participant_id in participant_ids {
        runtime
            .participant_audience_keys
            .entry(participant_id.clone())
            .or_insert_with(|| BattleRuntimeAudience::participant(participant_id));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeAudience {
    pub participant_key: String,
    pub player_key: Option<String>,
    pub public_key: String,
}

impl BattleRuntimeAudience {
    pub(crate) fn participant(participant_id: impl Into<String>) -> Self {
        let participant_id = participant_id.into();
        Self {
            participant_key: format!("participant:{participant_id}"),
            player_key: None,
            public_key: "public".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeCommandReceipt {
    pub command_id: String,
    pub command_type: String,
    pub actor_participant_id: String,
    pub client_nonce_text: String,
    pub client_nonce: u64,
    pub payload_hash: String,
    #[cfg(not(feature = "benchmark"))]
    pub payload_json: Option<String>,
    pub response: CommandResponse,
}

impl BattleRuntimeCommandReceipt {
    pub(crate) fn status_view(&self) -> CommandStatusView {
        self.response.status_view()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeEvent {
    pub command_id: Option<String>,
    pub event: ApiEventView,
    pub flushed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeReadyKey {
    pub participant_id: String,
    pub round_number: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeDeadline {
    pub action_deadline_at_ms: Option<u64>,
    pub timeout_job_key: Option<String>,
    pub round_job_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeSnapshot {
    pub runtimes: Vec<BattleRuntime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BattleRuntimeUpgradeRef {
    session_id: String,
    battle_id: String,
    session_event_sequence_cursor: u64,
    dirty_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BattleRuntimeEventSeqBlock {
    pub next_event_seq: u64,
    pub exclusive_end_event_seq: u64,
}

impl BattleRuntimeEventSeqBlock {
    fn take_event_seq(&mut self) -> Option<u64> {
        if self.next_event_seq >= self.exclusive_end_event_seq {
            return None;
        }
        let event_seq = self.next_event_seq;
        self.next_event_seq = self.next_event_seq.saturating_add(1);
        Some(event_seq)
    }
}

thread_local! {
    static ACTIVE_BATTLE_RUNTIMES: RefCell<BTreeMap<String, BattleRuntime>> =
        RefCell::new(BTreeMap::new());
    static ACTIVE_SESSION_EVENT_SEQ_BLOCKS: RefCell<BTreeMap<String, BattleRuntimeEventSeqBlock>> =
        RefCell::new(BTreeMap::new());
    static ARCHIVED_SESSION_RUNTIME_EVENTS: RefCell<BTreeMap<String, Vec<BattleRuntimeEvent>>> =
        RefCell::new(BTreeMap::new());
    static ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS:
        RefCell<BTreeMap<String, Vec<BattleRuntimeCommandReceipt>>> =
            RefCell::new(BTreeMap::new());
}

pub(crate) fn contains_runtime(battle_id: &str) -> bool {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(battle_id))
}

pub(crate) fn active_runtime_count() -> usize {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| runtimes.borrow().len())
}

pub(crate) fn insert_runtime(runtime: BattleRuntime) -> Option<BattleRuntime> {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow_mut()
            .insert(runtime.battle_id.clone(), runtime)
    })
}

pub(crate) fn remove_runtime(battle_id: &str) -> Option<BattleRuntime> {
    let removed = ACTIVE_BATTLE_RUNTIMES.with(|runtimes| runtimes.borrow_mut().remove(battle_id));
    if let Some(runtime) = removed.as_ref() {
        archive_runtime_events(runtime);
        archive_runtime_command_receipts(runtime);
    }
    removed
}

pub(crate) fn reserve_session_event_seq(session: &mut GameSession) -> Result<u64, ApiError> {
    let session_id = session.id().to_string();
    if let Some(event_seq) = take_reserved_session_event_seq(&session_id) {
        return Ok(event_seq);
    }

    let start = session.next_event_seq;
    let end = start
        .checked_add(BATTLE_RUNTIME_EVENT_SEQ_BLOCK_SIZE)
        .ok_or_else(|| {
            ApiError::new(
                "event_sequence_exhausted",
                "session event sequence cannot reserve another active battle block",
                true,
            )
        })?;
    let mut updated = session.clone();
    updated.next_event_seq = end;
    *session = session_repo::update_session(updated)?;
    ACTIVE_SESSION_EVENT_SEQ_BLOCKS.with(|blocks| {
        blocks.borrow_mut().insert(
            session_id,
            BattleRuntimeEventSeqBlock {
                next_event_seq: start.saturating_add(1),
                exclusive_end_event_seq: end,
            },
        );
    });
    Ok(start)
}

fn take_reserved_session_event_seq(session_id: &str) -> Option<u64> {
    ACTIVE_SESSION_EVENT_SEQ_BLOCKS.with(|blocks| {
        let mut blocks = blocks.borrow_mut();
        let block = blocks.get_mut(session_id)?;
        block.take_event_seq()
    })
}

pub(crate) fn with_runtime<R>(
    battle_id: &str,
    read: impl FnOnce(&BattleRuntime) -> R,
) -> Option<R> {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        let runtimes = runtimes.borrow();
        runtimes.get(battle_id).map(read)
    })
}

pub(crate) fn with_runtime_mut<R>(
    battle_id: &str,
    mutate: impl FnOnce(&mut BattleRuntime) -> R,
) -> Option<R> {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.get_mut(battle_id).map(mutate)
    })
}

pub(crate) fn active_events_after(
    session_id: &str,
    audience_key: &str,
    events_after_seq: u64,
) -> Vec<ApiEventView> {
    let mut events = ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .flat_map(|runtime| runtime.active_events.iter())
            .filter(|runtime_event| !runtime_event.flushed)
            .map(|runtime_event| &runtime_event.event)
            .filter(|event| {
                event.audience_key == audience_key && event.event_seq > events_after_seq
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    ARCHIVED_SESSION_RUNTIME_EVENTS.with(|archived| {
        if let Some(archived_events) = archived.borrow().get(session_id) {
            events.extend(
                archived_events
                    .iter()
                    .filter(|runtime_event| !runtime_event.flushed)
                    .map(|runtime_event| &runtime_event.event)
                    .filter(|event| {
                        event.audience_key == audience_key && event.event_seq > events_after_seq
                    })
                    .cloned(),
            );
        }
    });
    events
}

pub(crate) fn command_receipt_by_id(
    session_id: &str,
    command_id: &str,
) -> Option<BattleRuntimeCommandReceipt> {
    if let Some(receipt) = ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .find_map(|runtime| {
                runtime
                    .command_receipts
                    .iter()
                    .find(|receipt| receipt.command_id == command_id)
                    .cloned()
            })
    }) {
        return Some(receipt);
    }

    ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS.with(|archived| {
        archived
            .borrow()
            .get(session_id)?
            .iter()
            .find(|receipt| receipt.command_id == command_id)
            .cloned()
    })
}

pub(crate) fn command_receipt_by_nonce(
    session_id: &str,
    actor_participant_id: &str,
    client_nonce: u64,
) -> Option<BattleRuntimeCommandReceipt> {
    if let Some(receipt) = ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .find_map(|runtime| {
                runtime
                    .command_receipt_by_nonce(actor_participant_id, client_nonce)
                    .cloned()
            })
    }) {
        return Some(receipt);
    }

    ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS.with(|archived| {
        archived
            .borrow()
            .get(session_id)?
            .iter()
            .find(|receipt| {
                receipt.actor_participant_id == actor_participant_id
                    && receipt.client_nonce == client_nonce
            })
            .cloned()
    })
}

pub(crate) fn archive_runtime_events(runtime: &BattleRuntime) {
    let events = runtime
        .active_events
        .iter()
        .filter(|event| !event.flushed)
        .cloned()
        .collect::<Vec<_>>();
    if events.is_empty() {
        return;
    }
    ARCHIVED_SESSION_RUNTIME_EVENTS.with(|archived| {
        let mut archived = archived.borrow_mut();
        let session_events = archived.entry(runtime.session_id.clone()).or_default();
        let mut existing_keys = session_events
            .iter()
            .map(|event| event.event.event_key.clone())
            .collect::<BTreeSet<_>>();
        for event in events {
            if existing_keys.insert(event.event.event_key.clone()) {
                session_events.push(event);
            }
        }
    });
}

pub(crate) fn archive_runtime_command_receipts(runtime: &BattleRuntime) {
    if runtime.command_receipts.is_empty() {
        return;
    }
    ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS.with(|archived| {
        let mut archived = archived.borrow_mut();
        let session_receipts = archived.entry(runtime.session_id.clone()).or_default();
        let mut existing_ids = session_receipts
            .iter()
            .map(|receipt| receipt.command_id.clone())
            .collect::<BTreeSet<_>>();
        for receipt in &runtime.command_receipts {
            if existing_ids.insert(receipt.command_id.clone()) {
                session_receipts.push(receipt.clone());
            }
        }
    });
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn flush_runtime_archives_for_barrier() -> Result<usize, ApiError> {
    let active_runtimes = ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .cloned()
            .collect::<Vec<BattleRuntime>>()
    });
    let archived_events = ARCHIVED_SESSION_RUNTIME_EVENTS.with(|archived| {
        archived
            .borrow()
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
    });
    let archived_receipts = ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS.with(|archived| {
        archived
            .borrow()
            .iter()
            .flat_map(|(session_id, receipts)| {
                receipts
                    .iter()
                    .cloned()
                    .map(|receipt| (session_id.clone(), receipt))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });

    let mut flushed = 0_usize;
    let mut seen_receipts = BTreeSet::<String>::new();
    for runtime in &active_runtimes {
        for receipt in &runtime.command_receipts {
            if seen_receipts.insert(receipt.command_id.clone())
                && flush_runtime_command_receipt(runtime, receipt)?
            {
                flushed = flushed.saturating_add(1);
            }
        }
    }
    for (session_id, receipt) in &archived_receipts {
        if seen_receipts.insert(receipt.command_id.clone())
            && flush_runtime_command_receipt_for_session(session_id, receipt)?
        {
            flushed = flushed.saturating_add(1);
        }
    }

    let mut seen_events = BTreeSet::<String>::new();
    for runtime in &active_runtimes {
        for runtime_event in runtime
            .active_events
            .iter()
            .filter(|runtime_event| !runtime_event.flushed)
        {
            if seen_events.insert(runtime_event.event.event_key.clone())
                && flush_runtime_event(runtime_event)?
            {
                flushed = flushed.saturating_add(1);
            }
        }
    }
    for runtime_event in &archived_events {
        if seen_events.insert(runtime_event.event.event_key.clone())
            && flush_runtime_event(runtime_event)?
        {
            flushed = flushed.saturating_add(1);
        }
    }

    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        for runtime in runtimes.borrow_mut().values_mut() {
            for runtime_event in &mut runtime.active_events {
                runtime_event.flushed = true;
            }
        }
    });
    Ok(flushed)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_command_receipt(
    runtime: &BattleRuntime,
    receipt: &BattleRuntimeCommandReceipt,
) -> Result<bool, ApiError> {
    flush_runtime_command_receipt_for_session(&runtime.session_id, receipt)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_command_receipt_for_session(
    session_id_text: &str,
    receipt: &BattleRuntimeCommandReceipt,
) -> Result<bool, ApiError> {
    let Some(payload_json) = receipt.payload_json.clone() else {
        return Ok(false);
    };
    let session_id = parse_ulid_id::<GameSession>(session_id_text)?;
    let command_id = parse_ulid_id::<GameCommand>(&receipt.command_id)?;
    let actor_participant_id = parse_ulid_id::<GameParticipant>(&receipt.actor_participant_id)?;
    if commands_events_effects::load_game_command(command_id)?.is_some()
        || commands_events_effects::find_game_command_by_idempotency(
            session_id,
            "participant",
            &receipt.actor_participant_id,
            receipt.client_nonce,
        )?
        .is_some()
    {
        return Ok(false);
    }
    let now = Timestamp::now();
    let error = receipt.response.error.clone();
    let command = GameCommand {
        id: command_id.key(),
        session_id: session_id.key(),
        actor_kind: "participant".to_string(),
        actor_id_text: receipt.actor_participant_id.clone(),
        actor_player_id: None,
        actor_participant_id: Some(actor_participant_id.key()),
        champion_id: None,
        turn_number: receipt.response.durable_turn,
        client_nonce: receipt.client_nonce,
        command_type: receipt.command_type.clone(),
        status: receipt.response.status.as_str().to_string(),
        phase: receipt.response.phase.as_str().to_string(),
        payload_hash: receipt.payload_hash.clone(),
        payload_json,
        result_json: Some(format!(
            r#"{{"runtime_flushed":true,"command_id":"{}"}}"#,
            receipt.command_id
        )),
        error_code: error.as_ref().map(|error| error.code.clone()),
        error_message: error.as_ref().map(|error| error.message.clone()),
        error_details_json: error.and_then(|error| error.details_json),
        retryable: receipt.response.retryable,
        applied_at: (receipt.response.status == domm_game::CommandStatus::Applied).then_some(now),
        failed_at: receipt.response.error.as_ref().map(|_| now),
        ..Default::default()
    };
    commands_events_effects::insert_game_command(command)?;
    Ok(true)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_event(runtime_event: &BattleRuntimeEvent) -> Result<bool, ApiError> {
    let session_id = parse_ulid_id::<GameSession>(&runtime_event.event.session_id)?;
    if commands_events_effects::find_event_by_key(session_id, &runtime_event.event.event_key)?
        .is_some()
    {
        return Ok(false);
    }
    let command_id = durable_command_id(runtime_event.command_id.as_deref())?;
    commands_events_effects::create_game_event(
        session_id,
        command_id,
        None,
        runtime_event.event.turn_number,
        runtime_event.event.event_seq,
        runtime_event.event.event_key.clone(),
        runtime_event.event.audience_key.clone(),
        runtime_event.event.event_type.clone(),
        runtime_event.event.subject_kind.clone(),
        runtime_event.event.subject_id_text.clone(),
        runtime_event
            .event
            .payload
            .clone()
            .unwrap_or_else(|| "{}".to_string()),
    )?;
    Ok(true)
}

#[cfg(not(feature = "benchmark"))]
fn durable_command_id(command_id_text: Option<&str>) -> Result<Option<Id<GameCommand>>, ApiError> {
    let Some(command_id_text) = command_id_text else {
        return Ok(None);
    };
    let Ok(command_id) = try_parse_ulid_id::<GameCommand>(command_id_text) else {
        return Ok(None);
    };
    if commands_events_effects::load_game_command(command_id)?.is_some() {
        Ok(Some(command_id))
    } else {
        Ok(None)
    }
}

#[cfg(not(feature = "benchmark"))]
fn parse_ulid_id<E>(value: &str) -> Result<Id<E>, ApiError>
where
    E: EntityKey<Key = Ulid>,
{
    try_parse_ulid_id(value).map_err(|_| {
        ApiError::new(
            "invalid_battle_runtime_archive_id",
            "battle runtime archive contains an invalid id",
            true,
        )
    })
}

#[cfg(not(feature = "benchmark"))]
fn try_parse_ulid_id<E>(value: &str) -> Result<Id<E>, ()>
where
    E: EntityKey<Key = Ulid>,
{
    Ulid::from_str(value).map(Id::from_key).map_err(|_| ())
}

pub(crate) fn snapshot_for_upgrade() -> BattleRuntimeSnapshot {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| BattleRuntimeSnapshot {
        runtimes: runtimes.borrow().values().cloned().collect(),
    })
}

pub(crate) fn restore_from_upgrade(snapshot: BattleRuntimeSnapshot) {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.clear();
        for runtime in snapshot.runtimes {
            runtimes.insert(runtime.battle_id.clone(), runtime);
        }
    });
}

pub(crate) fn clear_all_for_tests() {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| runtimes.borrow_mut().clear());
    ACTIVE_SESSION_EVENT_SEQ_BLOCKS.with(|blocks| blocks.borrow_mut().clear());
    ARCHIVED_SESSION_RUNTIME_EVENTS.with(|events| events.borrow_mut().clear());
    ARCHIVED_SESSION_RUNTIME_COMMAND_RECEIPTS.with(|receipts| receipts.borrow_mut().clear());
}

pub(crate) fn persist_snapshot_for_upgrade() -> Result<(), String> {
    let refs = upgrade_refs_for_active_runtimes();
    let bytes = encode_upgrade_refs(&refs);
    if bytes.len() > MAX_BATTLE_RUNTIME_SNAPSHOT_BYTES as usize {
        return Err(format!(
            "battle runtime snapshot exceeds {} bytes: {}",
            MAX_BATTLE_RUNTIME_SNAPSHOT_BYTES,
            bytes.len()
        ));
    }

    BATTLE_RUNTIME_SNAPSHOT_CELL.with(|cell| {
        cell.borrow_mut().set(RawBattleRuntimeSnapshot(bytes));
    });
    Ok(())
}

pub(crate) fn restore_snapshot_after_upgrade() -> Result<(), String> {
    let raw = BATTLE_RUNTIME_SNAPSHOT_CELL.with(|cell| cell.borrow().get().clone());
    if raw.as_bytes().is_empty() {
        return Ok(());
    }

    let refs = decode_upgrade_refs(raw.as_bytes())?;
    for active_ref in refs {
        restore_runtime_from_upgrade_ref(&active_ref)?;
    }
    BATTLE_RUNTIME_SNAPSHOT_CELL.with(|cell| {
        cell.borrow_mut().set(RawBattleRuntimeSnapshot::empty());
    });
    Ok(())
}

fn upgrade_refs_for_active_runtimes() -> Vec<BattleRuntimeUpgradeRef> {
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .map(|runtime| BattleRuntimeUpgradeRef {
                session_id: runtime.session_id.clone(),
                battle_id: runtime.battle_id.clone(),
                session_event_sequence_cursor: runtime.session_event_sequence_cursor,
                dirty_generation: runtime.dirty_generation,
            })
            .collect()
    })
}

fn encode_upgrade_refs(refs: &[BattleRuntimeUpgradeRef]) -> Vec<u8> {
    let mut text = String::new();
    text.push_str(UPGRADE_REFS_MAGIC);
    text.push('\n');
    for active_ref in refs {
        text.push_str(&active_ref.session_id);
        text.push('\t');
        text.push_str(&active_ref.battle_id);
        text.push('\t');
        text.push_str(&active_ref.session_event_sequence_cursor.to_string());
        text.push('\t');
        text.push_str(&active_ref.dirty_generation.to_string());
        text.push('\n');
    }
    text.into_bytes()
}

fn decode_upgrade_refs(bytes: &[u8]) -> Result<Vec<BattleRuntimeUpgradeRef>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("battle runtime snapshot is not UTF-8: {error}"))?;
    let mut lines = text.lines();
    match lines.next() {
        Some(UPGRADE_REFS_MAGIC) => {}
        Some(other) => {
            return Err(format!(
                "battle runtime snapshot magic mismatch: expected {UPGRADE_REFS_MAGIC}, got {other}"
            ));
        }
        None => return Ok(Vec::new()),
    }

    let mut refs = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(format!(
                "battle runtime snapshot line {} has {} fields",
                index + 2,
                fields.len()
            ));
        }
        refs.push(BattleRuntimeUpgradeRef {
            session_id: fields[0].to_string(),
            battle_id: fields[1].to_string(),
            session_event_sequence_cursor: fields[2].parse().map_err(|error| {
                format!(
                    "battle runtime snapshot line {} invalid event cursor: {error}",
                    index + 2
                )
            })?,
            dirty_generation: fields[3].parse().map_err(|error| {
                format!(
                    "battle runtime snapshot line {} invalid dirty generation: {error}",
                    index + 2
                )
            })?,
        });
    }
    Ok(refs)
}

fn restore_runtime_from_upgrade_ref(active_ref: &BattleRuntimeUpgradeRef) -> Result<(), String> {
    let session_id = parse_text_id::<GameSession>(&active_ref.session_id, "session_id")?;
    let battle_id = parse_text_id::<Battle>(&active_ref.battle_id, "battle_id")?;
    let Some(session) = session_repo::load_session(session_id)
        .map_err(|error| format!("load session for runtime restore failed: {}", error.message))?
    else {
        return Ok(());
    };
    let Some(battle) = battle_repo::load_battle(battle_id)
        .map_err(|error| format!("load battle for runtime restore failed: {}", error.message))?
    else {
        return Ok(());
    };
    if battle.state != "active" {
        return Ok(());
    }

    let mut runtime = hydrate_runtime_from_rows(&session, battle)
        .map_err(|error| format!("hydrate runtime after upgrade failed: {}", error.message))?;
    runtime.session_event_sequence_cursor = active_ref.session_event_sequence_cursor;
    runtime.dirty_generation = active_ref.dirty_generation;
    insert_runtime(runtime);
    Ok(())
}

fn parse_text_id<E>(value: &str, field: &str) -> Result<Id<E>, String>
where
    E: EntityKey<Key = Ulid>,
{
    Ulid::from_str(value)
        .map(Id::from_key)
        .map_err(|_| format!("battle runtime snapshot has invalid {field}: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state(session_id: &str, battle_id: &str) -> BattleState {
        BattleState {
            session_seed: "seed".to_string(),
            battles: vec![domm_game::BattleRecord {
                battle_id: battle_id.to_string(),
                session_id: session_id.to_string(),
                state: "active".to_string(),
                battle_type: "test".to_string(),
                attacker_champion_id: None,
                defender_champion_id: None,
                defender_town_id: None,
                defender_neutral_army_id: None,
                current_round: 1,
                active_side: domm_game::BATTLE_SIDE_ATTACKER.to_string(),
                active_stack_id: None,
                grid_width: domm_game::BATTLE_GRID_WIDTH,
                grid_height: domm_game::BATTLE_GRID_HEIGHT,
                max_rounds: domm_game::BATTLE_MAX_ROUNDS,
                turn_seed: 1,
                winner_participant_id: None,
                created_turn: 1,
                action_deadline_at: None,
                resolved_at: None,
                cleanup_after_turn: 0,
                last_command_id: None,
            }],
            stacks: Vec::new(),
            obstacles: Vec::new(),
            occupancy: Vec::new(),
            commands: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn runtime_store_round_trips_by_battle_id() {
        clear_all_for_tests();
        let runtime = BattleRuntime::new("session:1", "battle:1", empty_state("s1", "b1"), 12);

        assert_eq!(active_runtime_count(), 0);
        assert!(insert_runtime(runtime).is_none());
        assert!(contains_runtime("battle:1"));
        assert_eq!(
            with_runtime("battle:1", |runtime| runtime.session_event_sequence_cursor),
            Some(12)
        );

        with_runtime_mut("battle:1", |runtime| runtime.mark_ready("participant:1", 2));
        assert_eq!(
            with_runtime("battle:1", |runtime| runtime.ready_participants.len()),
            Some(1)
        );
        assert!(remove_runtime("battle:1").is_some());
        assert_eq!(active_runtime_count(), 0);
    }

    #[test]
    fn runtime_snapshot_restores_all_active_battles() {
        clear_all_for_tests();
        insert_runtime(BattleRuntime::new(
            "session:1",
            "battle:1",
            empty_state("s1", "b1"),
            12,
        ));
        insert_runtime(BattleRuntime::new(
            "session:1",
            "battle:2",
            empty_state("s1", "b2"),
            18,
        ));

        let snapshot = snapshot_for_upgrade();
        clear_all_for_tests();
        restore_from_upgrade(snapshot);

        assert_eq!(active_runtime_count(), 2);
        assert_eq!(
            with_runtime("battle:2", |runtime| runtime.session_event_sequence_cursor),
            Some(18)
        );
    }

    #[test]
    fn upgrade_refs_round_trip_without_full_runtime_encoding() {
        let refs = vec![BattleRuntimeUpgradeRef {
            session_id: "session:1".to_string(),
            battle_id: "battle:1".to_string(),
            session_event_sequence_cursor: 42,
            dirty_generation: 7,
        }];

        let bytes = encode_upgrade_refs(&refs);
        let decoded = decode_upgrade_refs(&bytes).expect("upgrade refs should decode");

        assert_eq!(decoded, refs);
    }

    #[test]
    fn event_sequence_block_hands_out_reserved_range_only() {
        let mut block = BattleRuntimeEventSeqBlock {
            next_event_seq: 10,
            exclusive_end_event_seq: 12,
        };

        assert_eq!(block.take_event_seq(), Some(10));
        assert_eq!(block.take_event_seq(), Some(11));
        assert_eq!(block.take_event_seq(), None);
    }

    #[test]
    fn clear_all_for_tests_clears_event_sequence_blocks() {
        clear_all_for_tests();
        ACTIVE_SESSION_EVENT_SEQ_BLOCKS.with(|blocks| {
            blocks.borrow_mut().insert(
                "session:1".to_string(),
                BattleRuntimeEventSeqBlock {
                    next_event_seq: 5,
                    exclusive_end_event_seq: 6,
                },
            );
        });

        assert_eq!(take_reserved_session_event_seq("session:1"), Some(5));
        ACTIVE_SESSION_EVENT_SEQ_BLOCKS.with(|blocks| {
            blocks.borrow_mut().insert(
                "session:1".to_string(),
                BattleRuntimeEventSeqBlock {
                    next_event_seq: 5,
                    exclusive_end_event_seq: 6,
                },
            );
        });
        clear_all_for_tests();

        assert_eq!(take_reserved_session_event_seq("session:1"), None);
    }
}
