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

use candid::CandidType;
use canic_cdk::structures::{
    Cell as StableCell, DefaultMemoryImpl, Storable, memory::VirtualMemory, storable::Bound,
};
use domm_degens_schema::schema::{Battle, GameSession};
use domm_game::{ApiEventView, BattleState, CommandResponse, CommandStatusView};
use icydb::traits::EntityValue;
use serde::{Deserialize, Serialize};

use super::battle_rows;

pub(crate) const BATTLE_RUNTIME_MEMORY_ID: u8 = 23;
const MAX_BATTLE_RUNTIME_SNAPSHOT_BYTES: u32 = 32 * 1024 * 1024;

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

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntime {
    pub session_id: String,
    pub battle_id: String,
    pub state: BattleState,
    pub participant_audience_keys: BTreeMap<String, BattleRuntimeAudience>,
    pub command_receipts: BTreeMap<String, BattleRuntimeCommandReceipt>,
    pub command_receipts_by_nonce: BTreeMap<BattleRuntimeNonceKey, String>,
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
            command_receipts: BTreeMap::new(),
            command_receipts_by_nonce: BTreeMap::new(),
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
        self.command_receipts_by_nonce.insert(
            BattleRuntimeNonceKey {
                actor_participant_id: receipt.actor_participant_id.clone(),
                client_nonce: receipt.client_nonce,
            },
            receipt.command_id.clone(),
        );
        self.command_receipts
            .insert(receipt.command_id.clone(), receipt);
        self.mark_dirty();
    }

    pub(crate) fn command_receipt_by_nonce(
        &self,
        actor_participant_id: &str,
        client_nonce: u64,
    ) -> Option<&BattleRuntimeCommandReceipt> {
        let key = BattleRuntimeNonceKey {
            actor_participant_id: actor_participant_id.to_string(),
            client_nonce,
        };
        let command_id = self.command_receipts_by_nonce.get(&key)?;
        self.command_receipts.get(command_id)
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

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeCommandReceipt {
    pub command_id: String,
    pub command_type: String,
    pub actor_participant_id: String,
    pub client_nonce_text: String,
    pub client_nonce: u64,
    pub payload_hash: String,
    pub response: CommandResponse,
}

impl BattleRuntimeCommandReceipt {
    pub(crate) fn status_view(&self) -> CommandStatusView {
        self.response.status_view()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeNonceKey {
    pub actor_participant_id: String,
    pub client_nonce: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeEvent {
    pub command_id: Option<String>,
    pub event: ApiEventView,
    pub flushed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeReadyKey {
    pub participant_id: String,
    pub round_number: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeDeadline {
    pub action_deadline_at_ms: Option<u64>,
    pub timeout_job_key: Option<String>,
    pub round_job_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub(crate) struct BattleRuntimeSnapshot {
    pub runtimes: Vec<BattleRuntime>,
}

thread_local! {
    static ACTIVE_BATTLE_RUNTIMES: RefCell<BTreeMap<String, BattleRuntime>> =
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
    ACTIVE_BATTLE_RUNTIMES.with(|runtimes| runtimes.borrow_mut().remove(battle_id))
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
}

pub(crate) fn persist_snapshot_for_upgrade() -> Result<(), String> {
    let snapshot = snapshot_for_upgrade();
    let bytes = candid::encode_one(snapshot)
        .map_err(|error| format!("battle runtime Candid encode failed: {error}"))?;
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

    let snapshot = candid::decode_one::<BattleRuntimeSnapshot>(raw.as_bytes())
        .map_err(|error| format!("battle runtime Candid decode failed: {error}"))?;
    restore_from_upgrade(snapshot);
    BATTLE_RUNTIME_SNAPSHOT_CELL.with(|cell| {
        cell.borrow_mut().set(RawBattleRuntimeSnapshot::empty());
    });
    Ok(())
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
    fn runtime_snapshot_candid_round_trips() {
        let snapshot = BattleRuntimeSnapshot {
            runtimes: vec![BattleRuntime::new(
                "session:1",
                "battle:1",
                empty_state("s1", "b1"),
                42,
            )],
        };

        let bytes = candid::encode_one(&snapshot).expect("snapshot should encode");
        let decoded: BattleRuntimeSnapshot =
            candid::decode_one(&bytes).expect("snapshot should decode");

        assert_eq!(decoded, snapshot);
    }
}
