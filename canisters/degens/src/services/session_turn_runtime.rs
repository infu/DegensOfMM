//! Heap-resident active session-turn runtime state.
//!
//! This is the movement/session-turn counterpart to `BattleRuntime`. It starts
//! as an inert aggregate shell so endpoint code can be moved onto it in small,
//! testable checkpoints.

#![allow(dead_code)]

#[cfg(not(feature = "benchmark"))]
use std::borrow::Cow;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

#[cfg(not(feature = "benchmark"))]
use canic_cdk::structures::{
    Cell as StableCell, DefaultMemoryImpl, Storable, memory::VirtualMemory, storable::Bound,
};
use domm_degens_schema::schema::{
    Champion, GameCommand, GameParticipant, GameSession, MovementIntent, QuestState,
    ScenarioRuleState, WorldObject,
};
#[cfg(not(feature = "benchmark"))]
use domm_degens_schema::schema::{MapObjectDefinition, MapOccupancy, NeutralArmy};
use domm_game::{ApiError, ApiEventView, CommandResponse, CommandStatusView};
#[cfg(not(feature = "benchmark"))]
use icydb::{traits::EntityKey, types::Timestamp};
use icydb::{
    traits::EntityValue,
    types::{Id, Ulid},
};

use crate::repos::{
    champions_artifacts, map_visibility_occupancy, players, sessions, towns, turn_ready,
};
#[cfg(not(feature = "benchmark"))]
use crate::repos::{cleanup, commands_events_effects, economy, movement, scenario_progress};

pub(crate) const SESSION_TURN_RUNTIME_EVENT_SEQ_BLOCK_SIZE: u64 = 4_096;
#[cfg(not(feature = "benchmark"))]
pub(crate) const SESSION_TURN_RUNTIME_MEMORY_ID: u8 = 24;
#[cfg(not(feature = "benchmark"))]
const MAX_SESSION_TURN_RUNTIME_SNAPSHOT_BYTES: u32 = 64 * 1024 * 1024;

#[cfg(not(feature = "benchmark"))]
struct SessionTurnRuntimeStableCell;

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Eq, PartialEq)]
struct RawSessionTurnRuntimeSnapshot(Vec<u8>);

