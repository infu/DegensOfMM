//! Heap-resident active battle runtime state.
//!
//! This module is intentionally structural first. Endpoint code will move onto
//! it in the next checkpoints; until then these types define the aggregate that
//! active battle commands will mutate instead of tactical child rows.

#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use candid::CandidType;
use domm_game::{ApiEventView, BattleState, CommandResponse, CommandStatusView};
use serde::{Deserialize, Serialize};

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
}
