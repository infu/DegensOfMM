//! Heap-resident active session-turn runtime state.
//!
//! This is the movement/session-turn counterpart to `BattleRuntime`. It starts
//! as an inert aggregate shell so endpoint code can be moved onto it in small,
//! testable checkpoints.

#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use domm_degens_schema::schema::{
    Champion, GameCommand, GameParticipant, GameSession, MovementIntent, WorldObject,
};
use domm_game::{ApiError, ApiEventView, CommandResponse, CommandStatusView};
use icydb::{traits::EntityValue, types::Id};

use crate::repos::{
    champions_artifacts, map_visibility_occupancy, players, sessions, towns, turn_ready,
};

pub(crate) const SESSION_TURN_RUNTIME_EVENT_SEQ_BLOCK_SIZE: u64 = 4_096;

#[derive(Clone)]
pub(crate) struct SessionTurnRuntime {
    pub session_id: String,
    pub turn_number: u32,
    pub session: Option<GameSession>,
    pub turn_started_at_ms: u64,
    pub turn_deadline_at_ms: u64,
    pub turn_duration_ms: u64,
    pub closing: bool,
    pub generation: u64,
    pub participants: Vec<SessionTurnParticipant>,
    pub ready_participants: BTreeSet<String>,
    pub champion_snapshots: Vec<Champion>,
    pub world_object_snapshots: Vec<WorldObject>,
    pub occupancy_index: Vec<RuntimeOccupancyCell>,
    pub contact_index: Vec<RuntimeContactCell>,
    pub intents: Vec<RuntimeMovementIntent>,
    pub command_receipts: Vec<SessionTurnCommandReceipt>,
    pub active_events: Vec<SessionTurnEvent>,
    pub event_seq_block: Option<SessionTurnEventSeqBlock>,
    pub champion_deltas: Vec<ChampionTurnDelta>,
    pub occupancy_deltas: Vec<OccupancyTurnDelta>,
    pub visibility_deltas: Vec<VisibilityTurnDelta>,
    pub object_deltas: Vec<ObjectTurnDelta>,
    pub resource_deltas: Vec<ResourceTurnDelta>,
    pub partial_cursor: Option<MovementCursor>,
    pub dirty: SessionTurnDirtySets,
}

impl SessionTurnRuntime {
    pub(crate) fn new(
        session_id: impl Into<String>,
        turn_number: u32,
        turn_started_at_ms: u64,
        turn_deadline_at_ms: u64,
        turn_duration_ms: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            turn_number,
            session: None,
            turn_started_at_ms,
            turn_deadline_at_ms,
            turn_duration_ms,
            closing: false,
            generation: 0,
            participants: Vec::new(),
            ready_participants: BTreeSet::new(),
            champion_snapshots: Vec::new(),
            world_object_snapshots: Vec::new(),
            occupancy_index: Vec::new(),
            contact_index: Vec::new(),
            intents: Vec::new(),
            command_receipts: Vec::new(),
            active_events: Vec::new(),
            event_seq_block: None,
            champion_deltas: Vec::new(),
            occupancy_deltas: Vec::new(),
            visibility_deltas: Vec::new(),
            object_deltas: Vec::new(),
            resource_deltas: Vec::new(),
            partial_cursor: None,
            dirty: SessionTurnDirtySets::default(),
        }
    }

    pub(crate) fn key(&self) -> String {
        runtime_key(&self.session_id, self.turn_number)
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub(crate) fn upsert_participant(&mut self, participant: SessionTurnParticipant) {
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
    }

    pub(crate) fn upsert_champion_snapshot(&mut self, champion: Champion) {
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
    }

    pub(crate) fn upsert_world_object_snapshot(&mut self, object: WorldObject) {
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
    }

    pub(crate) fn upsert_occupancy_cell(&mut self, cell: RuntimeOccupancyCell) {
        if let Some(existing) = self.occupancy_index.iter_mut().find(|existing| {
            existing.x == cell.x && existing.y == cell.y && existing.layer == cell.layer
        }) {
            *existing = cell;
        } else {
            self.occupancy_index.push(cell);
        }
        self.dirty.occupancy_index = true;
        self.mark_dirty();
    }

    pub(crate) fn upsert_occupancy_for_occupant(&mut self, cell: RuntimeOccupancyCell) {
        self.occupancy_index.retain(|existing| {
            !(existing.layer == cell.layer
                && (existing.x == cell.x && existing.y == cell.y
                    || existing.occupant_kind == cell.occupant_kind
                        && existing.occupant_id_text == cell.occupant_id_text))
        });
        self.occupancy_index.push(cell);
        self.dirty.occupancy_index = true;
        self.mark_dirty();
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
        }
    }

    pub(crate) fn upsert_contact_cell(&mut self, cell: RuntimeContactCell) {
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
    }

    pub(crate) fn mark_ready(&mut self, participant_id: impl Into<String>) {
        self.ready_participants.insert(participant_id.into());
        self.dirty.ready = true;
        self.mark_dirty();
    }

    pub(crate) fn upsert_intent(&mut self, intent: RuntimeMovementIntent) {
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
    }

    pub(crate) fn insert_command_receipt(&mut self, receipt: SessionTurnCommandReceipt) {
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
        self.active_events.push(event);
        self.dirty.events = true;
        self.mark_dirty();
    }

    pub(crate) fn push_resource_delta(&mut self, delta: ResourceTurnDelta) {
        self.resource_deltas.push(delta);
        self.dirty.resource_deltas = true;
        self.mark_dirty();
    }

    pub(crate) fn push_object_delta(&mut self, delta: ObjectTurnDelta) {
        self.object_deltas.push(delta);
        self.dirty.object_deltas = true;
        self.mark_dirty();
    }
}