#[cfg(not(feature = "benchmark"))]
impl RawSessionTurnRuntimeSnapshot {
    const fn empty() -> Self {
        Self(Vec::new())
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[cfg(not(feature = "benchmark"))]
impl Storable for RawSessionTurnRuntimeSnapshot {
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
        max_size: MAX_SESSION_TURN_RUNTIME_SNAPSHOT_BYTES,
        is_fixed_size: false,
    };
}

#[cfg(not(feature = "benchmark"))]
thread_local! {
    static SESSION_TURN_RUNTIME_SNAPSHOT_CELL: RefCell<
        StableCell<RawSessionTurnRuntimeSnapshot, VirtualMemory<DefaultMemoryImpl>>,
    > = RefCell::new(StableCell::init(
        icydb::__reexports::canic_memory::ic_memory!(
            SessionTurnRuntimeStableCell,
            SESSION_TURN_RUNTIME_MEMORY_ID
        ),
        RawSessionTurnRuntimeSnapshot::empty(),
    ));
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnRuntime {
    pub session_id: String,
    pub turn_number: u32,
    pub session: Option<GameSession>,
    pub turn_started_at_ms: u64,
    pub turn_deadline_at_ms: u64,
    pub turn_duration_ms: u64,
    pub closing: bool,
    pub generation: u64,
    pub participants_complete: bool,
    pub ready_complete: bool,
    pub participants: Vec<SessionTurnParticipant>,
    pub ready_participants: BTreeSet<String>,
    pub champion_snapshots: Vec<Champion>,
    pub champion_spell_snapshots: Vec<RuntimeChampionSpell>,
    pub complete_champion_spellbooks: BTreeSet<String>,
    pub world_object_snapshots: Vec<WorldObject>,
    pub occupancy_index: Vec<RuntimeOccupancyCell>,
    pub contact_index: Vec<RuntimeContactCell>,
    pub quest_snapshots: Vec<QuestState>,
    pub scenario_rule_snapshots: Vec<ScenarioRuleState>,
    pub intents: Vec<RuntimeMovementIntent>,
    pub command_receipts: Vec<SessionTurnCommandReceipt>,
    pub active_events: Vec<SessionTurnEvent>,
    pub event_seq_block: Option<SessionTurnEventSeqBlock>,
    pub central_objectives_completed: Option<u32>,
    pub object_deltas: Vec<ObjectTurnDelta>,
    pub resource_deltas: Vec<ResourceTurnDelta>,
    pub partial_cursor: Option<MovementCursor>,
    pub dirty: SessionTurnDirtySets,
    #[cfg(not(feature = "benchmark"))]
    pub projection_dirty_queue: Vec<ProjectionDirtyEntry>,
    #[cfg(not(feature = "benchmark"))]
    pub projection_checkpoint: ProjectionCheckpoint,
}

impl SessionTurnRuntime {
    pub(crate) fn new(
        session_id: impl Into<String>,
        turn_number: u32,
        turn_started_at_ms: u64,
        turn_deadline_at_ms: u64,
        turn_duration_ms: u64,
    ) -> Self {
        let session_id = session_id.into();
        #[cfg(not(feature = "benchmark"))]
        let projection_checkpoint =
            ProjectionCheckpoint::new(runtime_key(&session_id, turn_number));
        Self {
            session_id,
            turn_number,
            session: None,
            turn_started_at_ms,
            turn_deadline_at_ms,
            turn_duration_ms,
            closing: false,
            generation: 0,
            participants_complete: false,
            ready_complete: false,
            participants: Vec::new(),
            ready_participants: BTreeSet::new(),
            champion_snapshots: Vec::new(),
            champion_spell_snapshots: Vec::new(),
            complete_champion_spellbooks: BTreeSet::new(),
            world_object_snapshots: Vec::new(),
            occupancy_index: Vec::new(),
            contact_index: Vec::new(),
            quest_snapshots: Vec::new(),
            scenario_rule_snapshots: Vec::new(),
            intents: Vec::new(),
            command_receipts: Vec::new(),
            active_events: Vec::new(),
            event_seq_block: None,
            central_objectives_completed: None,
            object_deltas: Vec::new(),
            resource_deltas: Vec::new(),
            partial_cursor: None,
            dirty: SessionTurnDirtySets::default(),
            #[cfg(not(feature = "benchmark"))]
            projection_dirty_queue: Vec::new(),
            #[cfg(not(feature = "benchmark"))]
            projection_checkpoint,
        }
    }

    pub(crate) fn key(&self) -> String {
        runtime_key(&self.session_id, self.turn_number)
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub(crate) fn upsert_participant(&mut self, participant: SessionTurnParticipant) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = participant.participant_id.clone();
        if let Some(existing) = self
            .participants
            .iter_mut()
            .find(|existing| existing.participant_id == participant.participant_id)
        {
            let principal_text = participant
                .principal_text
                .clone()
                .or_else(|| existing.principal_text.clone());
            *existing = participant;
            existing.principal_text = principal_text;
        } else {
            self.participants.push(participant);
        }
        self.dirty.participants = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Participant,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_champion_snapshot(&mut self, champion: Champion) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = champion.id().to_string();
        if let Some(existing) = self
            .champion_snapshots
            .iter_mut()
            .find(|existing| existing.id() == champion.id())
        {
            *existing = champion;
        } else {
            self.champion_snapshots.push(champion);
        }
        self.dirty.champion_snapshots = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Champion,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_champion_spell_snapshot(&mut self, spell: RuntimeChampionSpell) {
        let champion_id = Id::<Champion>::from_key(spell.champion_id).to_string();
        #[cfg(not(feature = "benchmark"))]
        let projection_key = format!("{}:{}", spell.champion_id, spell.spell_id);
        if let Some(existing) = self.champion_spell_snapshots.iter_mut().find(|existing| {
            existing.champion_id == spell.champion_id && existing.spell_id == spell.spell_id
        }) {
            *existing = spell;
        } else {
            self.champion_spell_snapshots.push(spell);
        }
        self.complete_champion_spellbooks.insert(champion_id);
        self.dirty.champion_spell_snapshots = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::ChampionSpell,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn mark_champion_spellbook_complete(&mut self, champion_id: Id<Champion>) {
        if self
            .complete_champion_spellbooks
            .insert(champion_id.to_string())
        {
            self.dirty.champion_spell_snapshots = true;
            self.mark_dirty();
            #[cfg(not(feature = "benchmark"))]
            self.record_projection_dirty(
                ProjectionEntity::ChampionSpell,
                champion_id.to_string(),
                ProjectionDirtyOp::Upsert,
                PROJECTION_PRIORITY_NORMAL,
            );
        }
    }

    pub(crate) fn upsert_world_object_snapshot(&mut self, object: WorldObject) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = object.id().to_string();
        if let Some(existing) = self
            .world_object_snapshots
            .iter_mut()
            .find(|existing| existing.id() == object.id())
        {
            *existing = object;
        } else {
            self.world_object_snapshots.push(object);
        }
        self.dirty.world_object_snapshots = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::WorldObject,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_occupancy_cell(&mut self, cell: RuntimeOccupancyCell) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = occupancy_projection_key(&cell);
        if let Some(existing) = self.occupancy_index.iter_mut().find(|existing| {
            existing.x == cell.x && existing.y == cell.y && existing.layer == cell.layer
        }) {
            *existing = cell;
        } else {
            self.occupancy_index.push(cell);
        }
        self.dirty.occupancy_index = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Occupancy,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_occupancy_for_occupant(&mut self, cell: RuntimeOccupancyCell) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = occupancy_projection_key(&cell);
        self.occupancy_index.retain(|existing| {
            !(existing.layer == cell.layer
                && (existing.x == cell.x && existing.y == cell.y
                    || existing.occupant_kind == cell.occupant_kind
                        && existing.occupant_id_text == cell.occupant_id_text))
        });
        self.occupancy_index.push(cell);
        self.dirty.occupancy_index = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Occupancy,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn remove_occupancy_for_occupant(&mut self, layer: &str, occupant_id_text: &str) {
        let before = self.occupancy_index.len();
        self.occupancy_index.retain(|existing| {
            !(existing.layer == layer
                && existing.occupant_kind == "champion"
                && existing.occupant_id_text == occupant_id_text)
        });
        if self.occupancy_index.len() != before {
            self.dirty.occupancy_index = true;
            self.mark_dirty();
            #[cfg(not(feature = "benchmark"))]
            self.record_projection_dirty(
                ProjectionEntity::Occupancy,
                format!("{layer}:champion:{occupant_id_text}"),
                ProjectionDirtyOp::Tombstone,
                PROJECTION_PRIORITY_NORMAL,
            );
        }
    }

    pub(crate) fn upsert_contact_cell(&mut self, cell: RuntimeContactCell) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = contact_projection_key(&cell);
        if let Some(existing) = self.contact_index.iter_mut().find(|existing| {
            existing.x == cell.x
                && existing.y == cell.y
                && existing.subject_kind == cell.subject_kind
                && existing.subject_id_text == cell.subject_id_text
        }) {
            *existing = cell;
        } else {
            self.contact_index.push(cell);
        }
        self.dirty.contact_index = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Contact,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_quest_snapshot(&mut self, quest: QuestState) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = quest.id().to_string();
        if let Some(existing) = self
            .quest_snapshots
            .iter_mut()
            .find(|existing| existing.id() == quest.id())
        {
            *existing = quest;
        } else {
            self.quest_snapshots.push(quest);
        }
        self.dirty.quest_snapshots = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Quest,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn upsert_scenario_rule_snapshot(&mut self, rule: ScenarioRuleState) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = rule.id().to_string();
        if let Some(existing) = self
            .scenario_rule_snapshots
            .iter_mut()
            .find(|existing| existing.id() == rule.id())
        {
            *existing = rule;
        } else {
            self.scenario_rule_snapshots.push(rule);
        }
        self.dirty.scenario_rule_snapshots = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::ScenarioRule,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn mark_ready(&mut self, participant_id: impl Into<String>) -> bool {
        #[cfg(feature = "benchmark")]
        {
            let inserted = self.ready_participants.insert(participant_id.into());
            if inserted {
                self.dirty.ready = true;
                self.mark_dirty();
            }
            inserted
        }
        #[cfg(not(feature = "benchmark"))]
        {
            let participant_id = participant_id.into();
            let inserted = self.ready_participants.insert(participant_id.clone());
            if inserted {
                self.dirty.ready = true;
                self.mark_dirty();
                self.record_projection_dirty(
                    ProjectionEntity::Ready,
                    participant_id,
                    ProjectionDirtyOp::Upsert,
                    PROJECTION_PRIORITY_NORMAL,
                );
            }
            inserted
        }
    }

    pub(crate) fn readiness_counts(&self) -> Option<RuntimeReadinessCounts> {
        if !self.participants_complete || !self.ready_complete {
            return None;
        }
        let mut participant_count = 0_usize;
        let mut ready_count = 0_usize;
        for participant in self
            .participants
            .iter()
            .filter(|participant| participant.status == "active")
        {
            participant_count = participant_count.saturating_add(1);
            if self
                .ready_participants
                .contains(&participant.participant_id)
            {
                ready_count = ready_count.saturating_add(1);
            }
        }
        Some(RuntimeReadinessCounts {
            ready_count,
            participant_count,
            all_ready: participant_count != 0 && ready_count >= participant_count,
        })
    }

    pub(crate) fn upsert_intent(&mut self, intent: RuntimeMovementIntent) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = intent.intent_id.clone();
        if let Some(existing) = self
            .intents
            .iter_mut()
            .find(|existing| existing.champion_id == intent.champion_id)
        {
            *existing = intent;
        } else {
            self.intents.push(intent);
        }
        self.dirty.intents = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::MovementIntent,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn insert_command_receipt(&mut self, receipt: SessionTurnCommandReceipt) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = receipt.command_id.clone();
        if let Some(existing) = self
            .command_receipts
            .iter_mut()
            .find(|existing| existing.command_id == receipt.command_id)
        {
            *existing = receipt;
        } else {
            self.command_receipts.push(receipt);
        }
        self.dirty.command_receipts = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::CommandReceipt,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn command_receipt_by_nonce(
        &self,
        actor_participant_id: &str,
        client_nonce: u64,
    ) -> Option<&SessionTurnCommandReceipt> {
        self.command_receipts.iter().find(|receipt| {
            receipt.actor_participant_id == actor_participant_id
                && receipt.client_nonce == client_nonce
        })
    }

    pub(crate) fn push_event(&mut self, event: SessionTurnEvent) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = event.event.event_key.clone();
        self.active_events.push(event);
        self.dirty.events = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::Event,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn push_resource_delta(&mut self, delta: ResourceTurnDelta) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = delta
            .ledger
            .as_ref()
            .map(|ledger| ledger.ledger_key.clone())
            .unwrap_or_else(|| delta.participant_id.clone());
        self.resource_deltas.push(delta);
        self.dirty.resource_deltas = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::ResourceDelta,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    pub(crate) fn push_object_delta(&mut self, delta: ObjectTurnDelta) {
        #[cfg(not(feature = "benchmark"))]
        let projection_key = format!("{}:{}", delta.subject_kind, delta.subject_id);
        self.central_objectives_completed = None;
        self.object_deltas.push(delta);
        self.dirty.object_deltas = true;
        self.mark_dirty();
        #[cfg(not(feature = "benchmark"))]
        self.record_projection_dirty(
            ProjectionEntity::ObjectDelta,
            projection_key,
            ProjectionDirtyOp::Upsert,
            PROJECTION_PRIORITY_NORMAL,
        );
    }

    #[cfg(not(feature = "benchmark"))]
    fn record_projection_dirty(
        &mut self,
        entity: ProjectionEntity,
        key: String,
        op: ProjectionDirtyOp,
        priority: u8,
    ) {
        let now_ms = crate::services::clock::now_ms();
        let kernel_id = self.key();
        if let Some(index) = self.projection_dirty_queue.iter().position(|entry| {
            entry.kernel_id == kernel_id && entry.entity == entity && entry.key == key
        }) {
            let existing = &mut self.projection_dirty_queue[index];
            existing.generation = self.generation;
            existing.op = op;
            existing.priority = existing.priority.min(priority);
            existing.last_dirty_at_ms = now_ms;
            self.projection_checkpoint.pending_entries = self.projection_dirty_queue.len();
            return;
        }
        self.projection_dirty_queue.push(ProjectionDirtyEntry {
            kernel_id,
            generation: self.generation,
            entity,
            key,
            op,
            priority,
            first_dirty_at_ms: now_ms,
            last_dirty_at_ms: now_ms,
        });
        self.projection_checkpoint.pending_entries = self.projection_dirty_queue.len();
    }
}

#[cfg(not(feature = "benchmark"))]
const PROJECTION_PRIORITY_NORMAL: u8 = 50;

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct ProjectionDirtyEntry {
    pub kernel_id: String,
    pub generation: u64,
    pub entity: ProjectionEntity,
    pub key: String,
    pub op: ProjectionDirtyOp,
    pub priority: u8,
    pub first_dirty_at_ms: u64,
    pub last_dirty_at_ms: u64,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) enum ProjectionEntity {
    Session,
    Participant,
    Ready,
    Champion,
    ChampionSpell,
    WorldObject,
    Occupancy,
    Contact,
    MovementIntent,
    CommandReceipt,
    Event,
    ObjectDelta,
    ResourceDelta,
    Quest,
    ScenarioRule,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) enum ProjectionDirtyOp {
    Upsert,
    Tombstone,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct ProjectionCheckpoint {
    pub kernel_id: String,
    pub flushed_generation: u64,
    pub flushed_at_ms: u64,
    pub pending_entries: usize,
}

#[cfg(not(feature = "benchmark"))]
impl ProjectionCheckpoint {
    fn new(kernel_id: String) -> Self {
        Self {
            kernel_id,
            flushed_generation: 0,
            flushed_at_ms: 0,
            pending_entries: 0,
        }
    }
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProjectionFlushLimits {
    pub max_rows: usize,
    pub max_instructions: u64,
    pub max_stable_pages_delta: u64,
}

#[cfg(not(feature = "benchmark"))]
impl ProjectionFlushLimits {
    pub(crate) fn unbounded() -> Self {
        Self {
            max_rows: usize::MAX,
            max_instructions: u64::MAX,
            max_stable_pages_delta: u64::MAX,
        }
    }
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectionFlushOutcome {
    pub entries_processed: usize,
    pub rows_flushed: usize,
    pub queue_len_before: usize,
    pub queue_len_after: usize,
    pub truncated: bool,
    pub instruction_delta: u64,
    pub stable_pages_delta: u64,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectionKernelDiagnostic {
    pub kernel_id: String,
    pub session_id: String,
    pub turn_number: u32,
    pub dirty_queue_len: usize,
    pub oldest_dirty_age_ms: Option<u64>,
    pub kernel_generation: u64,
    pub flushed_generation: u64,
    pub lag_generations: u64,
    pub lag_ms: u64,
    pub flushed_at_ms: u64,
    pub pending_entries: usize,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectionFlushDiagnostic {
    pub flushed_at_ms: u64,
    pub entries_processed: usize,
    pub rows_flushed: usize,
    pub queue_len_before: usize,
    pub queue_len_after: usize,
    pub flush_truncated: bool,
    pub stable_pages_delta: u64,
    pub flush_instructions: u64,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectionDiagnosticSnapshot {
    pub kernels: Vec<ProjectionKernelDiagnostic>,
    pub total_dirty_queue_len: usize,
    pub oldest_dirty_age_ms: Option<u64>,
    pub last_flush: Option<ProjectionFlushDiagnostic>,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnParticipant {
    pub participant_id: String,
    pub player_id: String,
    pub principal_text: Option<String>,
    pub slot_index: u8,
    pub status: String,
    pub participant: Option<GameParticipant>,
}

pub(crate) struct RuntimeReadinessCounts {
    pub ready_count: usize,
    pub participant_count: usize,
    pub all_ready: bool,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct RuntimeOccupancyCell {
    pub x: u16,
    pub y: u16,
    pub layer: String,
    pub occupant_kind: String,
    pub occupant_id_text: String,
    pub owner_participant_id: Option<String>,
    pub blocking: bool,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct RuntimeContactCell {
    pub x: u16,
    pub y: u16,
    pub subject_kind: String,
    pub subject_id_text: String,
    pub owner_participant_id: Option<String>,
    pub guarded_neutral_army_id: Option<String>,
    pub status: String,
}

#[cfg(not(feature = "benchmark"))]
fn occupancy_projection_key(cell: &RuntimeOccupancyCell) -> String {
    format!("{}:{}:{}", cell.layer, cell.x, cell.y)
}

#[cfg(not(feature = "benchmark"))]
fn contact_projection_key(cell: &RuntimeContactCell) -> String {
    format!(
        "{}:{}:{}:{}",
        cell.x, cell.y, cell.subject_kind, cell.subject_id_text
    )
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct RuntimeMovementIntent {
    pub intent_id: String,
    pub command_id: String,
    pub actor_participant_id: String,
    pub champion_id: String,
    pub path_json: String,
    pub path_hash: String,
    pub status: String,
    pub durable_intent: Option<MovementIntent>,
    pub champion: Option<Champion>,
    pub participant: Option<GameParticipant>,
}

impl RuntimeMovementIntent {
    pub(crate) fn from_durable(intent: MovementIntent) -> Self {
        Self {
            intent_id: intent.id().to_string(),
            command_id: Id::<GameCommand>::from_key(intent.command_id).to_string(),
            actor_participant_id: Id::<GameParticipant>::from_key(intent.actor_participant_id)
                .to_string(),
            champion_id: Id::<Champion>::from_key(intent.champion_id).to_string(),
            path_json: intent.path_json.clone(),
            path_hash: intent.path_hash.clone(),
            status: intent.status.clone(),
            durable_intent: Some(intent),
            champion: None,
            participant: None,
        }
    }

    pub(crate) fn from_pending(
        intent: MovementIntent,
        champion: Champion,
        participant: GameParticipant,
    ) -> Self {
        let mut runtime_intent = Self::from_durable(intent);
        runtime_intent.champion = Some(champion);
        runtime_intent.participant = Some(participant);
        runtime_intent
    }
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct RuntimeChampionSpell {
    pub champion_id: Ulid,
    pub spell_id: Ulid,
    pub spell_slug: Option<String>,
    pub learned_turn: u32,
    pub last_command_id: Option<Ulid>,
    pub needs_flush: bool,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnCommandReceipt {
    pub command_id: String,
    pub command_type: String,
    pub actor_participant_id: String,
    pub client_nonce_text: String,
    pub client_nonce: u64,
    pub turn_number: u32,
    pub payload_hash: String,
    #[cfg(not(feature = "benchmark"))]
    pub payload_json: Option<String>,
    pub response: CommandResponse,
}

impl SessionTurnCommandReceipt {
    pub(crate) fn status_view(&self) -> CommandStatusView {
        self.response.status_view()
    }
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnEvent {
    pub command_id: Option<String>,
    pub event: ApiEventView,
    pub flushed: bool,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnEventSeqBlock {
    pub next_event_seq: u64,
    pub exclusive_end_event_seq: u64,
}

impl SessionTurnEventSeqBlock {
    pub(crate) fn take_event_seq(&mut self) -> Option<u64> {
        if self.next_event_seq >= self.exclusive_end_event_seq {
            return None;
        }
        let event_seq = self.next_event_seq;
        self.next_event_seq = self.next_event_seq.saturating_add(1);
        Some(event_seq)
    }
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct ObjectTurnDelta {
    pub subject_kind: String,
    pub subject_id: String,
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct ResourceTurnDelta {
    pub participant_id: String,
    pub gold: i64,
    pub wood: i64,
    pub stone: i64,
    pub iron: i64,
    pub crystal: i64,
    pub ember: i64,
    pub aether: i64,
    #[cfg(not(feature = "benchmark"))]
    pub ledger: Option<ResourceLedgerDelta>,
}

#[cfg(not(feature = "benchmark"))]
#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct ResourceLedgerDelta {
    pub command_id: String,
    pub ledger_key: String,
    pub resource_key: String,
    pub delta: i64,
    pub balance_after: u64,
    pub reason: String,
}

#[derive(Clone)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct MovementCursor {
    pub consumed_steps: u32,
    pub parked_intents: u32,
}

#[derive(Clone, Default)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnDirtySets {
    pub participants: bool,
    pub ready: bool,
    pub champion_snapshots: bool,
    pub champion_spell_snapshots: bool,
    pub world_object_snapshots: bool,
    pub occupancy_index: bool,
    pub contact_index: bool,
    pub intents: bool,
    pub command_receipts: bool,
    pub events: bool,
    pub object_deltas: bool,
    pub resource_deltas: bool,
    pub quest_snapshots: bool,
    pub scenario_rule_snapshots: bool,
    pub cursor: bool,
}

#[derive(Default)]
#[cfg_attr(
    not(feature = "benchmark"),
    derive(candid::CandidType, serde::Deserialize)
)]
pub(crate) struct SessionTurnRuntimeSnapshot {
    pub runtimes: Vec<SessionTurnRuntime>,
}

thread_local! {
    static ACTIVE_SESSION_TURN_RUNTIMES: RefCell<BTreeMap<String, SessionTurnRuntime>> =
        RefCell::new(BTreeMap::new());
    #[cfg(not(feature = "benchmark"))]
    static LAST_SESSION_PROJECTION_FLUSH: RefCell<Option<ProjectionFlushDiagnostic>> =
        const { RefCell::new(None) };
}

pub(crate) fn runtime_key(session_id: &str, turn_number: u32) -> String {
    format!("{session_id}:{turn_number}")
}

pub(crate) fn contains_runtime(session_id: &str, turn_number: u32) -> bool {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(&key))
}

pub(crate) fn runtime_object_deltas_empty(session_id: &str, turn_number: u32) -> bool {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .get(&key)
            .is_some_and(|runtime| runtime.object_deltas.is_empty())
    })
}

pub(crate) fn participant_ready(session_id: &str, turn_number: u32, participant_id: &str) -> bool {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .get(&key)
            .is_some_and(|runtime| runtime.ready_participants.contains(participant_id))
    })
}

pub(crate) fn quest_snapshot(
    session_id: &str,
    turn_number: u32,
    participant_id: &str,
    quest_key: &str,
) -> Option<QuestState> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes.borrow().get(&key).and_then(|runtime| {
            runtime
                .quest_snapshots
                .iter()
                .find(|quest| {
                    Id::<GameParticipant>::from_key(quest.participant_id).to_string()
                        == participant_id
                        && quest.quest_key == quest_key
                })
                .cloned()
        })
    })
}

pub(crate) fn mirror_quest_snapshot(session_id: &str, turn_number: u32, quest: QuestState) -> bool {
    with_runtime_mut(session_id, turn_number, |runtime| {
        runtime.upsert_quest_snapshot(quest);
        true
    })
    .unwrap_or(false)
}

pub(crate) fn mirror_champion_spell_snapshot(
    session_id: &str,
    turn_number: u32,
    spell: RuntimeChampionSpell,
) -> bool {
    with_runtime_mut(session_id, turn_number, |runtime| {
        runtime.upsert_champion_spell_snapshot(spell);
        true
    })
    .unwrap_or(false)
}

pub(crate) fn scenario_rule_snapshot(
    session_id: &str,
    turn_number: u32,
    rule_key: &str,
) -> Option<ScenarioRuleState> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes.borrow().get(&key).and_then(|runtime| {
            runtime
                .scenario_rule_snapshots
                .iter()
                .find(|rule| rule.rule_key == rule_key)
                .cloned()
        })
    })
}

pub(crate) fn scenario_rule_snapshots(
    session_id: &str,
    turn_number: u32,
) -> Vec<ScenarioRuleState> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .get(&key)
            .map(|runtime| runtime.scenario_rule_snapshots.clone())
            .unwrap_or_default()
    })
}

pub(crate) fn mirror_scenario_rule_snapshot(
    session_id: &str,
    turn_number: u32,
    rule: ScenarioRuleState,
) -> bool {
    with_runtime_mut(session_id, turn_number, |runtime| {
        runtime.upsert_scenario_rule_snapshot(rule);
        true
    })
    .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn active_runtime_count() -> usize {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow().len())
}

pub(crate) fn latest_turn_number_for_session(session_id: &str) -> Option<u32> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .map(|runtime| runtime.turn_number)
            .max()
    })
}

pub(crate) fn caller_context_rows(
    caller_text: &str,
    session_id: &str,
) -> Option<(GameSession, GameParticipant)> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                let session = runtime.session.clone()?;
                if session.state != "active" {
                    return None;
                }
                let participant = runtime.participants.iter().find(|participant| {
                    participant.status == "active"
                        && participant.principal_text.as_deref() == Some(caller_text)
                })?;
                participant.participant.clone().map(|row| (session, row))
            })
    })
}

pub(crate) fn latest_session_rows(session_id: &str) -> Option<(GameSession, Vec<GameParticipant>)> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                let session = runtime.session.clone()?;
                if session.state != "active" {
                    return None;
                }
                let participants = runtime
                    .participants
                    .iter()
                    .filter(|participant| participant.status == "active")
                    .filter_map(|participant| participant.participant.clone())
                    .collect::<Vec<_>>();
                Some((session, participants))
            })
    })
}

pub(crate) fn mirror_champion_update(champion: &Champion) {
    let session_id = Id::<GameSession>::from_key(champion.session_id).to_string();
    let champion_id = champion.id().to_string();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        for runtime in runtimes
            .values_mut()
            .filter(|runtime| runtime.session_id == session_id)
        {
            runtime.upsert_champion_snapshot(champion.clone());
            if matches!(champion.status.as_str(), "active" | "in_battle") {
                runtime.upsert_occupancy_for_occupant(RuntimeOccupancyCell {
                    x: champion.x,
                    y: champion.y,
                    layer: "champion".to_string(),
                    occupant_kind: "champion".to_string(),
                    occupant_id_text: champion_id.clone(),
                    owner_participant_id: Some(
                        Id::<GameParticipant>::from_key(champion.participant_id).to_string(),
                    ),
                    blocking: champion.status == "active",
                });
            } else {
                runtime.remove_occupancy_for_occupant("champion", &champion_id);
            }
        }
    });
}

pub(crate) fn mirror_participant_update(participant: &GameParticipant) {
    let session_id = Id::<GameSession>::from_key(participant.session_id).to_string();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        for runtime in runtimes
            .values_mut()
            .filter(|runtime| runtime.session_id == session_id)
        {
            let principal_text = runtime
                .participants
                .iter()
                .find(|existing| existing.participant_id == participant.id().to_string())
                .and_then(|existing| existing.principal_text.clone());
            runtime.upsert_participant(SessionTurnParticipant {
                participant_id: participant.id().to_string(),
                player_id: Id::<domm_degens_schema::schema::PlayerAccount>::from_key(
                    participant.player_id,
                )
                .to_string(),
                principal_text,
                slot_index: participant.slot_index,
                status: participant.status.clone(),
                participant: Some(participant.clone()),
            });
        }
    });
}

pub(crate) fn mirror_world_object_update(object: &WorldObject) {
    let session_id = Id::<GameSession>::from_key(object.session_id).to_string();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        for runtime in runtimes
            .values_mut()
            .filter(|runtime| runtime.session_id == session_id)
        {
            runtime.upsert_world_object_snapshot(object.clone());
            runtime.push_object_delta(ObjectTurnDelta {
                subject_kind: "world_object".to_string(),
                subject_id: object.id().to_string(),
                x: object.x,
                y: object.y,
                visible: object.state != "collected",
            });
            runtime.upsert_contact_cell(RuntimeContactCell {
                x: object.x,
                y: object.y,
                subject_kind: "world_object".to_string(),
                subject_id_text: object.id().to_string(),
                owner_participant_id: object
                    .owner_participant_id
                    .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
                guarded_neutral_army_id: object.guarded_neutral_army_id.map(|id| {
                    Id::<domm_degens_schema::schema::NeutralArmy>::from_key(id).to_string()
                }),
                status: object.state.clone(),
            });
        }
    });
}