#[derive(Clone)]
pub(crate) struct SessionTurnParticipant {
    pub participant_id: String,
    pub player_id: String,
    pub principal_text: Option<String>,
    pub slot_index: u8,
    pub status: String,
    pub participant: Option<GameParticipant>,
}

#[derive(Clone)]
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
pub(crate) struct RuntimeContactCell {
    pub x: u16,
    pub y: u16,
    pub subject_kind: String,
    pub subject_id_text: String,
    pub owner_participant_id: Option<String>,
    pub guarded_neutral_army_id: Option<String>,
    pub status: String,
}

#[derive(Clone)]
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
pub(crate) struct SessionTurnCommandReceipt {
    pub command_id: String,
    pub command_type: String,
    pub actor_participant_id: String,
    pub client_nonce_text: String,
    pub client_nonce: u64,
    pub payload_hash: String,
    pub response: CommandResponse,
}

impl SessionTurnCommandReceipt {
    pub(crate) fn status_view(&self) -> CommandStatusView {
        self.response.status_view()
    }
}

#[derive(Clone)]
pub(crate) struct SessionTurnEvent {
    pub command_id: Option<String>,
    pub event: ApiEventView,
    pub flushed: bool,
}

#[derive(Clone)]
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
pub(crate) struct ChampionTurnDelta {
    pub champion_id: String,
    pub x: u16,
    pub y: u16,
    pub movement_remaining: u16,
    pub status: String,
}

#[derive(Clone)]
pub(crate) struct OccupancyTurnDelta {
    pub subject_kind: String,
    pub subject_id: String,
    pub from_x: Option<u16>,
    pub from_y: Option<u16>,
    pub to_x: Option<u16>,
    pub to_y: Option<u16>,
}

#[derive(Clone)]
pub(crate) struct VisibilityTurnDelta {
    pub participant_id: String,
    pub chunk_x: u16,
    pub chunk_y: u16,
}

#[derive(Clone)]
pub(crate) struct ObjectTurnDelta {
    pub subject_kind: String,
    pub subject_id: String,
    pub x: u16,
    pub y: u16,
    pub visible: bool,
}

#[derive(Clone)]
pub(crate) struct ResourceTurnDelta {
    pub participant_id: String,
    pub gold: i64,
    pub wood: i64,
    pub stone: i64,
    pub iron: i64,
    pub crystal: i64,
    pub ember: i64,
    pub aether: i64,
}

#[derive(Clone)]
pub(crate) struct MovementCursor {
    pub consumed_steps: u32,
    pub parked_intents: u32,
}

#[derive(Clone, Default)]
pub(crate) struct SessionTurnDirtySets {
    pub participants: bool,
    pub ready: bool,
    pub champion_snapshots: bool,
    pub world_object_snapshots: bool,
    pub occupancy_index: bool,
    pub contact_index: bool,
    pub intents: bool,
    pub command_receipts: bool,
    pub events: bool,
    pub champion_deltas: bool,
    pub occupancy_deltas: bool,
    pub visibility_deltas: bool,
    pub object_deltas: bool,
    pub resource_deltas: bool,
    pub cursor: bool,
}

#[derive(Default)]
pub(crate) struct SessionTurnRuntimeSnapshot {
    pub runtimes: Vec<SessionTurnRuntime>,
}

thread_local! {
    static ACTIVE_SESSION_TURN_RUNTIMES: RefCell<BTreeMap<String, SessionTurnRuntime>> =
        RefCell::new(BTreeMap::new());
}

pub(crate) fn runtime_key(session_id: &str, turn_number: u32) -> String {
    format!("{session_id}:{turn_number}")
}

pub(crate) fn contains_runtime(session_id: &str, turn_number: u32) -> bool {
    let key = runtime_key(session_id, turn_number);
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(&key))
}

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
        let mut resource_delta = ResourceTurnDelta {
            participant_id: participant_id.to_string(),
            gold: 0,
            wood: 0,
            stone: 0,
            iron: 0,
            crystal: 0,
            ember: 0,
            aether: 0,
        };
        match resource_key {
            "gold" => resource_delta.gold = delta,
            "wood" => resource_delta.wood = delta,
            "stone" => resource_delta.stone = delta,
            "iron" => resource_delta.iron = delta,
            "crystal" => resource_delta.crystal = delta,
            "ember" => resource_delta.ember = delta,
            "aether" => resource_delta.aether = delta,
            _ => return,
        }
        runtime.push_resource_delta(resource_delta);
    });
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
    hydrate_runtime_ready_rows(session, runtime)?;
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
                runtime.upsert_champion_snapshot(champion);
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

pub(crate) fn clear_all_for_tests() {
    ACTIVE_SESSION_TURN_RUNTIMES.with(|runtimes| runtimes.borrow_mut().clear());
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