pub(crate) fn participant_snapshot(
    session_id: &str,
    participant_id: &str,
) -> Option<GameParticipant> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                runtime
                    .participants
                    .iter()
                    .find(|participant| participant.participant_id == participant_id)
                    .and_then(|participant| participant.participant.clone())
            })
    })
}

pub(crate) fn record_resource_delta(
    session_id: &str,
    turn_number: u32,
    participant_id: &str,
    resource_key: &str,
    delta: i64,
) {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        let Some(runtime) = runtimes.get_mut(&runtime_key(session_id, turn_number)) else {
            return;
        };
        let Some(resource_delta) = resource_turn_delta(participant_id, resource_key, delta) else {
            return;
        };
        runtime.push_resource_delta(resource_delta);
    });
}

#[cfg(not(feature = "benchmark"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_resource_ledger_delta(
    session_id: &str,
    turn_number: u32,
    participant_id: &str,
    command_id: &str,
    ledger_key: String,
    resource_key: &str,
    delta: i64,
    balance_after: u64,
    reason: &str,
) {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        let Some(runtime) = runtimes.get_mut(&runtime_key(session_id, turn_number)) else {
            return;
        };
        let Some(mut resource_delta) = resource_turn_delta(participant_id, resource_key, delta)
        else {
            return;
        };
        resource_delta.ledger = Some(ResourceLedgerDelta {
            command_id: command_id.to_string(),
            ledger_key,
            resource_key: resource_key.to_string(),
            delta,
            balance_after,
            reason: reason.to_string(),
        });
        runtime.push_resource_delta(resource_delta);
    });
}

fn resource_turn_delta(
    participant_id: &str,
    resource_key: &str,
    delta: i64,
) -> Option<ResourceTurnDelta> {
    let mut resource_delta = ResourceTurnDelta {
        participant_id: participant_id.to_string(),
        gold: 0,
        wood: 0,
        stone: 0,
        iron: 0,
        crystal: 0,
        ember: 0,
        aether: 0,
        #[cfg(not(feature = "benchmark"))]
        ledger: None,
    };
    match resource_key {
        "gold" => resource_delta.gold = delta,
        "wood" => resource_delta.wood = delta,
        "stone" => resource_delta.stone = delta,
        "iron" => resource_delta.iron = delta,
        "crystal" => resource_delta.crystal = delta,
        "ember" => resource_delta.ember = delta,
        "aether" => resource_delta.aether = delta,
        _ => return None,
    }
    Some(resource_delta)
}

pub(crate) fn champion_snapshot(session_id: &str, champion_id: &str) -> Option<Champion> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                runtime
                    .champion_snapshots
                    .iter()
                    .find(|champion| champion.id().to_string() == champion_id)
                    .cloned()
            })
    })
}

pub(crate) fn champion_snapshot_by_start(session_id: &str, x: u16, y: u16) -> Option<Champion> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                runtime
                    .champion_snapshots
                    .iter()
                    .find(|champion| champion.x == x && champion.y == y)
                    .cloned()
            })
    })
}

pub(crate) fn world_object_at(session_id: &str, x: u16, y: u16) -> Option<Option<WorldObject>> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .map(|runtime| {
                runtime
                    .world_object_snapshots
                    .iter()
                    .find(|object| object.x == x && object.y == y)
                    .cloned()
            })
    })
}

pub(crate) fn world_object_by_id(session_id: &str, object_id: &str) -> Option<WorldObject> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .and_then(|runtime| {
                runtime
                    .world_object_snapshots
                    .iter()
                    .find(|object| object.id().to_string() == object_id)
                    .cloned()
            })
    })
}

pub(crate) fn world_object_snapshots(session_id: &str) -> Vec<WorldObject> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .max_by_key(|runtime| runtime.turn_number)
            .map(|runtime| runtime.world_object_snapshots.clone())
            .unwrap_or_default()
    })
}

pub(crate) fn insert_runtime(runtime: SessionTurnRuntime) -> Option<SessionTurnRuntime> {
    ACTIVE_SESSION_TURN_RUNTIMES
        .with(|runtimes| runtimes.borrow_mut().insert(runtime.key(), runtime))
}

pub(crate) fn remove_runtime(session_id: &str, turn_number: u32) -> Option<SessionTurnRuntime> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow_mut().remove(&key))
}

pub(crate) fn mirror_session_update(session: &GameSession) -> bool {
    let session_id = session.id().to_string();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut updated = false;
        for runtime in runtimes
            .borrow_mut()
            .values_mut()
            .filter(|runtime| runtime.session_id == session_id)
        {
            runtime.session = Some(session.clone());
            runtime.mark_dirty();
            #[cfg(not(feature = "benchmark"))]
            runtime.record_projection_dirty(
                ProjectionEntity::Session,
                session_id.clone(),
                ProjectionDirtyOp::Upsert,
                PROJECTION_PRIORITY_NORMAL,
            );
            updated = true;
        }
        updated
    })
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn projection_dirty_queue_snapshot() -> Vec<ProjectionDirtyEntry> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .flat_map(|runtime| runtime.projection_dirty_queue.iter().cloned())
            .collect()
    })
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn projection_checkpoints_snapshot() -> Vec<ProjectionCheckpoint> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .map(|runtime| runtime.projection_checkpoint.clone())
            .collect()
    })
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn projection_diagnostic_snapshot() -> ProjectionDiagnosticSnapshot {
    let now_ms = crate::services::clock::now_ms();
    let mut kernels = ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .map(|runtime| projection_kernel_diagnostic(runtime, now_ms))
            .collect::<Vec<_>>()
    });
    kernels.sort_by(|left, right| left.kernel_id.cmp(&right.kernel_id));
    let total_dirty_queue_len = kernels.iter().map(|kernel| kernel.dirty_queue_len).sum();
    let oldest_dirty_age_ms = kernels
        .iter()
        .filter_map(|kernel| kernel.oldest_dirty_age_ms)
        .max();
    let last_flush = LAST_SESSION_PROJECTION_FLUSH.with(|last_flush| last_flush.borrow().clone());

    ProjectionDiagnosticSnapshot {
        kernels,
        total_dirty_queue_len,
        oldest_dirty_age_ms,
        last_flush,
    }
}

#[cfg(not(feature = "benchmark"))]
fn projection_kernel_diagnostic(
    runtime: &SessionTurnRuntime,
    now_ms: u64,
) -> ProjectionKernelDiagnostic {
    let oldest_dirty_at_ms = runtime
        .projection_dirty_queue
        .iter()
        .map(|entry| entry.first_dirty_at_ms)
        .min();
    let oldest_dirty_age_ms = oldest_dirty_at_ms.map(|dirty_at| now_ms.saturating_sub(dirty_at));
    let lag_generations = runtime
        .generation
        .saturating_sub(runtime.projection_checkpoint.flushed_generation);

    ProjectionKernelDiagnostic {
        kernel_id: runtime.key(),
        session_id: runtime.session_id.clone(),
        turn_number: runtime.turn_number,
        dirty_queue_len: runtime.projection_dirty_queue.len(),
        oldest_dirty_age_ms,
        kernel_generation: runtime.generation,
        flushed_generation: runtime.projection_checkpoint.flushed_generation,
        lag_generations,
        lag_ms: oldest_dirty_age_ms.unwrap_or(0),
        flushed_at_ms: runtime.projection_checkpoint.flushed_at_ms,
        pending_entries: runtime.projection_checkpoint.pending_entries,
    }
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn flush_runtime_projection_queue(
    limits: ProjectionFlushLimits,
) -> Result<ProjectionFlushOutcome, ApiError> {
    let mut entries = projection_dirty_queue_snapshot();
    let queue_len_before = entries.len();
    entries.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then(left.first_dirty_at_ms.cmp(&right.first_dirty_at_ms))
            .then(left.kernel_id.cmp(&right.kernel_id))
            .then(left.key.cmp(&right.key))
    });

    let instruction_start = canic_cdk::api::instruction_counter();
    let stable_pages_start = canic_cdk::api::stable_size();
    let mut outcome = ProjectionFlushOutcome {
        queue_len_before,
        ..ProjectionFlushOutcome::default()
    };

    for entry in entries {
        if projection_flush_limit_reached(&limits, &outcome, instruction_start, stable_pages_start)
        {
            outcome.truncated = true;
            break;
        }

        let runtime = runtime_snapshot_by_kernel_id(&entry.kernel_id);
        let rows_flushed = if let Some(runtime) = runtime {
            flush_projection_dirty_entry(&runtime, &entry)?
        } else {
            0
        };
        complete_projection_dirty_entry(&entry);
        outcome.entries_processed = outcome.entries_processed.saturating_add(1);
        outcome.rows_flushed = outcome.rows_flushed.saturating_add(rows_flushed);
    }

    outcome.instruction_delta =
        canic_cdk::api::instruction_counter().saturating_sub(instruction_start);
    outcome.stable_pages_delta = canic_cdk::api::stable_size().saturating_sub(stable_pages_start);
    outcome.queue_len_after = projection_dirty_queue_snapshot().len();
    if outcome.queue_len_after != 0
        && projection_flush_limit_reached(&limits, &outcome, instruction_start, stable_pages_start)
    {
        outcome.truncated = true;
    }
    record_projection_flush_diagnostic(&outcome);
    Ok(outcome)
}

#[cfg(not(feature = "benchmark"))]
fn record_projection_flush_diagnostic(outcome: &ProjectionFlushOutcome) {
    let diagnostic = ProjectionFlushDiagnostic {
        flushed_at_ms: crate::services::clock::now_ms(),
        entries_processed: outcome.entries_processed,
        rows_flushed: outcome.rows_flushed,
        queue_len_before: outcome.queue_len_before,
        queue_len_after: outcome.queue_len_after,
        flush_truncated: outcome.truncated,
        stable_pages_delta: outcome.stable_pages_delta,
        flush_instructions: outcome.instruction_delta,
    };
    LAST_SESSION_PROJECTION_FLUSH.with(|last_flush| {
        *last_flush.borrow_mut() = Some(diagnostic);
    });
}

#[cfg(not(feature = "benchmark"))]
fn projection_flush_limit_reached(
    limits: &ProjectionFlushLimits,
    outcome: &ProjectionFlushOutcome,
    instruction_start: u64,
    stable_pages_start: u64,
) -> bool {
    outcome.entries_processed >= limits.max_rows
        || canic_cdk::api::instruction_counter().saturating_sub(instruction_start)
            >= limits.max_instructions
        || canic_cdk::api::stable_size().saturating_sub(stable_pages_start)
            >= limits.max_stable_pages_delta
}

#[cfg(not(feature = "benchmark"))]
fn runtime_snapshot_by_kernel_id(kernel_id: &str) -> Option<SessionTurnRuntime> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow().get(kernel_id).cloned())
}

#[cfg(not(feature = "benchmark"))]
fn complete_projection_dirty_entry(entry: &ProjectionDirtyEntry) {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        let Some(runtime) = runtimes.get_mut(&entry.kernel_id) else {
            return;
        };
        if entry.entity == ProjectionEntity::Event {
            for runtime_event in &mut runtime.active_events {
                if runtime_event.event.event_key == entry.key {
                    runtime_event.flushed = true;
                }
            }
        }
        runtime.projection_dirty_queue.retain(|candidate| {
            !(candidate.kernel_id == entry.kernel_id
                && candidate.entity == entry.entity
                && candidate.key == entry.key)
        });
        refresh_projection_checkpoint(runtime);
    });
}

#[cfg(not(feature = "benchmark"))]
fn refresh_projection_checkpoint(runtime: &mut SessionTurnRuntime) {
    let min_pending_generation = runtime
        .projection_dirty_queue
        .iter()
        .map(|entry| entry.generation)
        .min();
    let flushed_generation = min_pending_generation
        .map(|generation| generation.saturating_sub(1))
        .unwrap_or(runtime.generation)
        .min(runtime.generation);
    runtime.projection_checkpoint.kernel_id = runtime.key();
    runtime.projection_checkpoint.flushed_generation = runtime
        .projection_checkpoint
        .flushed_generation
        .max(flushed_generation);
    runtime.projection_checkpoint.flushed_at_ms = crate::services::clock::now_ms();
    runtime.projection_checkpoint.pending_entries = runtime.projection_dirty_queue.len();
}

pub(crate) fn with_runtime<R>(
    session_id: &str,
    turn_number: u32,
    read: impl FnOnce(&SessionTurnRuntime) -> R,
) -> Option<R> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let runtimes = runtimes.borrow();
        runtimes.get(&key).map(read)
    })
}

pub(crate) fn with_runtime_mut<R>(
    session_id: &str,
    turn_number: u32,
    mutate: impl FnOnce(&mut SessionTurnRuntime) -> R,
) -> Option<R> {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.get_mut(&key).map(mutate)
    })
}

pub(crate) fn prepare_active_turn_runtime(
    session: &mut GameSession,
) -> Result<Option<SessionTurnRuntime>, ApiError> {
    if session.state != "active"
        || contains_runtime(&session.id().to_string(), session.current_turn)
    {
        return Ok(None);
    }

    let event_seq_block = reserve_event_seq_block_in_session(session)?;
    let mut runtime = SessionTurnRuntime::new(
        session.id().to_string(),
        session.current_turn,
        timestamp_to_u64(session.turn_started_at),
        timestamp_to_u64(session.turn_deadline_at),
        u64::from(session.turn_duration_ms),
    );
    runtime.session = Some(session.clone());
    runtime.event_seq_block = Some(event_seq_block);
    hydrate_active_turn_rows(session, &mut runtime)?;
    if let Some(previous) = latest_runtime_before(&session.id().to_string(), session.current_turn) {
        carry_forward_runtime_state(&mut runtime, previous);
    }
    Ok(Some(runtime))
}

pub(crate) fn prepare_active_turn_runtime_from_previous(
    session: &mut GameSession,
    previous_turn: u32,
) -> Result<Option<SessionTurnRuntime>, ApiError> {
    if session.state != "active" {
        return Ok(None);
    }
    let session_id = session.id().to_string();
    if contains_runtime(&session_id, session.current_turn) {
        if let Some(previous) = with_runtime(&session_id, previous_turn, Clone::clone) {
            with_runtime_mut(&session_id, session.current_turn, |runtime| {
                carry_forward_runtime_state(runtime, previous);
            });
        }
        return Ok(None);
    }

    let event_seq_block = reserve_event_seq_block_in_session(session)?;
    let previous = with_runtime(&session_id, previous_turn, Clone::clone);
    let Some(previous) = previous else {
        let mut runtime = SessionTurnRuntime::new(
            session.id().to_string(),
            session.current_turn,
            timestamp_to_u64(session.turn_started_at),
            timestamp_to_u64(session.turn_deadline_at),
            u64::from(session.turn_duration_ms),
        );
        runtime.session = Some(session.clone());
        runtime.event_seq_block = Some(event_seq_block);
        hydrate_active_turn_rows(session, &mut runtime)?;
        return Ok(Some(runtime));
    };

    let mut runtime = SessionTurnRuntime::new(
        session.id().to_string(),
        session.current_turn,
        timestamp_to_u64(session.turn_started_at),
        timestamp_to_u64(session.turn_deadline_at),
        u64::from(session.turn_duration_ms),
    );
    runtime.session = Some(session.clone());
    runtime.event_seq_block = Some(event_seq_block);
    carry_forward_runtime_state(&mut runtime, previous);
    Ok(Some(runtime))
}

fn latest_runtime_before(session_id: &str, turn_number: u32) -> Option<SessionTurnRuntime> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id && runtime.turn_number < turn_number)
            .max_by_key(|runtime| runtime.turn_number)
            .cloned()
    })
}

fn carry_forward_runtime_state(runtime: &mut SessionTurnRuntime, previous: SessionTurnRuntime) {
    let participants_complete = previous.participants_complete;
    for participant in previous
        .participants
        .into_iter()
        .filter(|participant| participant.status == "active")
    {
        runtime.upsert_participant(participant);
    }
    for champion in previous.champion_snapshots {
        runtime.upsert_champion_snapshot(champion);
    }
    for spell in previous.champion_spell_snapshots {
        runtime.upsert_champion_spell_snapshot(spell);
    }
    for champion_id in previous.complete_champion_spellbooks {
        runtime.complete_champion_spellbooks.insert(champion_id);
    }
    for object in previous.world_object_snapshots {
        runtime.upsert_world_object_snapshot(object);
    }
    for cell in previous.occupancy_index {
        if cell.layer == "champion" {
            runtime.upsert_occupancy_for_occupant(cell);
        } else {
            runtime.upsert_occupancy_cell(cell);
        }
    }
    for cell in previous.contact_index {
        runtime.upsert_contact_cell(cell);
    }
    runtime.participants_complete = participants_complete;
    runtime.ready_complete = participants_complete;
}

pub(crate) fn ensure_active_turn_runtime(session: &mut GameSession) -> Result<(), ApiError> {
    let Some(runtime) = prepare_active_turn_runtime(session)? else {
        return Ok(());
    };
    *session = sessions::update_session(session.clone())?;
    insert_runtime(runtime);
    Ok(())
}

fn reserve_event_seq_block_in_session(
    session: &mut GameSession,
) -> Result<SessionTurnEventSeqBlock, ApiError> {
    let start = session.next_event_seq;
    let end = start
        .checked_add(SESSION_TURN_RUNTIME_EVENT_SEQ_BLOCK_SIZE)
        .ok_or_else(|| {
            ApiError::new(
                "event_sequence_exhausted",
                "session event sequence cannot reserve another active turn block",
                true,
            )
        })?;
    session.next_event_seq = end;
    Ok(SessionTurnEventSeqBlock {
        next_event_seq: start,
        exclusive_end_event_seq: end,
    })
}

fn hydrate_active_turn_rows(
    session: &GameSession,
    runtime: &mut SessionTurnRuntime,
) -> Result<(), ApiError> {
    let participants = sessions::page_participants_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items;
    hydrate_runtime_participants(runtime, &participants)?;
    runtime.participants_complete = true;
    hydrate_runtime_ready_rows(session, runtime)?;
    runtime.ready_complete = true;
    hydrate_runtime_champions_and_occupancy(session, runtime, &participants)?;
    hydrate_runtime_town_contacts(session, runtime)?;
    hydrate_runtime_world_object_contacts(session, runtime)?;
    Ok(())
}

fn hydrate_runtime_participants(
    runtime: &mut SessionTurnRuntime,
    participants: &[GameParticipant],
) -> Result<(), ApiError> {
    for participant in participants {
        let principal_text = players::load_player_account(Id::from_key(participant.player_id))?
            .map(|player| player.account_principal.to_string());
        runtime.upsert_participant(SessionTurnParticipant {
            participant_id: participant.id().to_string(),
            player_id: Id::<domm_degens_schema::schema::PlayerAccount>::from_key(
                participant.player_id,
            )
            .to_string(),
            principal_text,
            slot_index: participant.slot_index,
            status: participant.status.clone(),
            participant: Some(participant.clone()),
        });
    }
    Ok(())
}

fn hydrate_runtime_ready_rows(
    session: &GameSession,
    runtime: &mut SessionTurnRuntime,
) -> Result<(), ApiError> {
    for ready in turn_ready::page_turn_ready_by_session_turn(
        session.id(),
        session.current_turn,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    {
        runtime.mark_ready(Id::<GameParticipant>::from_key(ready.participant_id).to_string());
    }
    Ok(())
}

fn hydrate_runtime_champions_and_occupancy(
    session: &GameSession,
    runtime: &mut SessionTurnRuntime,
    participants: &[GameParticipant],
) -> Result<(), ApiError> {
    for participant in participants {
        for status in ["active", "in_battle"] {
            for champion in champions_artifacts::list_champions_by_session_owner_status(
                session.id(),
                participant.id(),
                status,
                domm_game::MAX_LIST_LIMIT,
            )? {
                let champion_id = champion.id().to_string();
                runtime.upsert_occupancy_cell(RuntimeOccupancyCell {
                    x: champion.x,
                    y: champion.y,
                    layer: "champion".to_string(),
                    occupant_kind: "champion".to_string(),
                    occupant_id_text: champion_id,
                    owner_participant_id: Some(participant.id().to_string()),
                    blocking: champion.status == "active",
                });
                let champion_id = champion.id();
                runtime.upsert_champion_snapshot(champion);
                if session.current_turn == 1 {
                    runtime.mark_champion_spellbook_complete(champion_id);
                }
            }
        }
    }
    Ok(())
}

fn hydrate_runtime_town_contacts(
    session: &GameSession,
    runtime: &mut SessionTurnRuntime,
) -> Result<(), ApiError> {
    for town in towns::page_towns_by_session_status(
        session.id(),
        "active",
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    {
        let town_id = town.id().to_string();
        let owner_participant_id = town
            .owner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string());
        runtime.upsert_occupancy_cell(RuntimeOccupancyCell {
            x: town.x,
            y: town.y,
            layer: "town".to_string(),
            occupant_kind: "town".to_string(),
            occupant_id_text: town_id.clone(),
            owner_participant_id: owner_participant_id.clone(),
            blocking: true,
        });
        runtime.upsert_contact_cell(RuntimeContactCell {
            x: town.x,
            y: town.y,
            subject_kind: "town".to_string(),
            subject_id_text: town_id,
            owner_participant_id,
            guarded_neutral_army_id: None,
            status: town.status.clone(),
        });
    }
    Ok(())
}

fn hydrate_runtime_world_object_contacts(
    session: &GameSession,
    runtime: &mut SessionTurnRuntime,
) -> Result<(), ApiError> {
    for object in map_visibility_occupancy::page_world_objects_by_session(
        session.id(),
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    {
        runtime.upsert_world_object_snapshot(object.clone());
        runtime.upsert_contact_cell(RuntimeContactCell {
            x: object.x,
            y: object.y,
            subject_kind: "world_object".to_string(),
            subject_id_text: object.id().to_string(),
            owner_participant_id: object
                .owner_participant_id
                .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
            guarded_neutral_army_id: object
                .guarded_neutral_army_id
                .map(|id| Id::<domm_degens_schema::schema::NeutralArmy>::from_key(id).to_string()),
            status: object.state.clone(),
        });
    }
    Ok(())
}

pub(crate) fn reserve_session_event_seq(
    runtime: &mut SessionTurnRuntime,
    session: &mut GameSession,
) -> Result<u64, ApiError> {
    if let Some(block) = runtime.event_seq_block.as_mut()
        && let Some(event_seq) = block.take_event_seq()
    {
        runtime.mark_dirty();
        return Ok(event_seq);
    }

    let start = session.next_event_seq;
    let end = start
        .checked_add(SESSION_TURN_RUNTIME_EVENT_SEQ_BLOCK_SIZE)
        .ok_or_else(|| {
            ApiError::new(
                "event_sequence_exhausted",
                "session event sequence cannot reserve another active turn block",
                true,
            )
        })?;
    let mut updated = session.clone();
    updated.next_event_seq = end;
    *session = sessions::update_session(updated)?;
    runtime.event_seq_block = Some(SessionTurnEventSeqBlock {
        next_event_seq: start.saturating_add(1),
        exclusive_end_event_seq: end,
    });
    runtime.dirty.events = true;
    runtime.mark_dirty();
    Ok(start)
}

pub(crate) fn take_reserved_session_event_seq(session_id: &str, turn_number: u32) -> Option<u64> {
    with_runtime_mut(session_id, turn_number, |runtime| {
        let event_seq = runtime
            .event_seq_block
            .as_mut()
            .and_then(SessionTurnEventSeqBlock::take_event_seq)?;
        runtime.dirty.events = true;
        runtime.mark_dirty();
        Some(event_seq)
    })
    .flatten()
}

pub(crate) fn active_events_after(
    session_id: &str,
    audience_key: &str,
    after_event_seq: u64,
) -> Vec<ApiEventView> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .flat_map(|runtime| runtime.active_events.iter())
            .filter(|runtime_event| !runtime_event.flushed)
            .map(|runtime_event| &runtime_event.event)
            .filter(|event| event.audience_key == audience_key && event.event_seq > after_event_seq)
            .cloned()
            .collect()
    })
}

pub(crate) fn command_receipt_by_id(
    session_id: &str,
    command_id: &str,
) -> Option<SessionTurnCommandReceipt> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
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
    })
}

pub(crate) fn command_receipt_by_nonce(
    session_id: &str,
    actor_participant_id: &str,
    client_nonce: u64,
) -> Option<SessionTurnCommandReceipt> {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .find_map(|runtime| {
                runtime
                    .command_receipt_by_nonce(actor_participant_id, client_nonce)
                    .cloned()
            })
    })
}

pub(crate) fn runtime_learned_champion_spell(
    session_id: &str,
    champion_id: Id<Champion>,
    spell_slug: &str,
) -> bool {
    let champion_key = champion_id.key();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        runtimes
            .borrow()
            .values()
            .filter(|runtime| runtime.session_id == session_id)
            .any(|runtime| {
                runtime.champion_spell_snapshots.iter().any(|spell| {
                    spell.champion_id == champion_key
                        && spell.spell_slug.as_deref() == Some(spell_slug)
                })
            })
    })
}

pub(crate) fn runtime_champion_spell_slugs_if_complete(
    session_id: &str,
    turn_number: u32,
    champion_id: Id<Champion>,
) -> Option<Vec<String>> {
    let key = runtime_key(session_id, turn_number);
    let champion_key = champion_id.key();
    let champion_id_text = champion_id.to_string();
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let runtimes = runtimes.borrow();
        let runtime = runtimes.get(&key)?;
        if !runtime
            .complete_champion_spellbooks
            .contains(&champion_id_text)
        {
            return None;
        }
        let mut slugs = runtime
            .champion_spell_snapshots
            .iter()
            .filter(|spell| spell.champion_id == champion_key)
            .filter_map(|spell| spell.spell_slug.clone())
            .collect::<Vec<_>>();
        slugs.sort();
        Some(slugs)
    })
}

#[cfg(not(feature = "benchmark"))]
fn flush_projection_dirty_entry(
    runtime: &SessionTurnRuntime,
    entry: &ProjectionDirtyEntry,
) -> Result<usize, ApiError> {
    if entry.op == ProjectionDirtyOp::Tombstone {
        return flush_projection_tombstone(runtime, entry);
    }

    match entry.entity {
        ProjectionEntity::Session => flush_runtime_session(runtime),
        ProjectionEntity::Participant => flush_runtime_participant(runtime, &entry.key),
        ProjectionEntity::Ready => Ok(usize::from(flush_runtime_ready_participant(
            runtime, &entry.key,
        )?)),
        ProjectionEntity::Champion => flush_runtime_champion(runtime, &entry.key),
        ProjectionEntity::ChampionSpell => flush_runtime_champion_spell(runtime, &entry.key),
        ProjectionEntity::WorldObject => flush_runtime_world_object(runtime, &entry.key),
        ProjectionEntity::Occupancy => flush_runtime_occupancy(runtime, &entry.key),
        ProjectionEntity::MovementIntent => flush_runtime_movement_intent(runtime, &entry.key),
        ProjectionEntity::CommandReceipt => {
            flush_runtime_command_receipt_by_key(runtime, &entry.key)
        }
        ProjectionEntity::Event => flush_runtime_event_by_key(runtime, &entry.key),
        ProjectionEntity::ResourceDelta => flush_runtime_resource_delta_by_key(runtime, &entry.key),
        ProjectionEntity::Quest => flush_runtime_quest(runtime, &entry.key),
        ProjectionEntity::ScenarioRule => flush_runtime_scenario_rule(runtime, &entry.key),
        ProjectionEntity::Contact | ProjectionEntity::ObjectDelta => Ok(0),
    }
}

#[cfg(not(feature = "benchmark"))]
fn flush_projection_tombstone(
    runtime: &SessionTurnRuntime,
    entry: &ProjectionDirtyEntry,
) -> Result<usize, ApiError> {
    if entry.entity != ProjectionEntity::Occupancy {
        return Ok(0);
    }
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let mut parts = entry.key.splitn(3, ':');
    let Some(layer) = parts.next() else {
        return Ok(0);
    };
    let Some(occupant_kind) = parts.next() else {
        return Ok(0);
    };
    let Some(occupant_id_text) = parts.next() else {
        return Ok(0);
    };
    let Some(row) = map_visibility_occupancy::find_occupancy_by_occupant(
        session_id,
        occupant_kind,
        occupant_id_text,
        0,
    )?
    .filter(|row| row.layer == layer) else {
        return Ok(0);
    };
    cleanup::delete_row_by_id::<MapOccupancy>("map.delete_runtime_occupancy", row.id())?;
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_session(runtime: &SessionTurnRuntime) -> Result<usize, ApiError> {
    let Some(session) = runtime.session.clone() else {
        return Ok(0);
    };
    sessions::update_session(session)?;
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_participant(
    runtime: &SessionTurnRuntime,
    participant_id: &str,
) -> Result<usize, ApiError> {
    let Some(participant) = runtime
        .participants
        .iter()
        .find(|participant| participant.participant_id == participant_id)
        .and_then(|participant| participant.participant.clone())
    else {
        return Ok(0);
    };
    sessions::update_participant(participant)?;
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_champion(
    runtime: &SessionTurnRuntime,
    champion_id: &str,
) -> Result<usize, ApiError> {
    let Some(champion) = runtime
        .champion_snapshots
        .iter()
        .find(|champion| champion.id().to_string() == champion_id)
    else {
        return Ok(0);
    };
    if champions_artifacts::load_champion(champion.id())?.is_some() {
        champions_artifacts::update_champion(champion.clone())?;
    } else {
        champions_artifacts::insert_champion_row(champion.clone())?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_champion_spell(
    runtime: &SessionTurnRuntime,
    spell_key: &str,
) -> Result<usize, ApiError> {
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let Some(spell) = runtime
        .champion_spell_snapshots
        .iter()
        .find(|spell| format!("{}:{}", spell.champion_id, spell.spell_id) == spell_key)
        .or_else(|| {
            runtime
                .champion_spell_snapshots
                .iter()
                .find(|spell| Id::<Champion>::from_key(spell.champion_id).to_string() == spell_key)
        })
    else {
        return Ok(0);
    };
    if !spell.needs_flush {
        return Ok(0);
    }
    if champions_artifacts::find_champion_spell(
        Id::<Champion>::from_key(spell.champion_id),
        Id::<domm_degens_schema::schema::SpellDefinition>::from_key(spell.spell_id),
    )?
    .is_some()
    {
        return Ok(0);
    }
    let Some(command_id) = spell.last_command_id else {
        return Ok(0);
    };
    champions_artifacts::create_champion_spell(
        session_id,
        Id::<Champion>::from_key(spell.champion_id),
        Id::<domm_degens_schema::schema::SpellDefinition>::from_key(spell.spell_id),
        spell.spell_slug.as_deref().unwrap_or(""),
        spell.learned_turn,
        Id::<GameCommand>::from_key(command_id),
    )?;
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_world_object(
    runtime: &SessionTurnRuntime,
    object_id: &str,
) -> Result<usize, ApiError> {
    let Some(object) = runtime
        .world_object_snapshots
        .iter()
        .find(|object| object.id().to_string() == object_id)
    else {
        return Ok(0);
    };
    if map_visibility_occupancy::load_world_object(object.id())?.is_some() {
        map_visibility_occupancy::update_world_object(object.clone())?;
    } else {
        map_visibility_occupancy::create_world_object(
            parse_ulid_id::<GameSession>(&runtime.session_id)?,
            Id::<MapObjectDefinition>::from_key(object.object_def_id),
            object
                .owner_participant_id
                .map(Id::<GameParticipant>::from_key),
            object
                .guarded_neutral_army_id
                .map(Id::<NeutralArmy>::from_key),
            object.x,
            object.y,
            object.chunk_x,
            object.chunk_y,
            object.state.clone(),
            object.scoring_kind.clone(),
            object.last_visited_turn,
            object.captured_turn,
            object.income_started_turn,
            object.instance_json.clone(),
        )?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_occupancy(
    runtime: &SessionTurnRuntime,
    occupancy_key: &str,
) -> Result<usize, ApiError> {
    let Some(cell) = runtime
        .occupancy_index
        .iter()
        .find(|cell| occupancy_projection_key(cell) == occupancy_key)
    else {
        return Ok(0);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let mut occupancy =
        map_visibility_occupancy::find_occupancy_cell(session_id, cell.x, cell.y, &cell.layer)?
            .filter(|row| {
                row.occupant_kind == cell.occupant_kind
                    && row.occupant_id_text == cell.occupant_id_text
            });
    if occupancy.is_none() {
        occupancy = map_visibility_occupancy::find_occupancy_by_occupant(
            session_id,
            &cell.occupant_kind,
            &cell.occupant_id_text,
            0,
        )?;
    }
    if let Some(mut row) = occupancy {
        row.x = cell.x;
        row.y = cell.y;
        row.chunk_x = runtime_chunk_coord(runtime, cell.x);
        row.chunk_y = runtime_chunk_coord(runtime, cell.y);
        row.layer = cell.layer.clone();
        row.occupant_kind = cell.occupant_kind.clone();
        row.occupant_id_text = cell.occupant_id_text.clone();
        row.occupant_cell_index = 0;
        row.blocking = cell.blocking;
        map_visibility_occupancy::update_occupancy_cell(row)?;
    } else {
        map_visibility_occupancy::create_occupancy_cell(
            session_id,
            cell.x,
            cell.y,
            runtime_chunk_coord(runtime, cell.x),
            runtime_chunk_coord(runtime, cell.y),
            cell.layer.clone(),
            cell.occupant_kind.clone(),
            cell.occupant_id_text.clone(),
            0,
            cell.blocking,
        )?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn runtime_chunk_coord(runtime: &SessionTurnRuntime, value: u16) -> u16 {
    let chunk_size = runtime
        .session
        .as_ref()
        .map(|session| u16::from(session.chunk_size).max(1))
        .unwrap_or(16);
    value / chunk_size
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_movement_intent(
    runtime: &SessionTurnRuntime,
    intent_id: &str,
) -> Result<usize, ApiError> {
    let Some(runtime_intent) = runtime
        .intents
        .iter()
        .find(|intent| intent.intent_id == intent_id)
    else {
        return Ok(0);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let Ok(champion_id) = try_parse_ulid_id::<Champion>(&runtime_intent.champion_id) else {
        return Ok(0);
    };
    let Ok(actor_participant_id) =
        try_parse_ulid_id::<GameParticipant>(&runtime_intent.actor_participant_id)
    else {
        return Ok(0);
    };
    let Ok(command_id) = try_parse_ulid_id::<GameCommand>(&runtime_intent.command_id) else {
        return Ok(0);
    };
    if let Some(mut existing) =
        movement::find_movement_intent(session_id, champion_id, runtime.turn_number)?
    {
        existing.command_id = command_id.key();
        existing.actor_participant_id = actor_participant_id.key();
        existing.status = runtime_intent.status.clone();
        existing.path_json = runtime_intent.path_json.clone();
        existing.path_hash = runtime_intent.path_hash.clone();
        if existing.status == "resolved" {
            existing.resolved_at = Some(Timestamp::now());
        }
        movement::update_movement_intent(existing)?;
    } else {
        movement::create_movement_intent(
            session_id,
            runtime.turn_number,
            actor_participant_id,
            champion_id,
            command_id,
            runtime_intent.status.clone(),
            runtime_intent.path_json.clone(),
            runtime_intent.path_hash.clone(),
        )?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_command_receipt_by_key(
    runtime: &SessionTurnRuntime,
    command_id: &str,
) -> Result<usize, ApiError> {
    let Some(receipt) = runtime
        .command_receipts
        .iter()
        .find(|receipt| receipt.command_id == command_id)
    else {
        return Ok(0);
    };
    Ok(usize::from(flush_runtime_command_receipt(
        runtime, receipt,
    )?))
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_event_by_key(
    runtime: &SessionTurnRuntime,
    event_key: &str,
) -> Result<usize, ApiError> {
    let Some(runtime_event) = runtime
        .active_events
        .iter()
        .find(|runtime_event| runtime_event.event.event_key == event_key)
    else {
        return Ok(0);
    };
    Ok(usize::from(flush_runtime_event(runtime_event)?))
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_resource_delta_by_key(
    runtime: &SessionTurnRuntime,
    resource_key: &str,
) -> Result<usize, ApiError> {
    let Some(resource_delta) = runtime
        .resource_deltas
        .iter()
        .find(|delta| resource_delta_projection_key(delta) == resource_key)
    else {
        return Ok(0);
    };
    Ok(usize::from(flush_runtime_resource_delta(
        runtime,
        resource_delta,
    )?))
}

#[cfg(not(feature = "benchmark"))]
fn resource_delta_projection_key(delta: &ResourceTurnDelta) -> String {
    delta
        .ledger
        .as_ref()
        .map(|ledger| ledger.ledger_key.clone())
        .unwrap_or_else(|| delta.participant_id.clone())
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_quest(runtime: &SessionTurnRuntime, quest_id: &str) -> Result<usize, ApiError> {
    let Some(quest) = runtime
        .quest_snapshots
        .iter()
        .find(|quest| quest.id().to_string() == quest_id)
    else {
        return Ok(0);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let participant_id = Id::<GameParticipant>::from_key(quest.participant_id);
    if scenario_progress::find_quest_by_participant_key(
        session_id,
        participant_id,
        &quest.quest_key,
    )?
    .is_some()
    {
        scenario_progress::update_quest_state(quest.clone())?;
    } else {
        scenario_progress::create_quest_state(
            session_id,
            participant_id,
            quest.quest_key.clone(),
            quest.title.clone(),
            quest.objective_key.clone(),
            quest.status.clone(),
            quest.progress_value,
            quest.required_value,
            quest.reward_gold,
        )?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_scenario_rule(
    runtime: &SessionTurnRuntime,
    rule_id: &str,
) -> Result<usize, ApiError> {
    let Some(rule) = runtime
        .scenario_rule_snapshots
        .iter()
        .find(|rule| rule.id().to_string() == rule_id)
    else {
        return Ok(0);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    if scenario_progress::find_scenario_rule_by_key(session_id, &rule.rule_key)?.is_some() {
        scenario_progress::update_scenario_rule_state(rule.clone())?;
    } else {
        scenario_progress::create_scenario_rule_state(
            session_id,
            rule.rule_key.clone(),
            rule.rule_type.clone(),
            rule.status.clone(),
            rule.victory_state.clone(),
            rule.required_value,
            rule.current_value,
            rule.owner_participant_id
                .map(Id::<GameParticipant>::from_key),
            rule.winner_participant_id
                .map(Id::<GameParticipant>::from_key),
            rule.disabled_reason.clone(),
            rule.last_checked_turn,
        )?;
    }
    Ok(1)
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn flush_runtime_projections_for_upgrade() -> Result<usize, ApiError> {
    let outcome = flush_runtime_projection_queue(ProjectionFlushLimits::unbounded())?;
    if outcome.truncated || outcome.queue_len_after != 0 {
        return Err(ApiError::new(
            "projection_flush_incomplete",
            "runtime projection flush barrier left pending dirty entries",
            true,
        )
        .with_details(format!(
            r#"{{"queue_len_before":{},"queue_len_after":{},"entries_processed":{},"rows_flushed":{},"truncated":{},"instruction_delta":{},"stable_pages_delta":{}}}"#,
            outcome.queue_len_before,
            outcome.queue_len_after,
            outcome.entries_processed,
            outcome.rows_flushed,
            outcome.truncated,
            outcome.instruction_delta,
            outcome.stable_pages_delta,
        )));
    }
    Ok(outcome.rows_flushed)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_ready_participant(
    runtime: &SessionTurnRuntime,
    participant_id: &str,
) -> Result<bool, ApiError> {
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let participant_id_text = participant_id;
    let participant_id = parse_ulid_id::<GameParticipant>(participant_id_text)?;
    if turn_ready::find_turn_ready(session_id, participant_id, runtime.turn_number)?.is_some() {
        return Ok(false);
    }
    let command_id = runtime
        .command_receipts
        .iter()
        .find(|receipt| {
            receipt.command_type == "end_turn"
                && receipt.actor_participant_id == participant_id_text
                && receipt.response.status == domm_game::CommandStatus::Applied
        })
        .and_then(|receipt| try_parse_ulid_id::<GameCommand>(&receipt.command_id).ok());
    turn_ready::mark_turn_ready(
        session_id,
        participant_id,
        runtime.turn_number,
        command_id,
        Timestamp::now(),
    )?;
    Ok(true)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_command_receipt(
    runtime: &SessionTurnRuntime,
    receipt: &SessionTurnCommandReceipt,
) -> Result<bool, ApiError> {
    let Some(payload_json) = receipt.payload_json.clone() else {
        return Ok(false);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
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
        turn_number: receipt.turn_number,
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
fn flush_runtime_resource_delta(
    runtime: &SessionTurnRuntime,
    resource_delta: &ResourceTurnDelta,
) -> Result<bool, ApiError> {
    let Some(ledger_delta) = resource_delta.ledger.clone() else {
        return Ok(false);
    };
    let session_id = parse_ulid_id::<GameSession>(&runtime.session_id)?;
    let participant_id = parse_ulid_id::<GameParticipant>(&resource_delta.participant_id)?;
    let command_id = parse_ulid_id::<GameCommand>(&ledger_delta.command_id)?;
    if economy::find_resource_ledger_entry(command_id, &ledger_delta.ledger_key)?.is_some() {
        return Ok(false);
    }
    economy::create_resource_ledger_entry(
        session_id,
        participant_id,
        command_id,
        ledger_delta.ledger_key,
        runtime.turn_number,
        ledger_delta.resource_key,
        ledger_delta.delta,
        ledger_delta.balance_after,
        ledger_delta.reason,
        "applied".to_string(),
    )?;
    Ok(true)
}

#[cfg(not(feature = "benchmark"))]
fn flush_runtime_event(runtime_event: &SessionTurnEvent) -> Result<bool, ApiError> {
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
            "invalid_runtime_snapshot_id",
            "runtime snapshot contains an invalid id",
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

pub(crate) fn snapshot_for_upgrade() -> SessionTurnRuntimeSnapshot {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| SessionTurnRuntimeSnapshot {
        runtimes: runtimes.borrow().values().cloned().collect(),
    })
}

pub(crate) fn restore_from_upgrade(snapshot: SessionTurnRuntimeSnapshot) {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| {
        let mut runtimes = runtimes.borrow_mut();
        runtimes.clear();
        for runtime in snapshot.runtimes {
            runtimes.insert(runtime.key(), runtime);
        }
    });
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn persist_snapshot_for_upgrade() -> Result<(), String> {
    let bytes = encode_snapshot_for_upgrade(&snapshot_for_upgrade())?;
    if bytes.len() > MAX_SESSION_TURN_RUNTIME_SNAPSHOT_BYTES as usize {
        return Err(format!(
            "session turn runtime snapshot exceeds {} bytes: {}",
            MAX_SESSION_TURN_RUNTIME_SNAPSHOT_BYTES,
            bytes.len()
        ));
    }

    SESSION_TURN_RUNTIME_SNAPSHOT_CELL.with(|cell| {
        cell.borrow_mut().set(RawSessionTurnRuntimeSnapshot(bytes));
    });
    Ok(())
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn restore_snapshot_after_upgrade() -> Result<(), String> {
    let raw = SESSION_TURN_RUNTIME_SNAPSHOT_CELL.with(|cell| cell.borrow().get().clone());
    if raw.as_bytes().is_empty() {
        return Ok(());
    }

    let snapshot = decode_snapshot_for_upgrade(raw.as_bytes())?;
    restore_from_upgrade(snapshot);
    SESSION_TURN_RUNTIME_SNAPSHOT_CELL.with(|cell| {
        cell.borrow_mut()
            .set(RawSessionTurnRuntimeSnapshot::empty());
    });
    Ok(())
}

#[cfg(not(feature = "benchmark"))]
fn encode_snapshot_for_upgrade(snapshot: &SessionTurnRuntimeSnapshot) -> Result<Vec<u8>, String> {
    candid::encode_one(snapshot)
        .map_err(|error| format!("encode session turn runtime snapshot failed: {error}"))
}

#[cfg(not(feature = "benchmark"))]
fn decode_snapshot_for_upgrade(bytes: &[u8]) -> Result<SessionTurnRuntimeSnapshot, String> {
    candid::decode_one(bytes)
        .map_err(|error| format!("decode session turn runtime snapshot failed: {error}"))
}

#[cfg(test)]
pub(crate) fn clear_all_for_tests() {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow_mut().clear());
    #[cfg(not(feature = "benchmark"))]
    LAST_SESSION_PROJECTION_FLUSH.with(|last_flush| {
        *last_flush.borrow_mut() = None;
    });
}

fn timestamp_to_u64(timestamp: icydb::types::Timestamp) -> u64 {
    timestamp.as_millis().try_into().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> SessionTurnRuntime {
        SessionTurnRuntime::new("session:1", 2, 100, 200, 100)
    }

    fn event(event_seq: u64, audience_key: &str) -> SessionTurnEvent {
        SessionTurnEvent {
            command_id: Some("command:1".to_string()),
            event: ApiEventView {
                session_id: "session:1".to_string(),
                event_seq,
                event_key: format!("event:{event_seq}"),
                audience_key: audience_key.to_string(),
                turn_number: 2,
                event_type: "movement_intent_submitted".to_string(),
                subject_kind: Some("champion".to_string()),
                subject_id_text: Some("champion:1".to_string()),
                payload: None,
                redacted: false,
            },
            flushed: false,
        }
    }

    #[test]
    fn runtime_store_round_trips_by_session_turn_key() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.mark_ready("participant:1");

        assert_eq!(active_runtime_count(), 0);
        assert!(insert_runtime(runtime).is_none());
        assert!(contains_runtime("session:1", 2));
        assert_eq!(
            with_runtime("session:1", 2, |runtime| runtime.ready_participants.len()),
            Some(1)
        );

        with_runtime_mut("session:1", 2, |runtime| {
            runtime.mark_ready("participant:2")
        });
        assert_eq!(
            with_runtime("session:1", 2, |runtime| runtime.ready_participants.len()),
            Some(2)
        );
        assert!(remove_runtime("session:1", 2).is_some());
        assert_eq!(active_runtime_count(), 0);
    }

    #[test]
    fn movement_intent_upsert_keeps_one_intent_per_champion() {
        let mut runtime = runtime();
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:1".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;2,1".to_string(),
            path_hash: "hash:1".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:2".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;1,2".to_string(),
            path_hash: "hash:2".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });

        assert_eq!(runtime.intents.len(), 1);
        assert_eq!(runtime.intents[0].command_id, "command:2");
        assert!(runtime.dirty.intents);
    }

    #[cfg(not(feature = "benchmark"))]
    #[test]
    fn projection_dirty_queue_coalesces_entity_key_and_tracks_generation() {
        let mut runtime = runtime();
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:1".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;2,1".to_string(),
            path_hash: "hash:1".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:2".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;1,2".to_string(),
            path_hash: "hash:2".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });

        assert_eq!(runtime.generation, 2);
        assert_eq!(runtime.projection_dirty_queue.len(), 1);
        let dirty = &runtime.projection_dirty_queue[0];
        assert_eq!(dirty.kernel_id, runtime.key());
        assert_eq!(dirty.generation, runtime.generation);
        assert_eq!(dirty.entity, ProjectionEntity::MovementIntent);
        assert_eq!(dirty.key, "intent:1");
        assert_eq!(dirty.op, ProjectionDirtyOp::Upsert);
        assert!(dirty.last_dirty_at_ms >= dirty.first_dirty_at_ms);
    }

    #[cfg(not(feature = "benchmark"))]
    #[test]
    fn completing_event_projection_entry_removes_queue_and_hides_runtime_event() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.push_event(event(10, "public"));
        let dirty = runtime.projection_dirty_queue[0].clone();
        insert_runtime(runtime);

        complete_projection_dirty_entry(&dirty);

        assert!(projection_dirty_queue_snapshot().is_empty());
        assert!(active_events_after("session:1", "public", 9).is_empty());
    }

    #[cfg(not(feature = "benchmark"))]
    #[test]
    fn projection_checkpoint_advances_to_oldest_pending_generation() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:1".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;2,1".to_string(),
            path_hash: "hash:1".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });
        runtime.push_event(event(10, "public"));
        let mut entries = runtime.projection_dirty_queue.clone();
        entries.sort_by_key(|entry| entry.generation);
        insert_runtime(runtime);

        complete_projection_dirty_entry(&entries[0]);
        let checkpoints = projection_checkpoints_snapshot();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].flushed_generation, 1);
        assert_eq!(checkpoints[0].pending_entries, 1);

        complete_projection_dirty_entry(&entries[1]);
        let checkpoints = projection_checkpoints_snapshot();
        assert_eq!(checkpoints[0].flushed_generation, 2);
        assert_eq!(checkpoints[0].pending_entries, 0);
    }

    #[cfg(not(feature = "benchmark"))]
    #[test]
    fn projection_diagnostics_report_queue_lag_and_checkpoint_state() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:1".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;2,1".to_string(),
            path_hash: "hash:1".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });
        runtime.push_event(event(10, "public"));
        insert_runtime(runtime);

        let snapshot = projection_diagnostic_snapshot();

        assert_eq!(snapshot.total_dirty_queue_len, 2);
        assert!(snapshot.oldest_dirty_age_ms.is_some());
        assert!(snapshot.last_flush.is_none());
        assert_eq!(snapshot.kernels.len(), 1);
        let kernel = &snapshot.kernels[0];
        assert_eq!(kernel.kernel_id, "session:1:2");
        assert_eq!(kernel.session_id, "session:1");
        assert_eq!(kernel.turn_number, 2);
        assert_eq!(kernel.dirty_queue_len, 2);
        assert_eq!(kernel.kernel_generation, 2);
        assert_eq!(kernel.flushed_generation, 0);
        assert_eq!(kernel.lag_generations, 2);
        assert_eq!(kernel.pending_entries, 2);
        assert!(kernel.oldest_dirty_age_ms.is_some());
    }

    #[test]
    fn active_events_filter_by_session_audience_and_sequence() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.push_event(event(10, "public"));
        runtime.push_event(event(11, "participant:1"));
        insert_runtime(runtime);

        let events = active_events_after("session:1", "public", 9);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_seq, 10);
        assert!(active_events_after("session:1", "public", 10).is_empty());
        assert!(active_events_after("session:2", "public", 9).is_empty());
    }

    #[test]
    fn snapshot_restores_all_active_turns() {
        clear_all_for_tests();
        insert_runtime(runtime());
        insert_runtime(SessionTurnRuntime::new("session:1", 3, 200, 300, 100));

        let snapshot = snapshot_for_upgrade();
        clear_all_for_tests();
        restore_from_upgrade(snapshot);

        assert_eq!(active_runtime_count(), 2);
        assert!(contains_runtime("session:1", 2));
        assert!(contains_runtime("session:1", 3));
    }

    #[cfg(not(feature = "benchmark"))]
    #[test]
    fn upgrade_snapshot_encoding_preserves_dirty_queue_and_checkpoint() {
        clear_all_for_tests();
        let mut runtime = runtime();
        runtime.upsert_intent(RuntimeMovementIntent {
            intent_id: "intent:1".to_string(),
            command_id: "command:1".to_string(),
            actor_participant_id: "participant:1".to_string(),
            champion_id: "champion:1".to_string(),
            path_json: "1,1;2,1".to_string(),
            path_hash: "hash:1".to_string(),
            status: "pending".to_string(),
            durable_intent: None,
            champion: None,
            participant: None,
        });
        runtime.push_event(event(10, "public"));
        insert_runtime(runtime);

        let snapshot = snapshot_for_upgrade();
        let bytes =
            encode_snapshot_for_upgrade(&snapshot).expect("session runtime snapshot should encode");
        let decoded =
            decode_snapshot_for_upgrade(&bytes).expect("session runtime snapshot should decode");

        assert_eq!(decoded.runtimes.len(), 1);
        let decoded_runtime = &decoded.runtimes[0];
        assert_eq!(decoded_runtime.active_events.len(), 1);
        assert_eq!(decoded_runtime.projection_dirty_queue.len(), 2);
        assert_eq!(decoded_runtime.projection_checkpoint.pending_entries, 2);
        assert_eq!(
            decoded_runtime.projection_checkpoint.kernel_id,
            decoded_runtime.key()
        );
    }

    #[test]
    fn event_sequence_block_hands_out_reserved_range_only() {
        let mut block = SessionTurnEventSeqBlock {
            next_event_seq: 4,
            exclusive_end_event_seq: 6,
        };

        assert_eq!(block.take_event_seq(), Some(4));
        assert_eq!(block.take_event_seq(), Some(5));
        assert_eq!(block.take_event_seq(), None);
    }
}
