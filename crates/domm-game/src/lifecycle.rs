use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::cleanup::ACTIVE_SESSION_LIMIT;
use crate::command::{
    CommandActor, CommandCoreError, EffectStatus, EventAudience, EventPage, GameCommandPayload,
    GameEventDraft, LobbyCommandJournal, LobbyCommandPayload, SessionCommandJournal,
};
use crate::content::{
    FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT, FIRST_PLAYABLE_MAP_WIDTH,
    FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION,
    ResourceCost, first_playable_content_manifest, first_playable_scenario,
};
use crate::driver::{ActiveMatchView, HeadlessBackend, PlayerView, SessionView};
use crate::fixtures::{ScenarioFixture, TURN_DURATION_MS, first_playable_fixture};

const MAX_PLAYERS_PER_SESSION: usize = 2;
const SETUP_SYSTEM_ACTOR: &str = "setup";

const SETUP_EFFECTS: &[SetupEffectSpec] = &[
    SetupEffectSpec {
        key: "seed_ruleset_content",
        effect_type: "ruleset_content",
        target_kind: "ruleset",
    },
    SetupEffectSpec {
        key: "seed_participants",
        effect_type: "participants",
        target_kind: "participant",
    },
    SetupEffectSpec {
        key: "seed_towns",
        effect_type: "towns",
        target_kind: "town",
    },
    SetupEffectSpec {
        key: "seed_champions",
        effect_type: "champions",
        target_kind: "champion",
    },
    SetupEffectSpec {
        key: "seed_map_chunks",
        effect_type: "map_chunks",
        target_kind: "map_chunk",
    },
    SetupEffectSpec {
        key: "seed_occupancy",
        effect_type: "occupancy",
        target_kind: "map_occupancy",
    },
    SetupEffectSpec {
        key: "seed_visibility",
        effect_type: "visibility",
        target_kind: "visibility_chunk",
    },
];

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ParticipantView {
    pub participant_id: String,
    pub session_id: String,
    pub player_id: String,
    pub faction_slug: String,
    pub slot_index: u8,
    pub status: String,
    pub ready: bool,
    pub resources: ResourceCost,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MatchHistoryEntry {
    pub session_id: String,
    pub result: String,
    pub opponent_name: Option<String>,
    pub turns_played: u32,
    pub summary_json: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SetupProjection {
    pub content_rows_seeded: u32,
    pub participant_rows_seeded: u32,
    pub town_rows_seeded: u32,
    pub champion_rows_seeded: u32,
    pub map_chunk_rows_seeded: u32,
    pub occupancy_rows_seeded: u32,
    pub visibility_chunk_rows_seeded: u32,
    pub setup_event_seq: Option<u64>,
    pub completed_effect_keys: Vec<String>,
}

impl SetupProjection {
    #[must_use]
    pub fn is_complete_for(&self, participant_count: usize) -> bool {
        let expected = SetupRequirements::for_participants(participant_count);
        self.content_rows_seeded == expected.content_rows
            && self.participant_rows_seeded == expected.participant_rows
            && self.town_rows_seeded == expected.town_rows
            && self.champion_rows_seeded == expected.champion_rows
            && self.map_chunk_rows_seeded == expected.map_chunk_rows
            && self.occupancy_rows_seeded == expected.occupancy_rows
            && self.visibility_chunk_rows_seeded == expected.visibility_chunk_rows
            && self.setup_event_seq.is_some()
            && SETUP_EFFECTS.iter().all(|effect| {
                self.completed_effect_keys
                    .iter()
                    .any(|key| key == effect.key)
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SetupRequirements {
    content_rows: u32,
    participant_rows: u32,
    town_rows: u32,
    champion_rows: u32,
    map_chunk_rows: u32,
    occupancy_rows: u32,
    visibility_chunk_rows: u32,
}

impl SetupRequirements {
    fn for_participants(participant_count: usize) -> Self {
        let manifest = first_playable_content_manifest();
        let scenario = first_playable_scenario();
        let content_rows = 1
            + manifest.factions.len()
            + manifest.champion_classes.len()
            + manifest.terrain.len()
            + manifest.units.len()
            + manifest.buildings.len()
            + manifest.spells.len()
            + manifest.artifacts.len()
            + manifest.map_objects.len();
        let chunks_x =
            u32::from(FIRST_PLAYABLE_MAP_WIDTH).div_ceil(u32::from(FIRST_PLAYABLE_CHUNK_SIZE));
        let chunks_y =
            u32::from(FIRST_PLAYABLE_MAP_HEIGHT).div_ceil(u32::from(FIRST_PLAYABLE_CHUNK_SIZE));
        let map_chunk_rows = chunks_x * chunks_y;
        let town_rows = scenario.starts.len() as u32;
        let champion_rows = scenario.starts.len() as u32;
        let occupancy_rows = town_rows
            + champion_rows
            + scenario.mines.len() as u32
            + scenario.resource_piles.len() as u32
            + scenario.central_objectives.len() as u32
            + scenario.neutral_armies.len() as u32;

        Self {
            content_rows: content_rows as u32,
            participant_rows: participant_count as u32,
            town_rows,
            champion_rows,
            map_chunk_rows,
            occupancy_rows,
            visibility_chunk_rows: participant_count as u32 * map_chunk_rows,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SetupEffectSpec {
    key: &'static str,
    effect_type: &'static str,
    target_kind: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlayerRecord {
    view: PlayerView,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionRecord {
    session_id: String,
    created_by_player_id: String,
    state: String,
    scenario_seed: String,
    participant_ids: Vec<String>,
    current_turn: u32,
    setup_command_id: Option<String>,
    setup_projection: SetupProjection,
}

impl SessionRecord {
    fn view(&self) -> SessionView {
        SessionView {
            session_id: self.session_id.clone(),
            state: self.state.clone(),
            participant_ids: self.participant_ids.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParticipantRecord {
    view: ParticipantView,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LobbyCommandResult {
    command_id: String,
    result_kind: LobbyResultKind,
    subject_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LobbyResultKind {
    RegisterPlayer,
    CreateSession,
    JoinSession,
    MarkReady,
    StartSession,
}

#[derive(Clone, Debug)]
pub struct LifecycleBackend {
    fixture: ScenarioFixture,
    lobby_journal: LobbyCommandJournal,
    session_journals: Vec<(String, SessionCommandJournal)>,
    players: Vec<PlayerRecord>,
    sessions: Vec<SessionRecord>,
    participants: Vec<ParticipantRecord>,
    match_history: Vec<(String, MatchHistoryEntry)>,
    lobby_results: Vec<LobbyCommandResult>,
    setup_trap_after_effects: Option<usize>,
}

impl Default for LifecycleBackend {
    fn default() -> Self {
        Self::new(first_playable_fixture())
    }
}

impl LifecycleBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            lobby_journal: LobbyCommandJournal::new(fixture.clock.start_timestamp_ms),
            fixture,
            session_journals: Vec::new(),
            players: Vec::new(),
            sessions: Vec::new(),
            participants: Vec::new(),
            match_history: Vec::new(),
            lobby_results: Vec::new(),
            setup_trap_after_effects: None,
        }
    }

    #[must_use]
    pub fn lobby_commands(&self) -> usize {
        self.lobby_journal.commands().len()
    }

    #[must_use]
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    #[must_use]
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    #[must_use]
    pub fn setup_projection(&self, session_id: &str) -> Option<&SetupProjection> {
        self.session(session_id)
            .ok()
            .map(|session| &session.setup_projection)
    }

    #[must_use]
    pub fn session_command_journal(&self, session_id: &str) -> Option<&SessionCommandJournal> {
        self.session_journals
            .iter()
            .find(|(id, _)| id == session_id)
            .map(|(_, journal)| journal)
    }

    pub fn set_setup_trap_after_effects(&mut self, effects: Option<usize>) {
        self.setup_trap_after_effects = effects;
    }

    pub fn get_my_player(&self, caller: Principal) -> Result<PlayerView, LifecycleError> {
        self.player_for_principal(caller)
            .map(|player| player.view.clone())
    }

    pub fn get_session(&self, session_id: &str) -> Result<SessionView, LifecycleError> {
        Ok(self.session(session_id)?.view())
    }

    pub fn get_my_participant(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ParticipantView, LifecycleError> {
        let player = self.player_for_principal(caller)?;
        self.participant_for_player(session_id, &player.view.player_id)
            .map(|participant| participant.view.clone())
    }

    pub fn get_match_history(
        &self,
        caller: Principal,
        cursor: usize,
        limit: usize,
    ) -> Result<Vec<MatchHistoryEntry>, LifecycleError> {
        let player = self.player_for_principal(caller)?;
        Ok(self
            .match_history
            .iter()
            .filter(|(player_id, _)| player_id == &player.view.player_id)
            .skip(cursor)
            .take(limit)
            .map(|(_, entry)| entry.clone())
            .collect())
    }

    pub fn get_events(
        &self,
        caller: Principal,
        session_id: &str,
        events_after_seq: u64,
        limit: usize,
    ) -> Result<EventPage, LifecycleError> {
        let player = self.player_for_principal(caller)?;
        let participant = self.participant_for_player(session_id, &player.view.player_id)?;
        let audience = EventAudience::participant(participant.view.participant_id.clone());
        let journal = self.session_command_journal(session_id).ok_or_else(|| {
            LifecycleError::SessionNotFound {
                session_id: session_id.to_string(),
            }
        })?;

        Ok(journal.event_page_after_seq(events_after_seq, Some(&audience), limit))
    }

    pub fn cancel_session(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<SessionView, LifecycleError> {
        let player_id = self.player_for_principal(caller)?.view.player_id.clone();
        let session = self.session_mut(session_id)?;
        if session.created_by_player_id != player_id {
            return Err(LifecycleError::NotSessionCreator);
        }
        if session.state == "active" {
            return Err(LifecycleError::SessionAlreadyActive);
        }
        session.state = "cancelled".to_string();
        Ok(session.view())
    }

    pub fn leave_session(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<SessionView, LifecycleError> {
        let player = self.player_for_principal(caller)?.clone();
        let participant = self
            .participant_for_player(session_id, &player.view.player_id)?
            .view
            .clone();
        if self.session(session_id)?.state == "active" {
            return Err(LifecycleError::SessionAlreadyActive);
        }
        self.session_mut(session_id)?
            .participant_ids
            .retain(|participant_id| participant_id != &participant.participant_id);
        self.participants
            .retain(|record| record.view.participant_id != participant.participant_id);
        Ok(self.session(session_id)?.view())
    }

    pub fn recover_session_setup(
        &mut self,
        session_id: &str,
    ) -> Result<SessionView, LifecycleError> {
        let command_id = self
            .session(session_id)?
            .setup_command_id
            .clone()
            .ok_or(LifecycleError::SetupNotStarted)?;
        self.run_setup(session_id, &command_id, None)
    }

    fn submit_lobby_command(
        &mut self,
        caller: Principal,
        player_id: Option<String>,
        client_nonce: &str,
        command_type: &str,
        payload_json: String,
    ) -> Result<(String, bool), LifecycleError> {
        let payload = LobbyCommandPayload::from_json(
            caller,
            player_id,
            client_nonce_u64(command_type, client_nonce),
            command_type,
            payload_json,
        );
        let outcome = self.lobby_journal.submit_command(payload)?;
        Ok((outcome.command.id, outcome.duplicate))
    }

    fn remember_lobby_result(
        &mut self,
        command_id: &str,
        result_kind: LobbyResultKind,
        subject_id: &str,
    ) {
        if self
            .lobby_results
            .iter()
            .any(|result| result.command_id == command_id)
        {
            return;
        }

        self.lobby_results.push(LobbyCommandResult {
            command_id: command_id.to_string(),
            result_kind,
            subject_id: subject_id.to_string(),
        });
    }

    fn lobby_result(
        &self,
        command_id: &str,
        result_kind: LobbyResultKind,
    ) -> Option<&LobbyCommandResult> {
        self.lobby_results
            .iter()
            .find(|result| result.command_id == command_id && result.result_kind == result_kind)
    }

    fn register_player_internal(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, LifecycleError> {
        let payload_json = format!(
            "{{\"principal\":\"{}\",\"display_name\":\"{}\"}}",
            caller.to_text(),
            escape_json(display_name)
        );
        let (command_id, duplicate) =
            self.submit_lobby_command(caller, None, client_nonce, "register_player", payload_json)?;
        if duplicate {
            if let Some(result) = self.lobby_result(&command_id, LobbyResultKind::RegisterPlayer) {
                return Ok(self
                    .players
                    .iter()
                    .find(|player| player.view.player_id == result.subject_id)
                    .expect("lobby result should point at existing player")
                    .view
                    .clone());
            }
        }

        if let Some(existing) = self
            .players
            .iter()
            .find(|player| player.view.principal == caller)
            .cloned()
        {
            self.lobby_journal.mark_command_applied(
                &command_id,
                Some(format!(
                    "{{\"player_id\":\"{}\",\"duplicate\":true}}",
                    escape_json(&existing.view.player_id)
                )),
            )?;
            self.remember_lobby_result(
                &command_id,
                LobbyResultKind::RegisterPlayer,
                &existing.view.player_id,
            );
            return Ok(existing.view.clone());
        }

        let player = PlayerView {
            player_id: self.player_id_for_principal(caller),
            display_name: display_name.to_string(),
            principal: caller,
        };
        self.players.push(PlayerRecord {
            view: player.clone(),
        });
        self.lobby_journal.mark_command_applied(
            &command_id,
            Some(format!(
                "{{\"player_id\":\"{}\",\"duplicate\":false}}",
                escape_json(&player.player_id)
            )),
        )?;
        self.remember_lobby_result(
            &command_id,
            LobbyResultKind::RegisterPlayer,
            &player.player_id,
        );
        Ok(player)
    }

    fn create_session_internal(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, LifecycleError> {
        if scenario_seed != self.fixture.scenario_seed {
            return Err(LifecycleError::UnknownScenarioSeed);
        }
        let player = self.player_for_principal(caller)?.clone();
        let payload_json = format!(
            "{{\"principal\":\"{}\",\"scenario_seed\":\"{}\"}}",
            caller.to_text(),
            escape_json(scenario_seed)
        );
        let (command_id, duplicate) = self.submit_lobby_command(
            caller,
            Some(player.view.player_id.clone()),
            client_nonce,
            "create_session",
            payload_json,
        )?;
        if duplicate {
            if let Some(result) = self.lobby_result(&command_id, LobbyResultKind::CreateSession) {
                return Ok(self.session(&result.subject_id)?.view());
            }
        }

        if self.player_has_active_session(&player.view.player_id) {
            self.lobby_journal.mark_command_failed(
                &command_id,
                "active_session_limit",
                "player already has an active or lobby session",
                false,
            )?;
            return Err(LifecycleError::ActiveSessionLimit);
        }
        if self.active_session_count() >= ACTIVE_SESSION_LIMIT as usize {
            self.lobby_journal.mark_command_failed(
                &command_id,
                "canister_active_session_limit_reached",
                "canister active session limit reached",
                false,
            )?;
            return Err(LifecycleError::ActiveSessionLimit);
        }

        let session_id = if self.sessions.is_empty() {
            self.fixture.ids.session_id.clone()
        } else {
            format!("session:{:06}", self.sessions.len() + 1)
        };
        let participant_id = self.participant_id_for_slot(&session_id, 0, &player.view.player_id);
        let session = SessionRecord {
            session_id: session_id.clone(),
            created_by_player_id: player.view.player_id.clone(),
            state: "lobby".to_string(),
            scenario_seed: scenario_seed.to_string(),
            participant_ids: vec![participant_id.clone()],
            current_turn: 1,
            setup_command_id: None,
            setup_projection: SetupProjection::default(),
        };
        self.sessions.push(session);
        self.participants.push(ParticipantRecord {
            view: ParticipantView {
                participant_id: participant_id.clone(),
                session_id: session_id.clone(),
                player_id: player.view.player_id.clone(),
                faction_slug: first_playable_scenario().starts[0].faction_slug.clone(),
                slot_index: 0,
                status: "active".to_string(),
                ready: false,
                resources: ResourceCost::starting_resources(),
            },
        });
        self.session_journals.push((
            session_id.clone(),
            SessionCommandJournal::new(session_id.clone(), self.fixture.clock.start_timestamp_ms),
        ));
        self.lobby_journal.mark_command_applied(
            &command_id,
            Some(format!(
                "{{\"session_id\":\"{}\"}}",
                escape_json(&session_id)
            )),
        )?;
        self.remember_lobby_result(&command_id, LobbyResultKind::CreateSession, &session_id);
        Ok(self.session(&session_id)?.view())
    }

    fn join_session_internal(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, LifecycleError> {
        let player = self.player_for_principal(caller)?.clone();
        let payload_json = format!(
            "{{\"principal\":\"{}\",\"session_id\":\"{}\"}}",
            caller.to_text(),
            escape_json(session_id)
        );
        let (command_id, duplicate) = self.submit_lobby_command(
            caller,
            Some(player.view.player_id.clone()),
            client_nonce,
            "join_session",
            payload_json,
        )?;
        if duplicate {
            if let Some(result) = self.lobby_result(&command_id, LobbyResultKind::JoinSession) {
                let participant = self
                    .participants
                    .iter()
                    .find(|participant| participant.view.participant_id == result.subject_id)
                    .expect("join result should point at existing participant");
                return Ok(self.session(&participant.view.session_id)?.view());
            }
        }

        if self.player_has_active_session(&player.view.player_id) {
            self.lobby_journal.mark_command_failed(
                &command_id,
                "active_session_limit",
                "player already has an active or lobby session",
                false,
            )?;
            return Err(LifecycleError::ActiveSessionLimit);
        }

        let participant_count = self.session(session_id)?.participant_ids.len();
        if self.session(session_id)?.state != "lobby" {
            return Err(LifecycleError::SessionNotJoinable);
        }
        if participant_count >= MAX_PLAYERS_PER_SESSION {
            return Err(LifecycleError::PlayerCapReached);
        }

        let slot_index = participant_count as u8;
        let participant_id =
            self.participant_id_for_slot(session_id, slot_index, &player.view.player_id);
        let faction_slug = first_playable_scenario().starts[slot_index as usize]
            .faction_slug
            .clone();
        self.participants.push(ParticipantRecord {
            view: ParticipantView {
                participant_id: participant_id.clone(),
                session_id: session_id.to_string(),
                player_id: player.view.player_id.clone(),
                faction_slug,
                slot_index,
                status: "active".to_string(),
                ready: false,
                resources: ResourceCost::starting_resources(),
            },
        });
        self.session_mut(session_id)?
            .participant_ids
            .push(participant_id.clone());
        self.lobby_journal.mark_command_applied(
            &command_id,
            Some(format!(
                "{{\"session_id\":\"{}\",\"participant_id\":\"{}\"}}",
                escape_json(session_id),
                escape_json(&participant_id)
            )),
        )?;
        self.remember_lobby_result(&command_id, LobbyResultKind::JoinSession, &participant_id);
        Ok(self.session(session_id)?.view())
    }

    fn mark_ready_internal(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, LifecycleError> {
        let player = self.player_for_principal(caller)?.clone();
        let participant_id = self
            .participant_for_player(session_id, &player.view.player_id)?
            .view
            .participant_id
            .clone();
        let payload_json = format!(
            "{{\"principal\":\"{}\",\"session_id\":\"{}\"}}",
            caller.to_text(),
            escape_json(session_id)
        );
        let (command_id, duplicate) = self.submit_lobby_command(
            caller,
            Some(player.view.player_id),
            client_nonce,
            "mark_ready",
            payload_json,
        )?;
        if duplicate {
            if let Some(result) = self.lobby_result(&command_id, LobbyResultKind::MarkReady) {
                return Ok(self.session(&result.subject_id)?.view());
            }
        }

        if self.session(session_id)?.state != "lobby" {
            return Err(LifecycleError::SessionNotJoinable);
        }
        self.participant_mut(&participant_id)?.view.ready = true;
        self.lobby_journal.mark_command_applied(
            &command_id,
            Some(format!(
                "{{\"session_id\":\"{}\",\"participant_id\":\"{}\"}}",
                escape_json(session_id),
                escape_json(&participant_id)
            )),
        )?;
        self.remember_lobby_result(&command_id, LobbyResultKind::MarkReady, session_id);
        Ok(self.session(session_id)?.view())
    }

    fn start_session_internal(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, LifecycleError> {
        let player = self.player_for_principal(caller)?.clone();
        let payload_json = format!(
            "{{\"principal\":\"{}\",\"session_id\":\"{}\"}}",
            caller.to_text(),
            escape_json(session_id)
        );
        let (command_id, duplicate) = self.submit_lobby_command(
            caller,
            Some(player.view.player_id.clone()),
            client_nonce,
            "start_session",
            payload_json,
        )?;
        if duplicate {
            if let Some(result) = self.lobby_result(&command_id, LobbyResultKind::StartSession) {
                return Ok(self.session(&result.subject_id)?.view());
            }
        }

        let session = self.session(session_id)?.clone();
        if session.created_by_player_id != player.view.player_id {
            return Err(LifecycleError::NotSessionCreator);
        }
        if session.state == "active" {
            return Ok(session.view());
        }
        if session.participant_ids.len() != MAX_PLAYERS_PER_SESSION {
            return Err(LifecycleError::PlayerCapReached);
        }
        if !self
            .participants_for_session(session_id)
            .all(|participant| participant.view.ready)
        {
            return Err(LifecycleError::ParticipantsNotReady);
        }

        self.session_mut(session_id)?.state = "starting".to_string();
        let setup_command_id = self.ensure_setup_command(session_id)?;
        match self.run_setup(session_id, &setup_command_id, self.setup_trap_after_effects) {
            Ok(view) => {
                self.lobby_journal.mark_command_applied(
                    &command_id,
                    Some(format!(
                        "{{\"session_id\":\"{}\",\"setup_complete\":true}}",
                        escape_json(session_id)
                    )),
                )?;
                self.remember_lobby_result(&command_id, LobbyResultKind::StartSession, session_id);
                Ok(view)
            }
            Err(LifecycleError::SetupInterrupted) => {
                self.lobby_journal.mark_command_failed(
                    &command_id,
                    "setup_interrupted",
                    "setup stopped before all idempotent phases completed",
                    true,
                )?;
                Err(LifecycleError::SetupInterrupted)
            }
            Err(error) => Err(error),
        }
    }

    fn ensure_setup_command(&mut self, session_id: &str) -> Result<String, LifecycleError> {
        if let Some(command_id) = self.session(session_id)?.setup_command_id.clone() {
            return Ok(command_id);
        }
        let scenario = first_playable_scenario();
        let actor = CommandActor::system(SETUP_SYSTEM_ACTOR);
        let payload = GameCommandPayload::from_json(
            session_id.to_string(),
            actor,
            1,
            client_nonce_u64("setup", session_id),
            "setup_session",
            format!(
                "{{\"scenario_hash\":\"{}\",\"ruleset\":\"{}\"}}",
                scenario.scenario_hash, FIRST_PLAYABLE_RULESET_ID
            ),
        );
        let command = self
            .session_journal_mut(session_id)?
            .submit_command(payload)?
            .command;
        self.session_journal_mut(session_id)?
            .begin_apply(&command.id)?;
        self.session_mut(session_id)?.setup_command_id = Some(command.id.clone());
        Ok(command.id)
    }

    fn run_setup(
        &mut self,
        session_id: &str,
        setup_command_id: &str,
        trap_after_effects: Option<usize>,
    ) -> Result<SessionView, LifecycleError> {
        let mut applied_this_call = 0usize;
        for effect in SETUP_EFFECTS {
            if self
                .session(session_id)?
                .setup_projection
                .completed_effect_keys
                .iter()
                .any(|key| key == effect.key)
            {
                continue;
            }
            if trap_after_effects == Some(applied_this_call) {
                return Err(LifecycleError::SetupInterrupted);
            }
            self.apply_setup_effect(session_id, setup_command_id, effect)?;
            applied_this_call += 1;
        }

        self.ensure_setup_event(session_id, setup_command_id)?;
        let participant_count = self.session(session_id)?.participant_ids.len();
        if !self
            .session(session_id)?
            .setup_projection
            .is_complete_for(participant_count)
        {
            return Err(LifecycleError::SetupIncomplete);
        }
        self.session_journal_mut(session_id)?.mark_command_applied(
            setup_command_id,
            Some("{\"setup_complete\":true}".to_string()),
        )?;
        self.session_mut(session_id)?.state = "active".to_string();
        Ok(self.session(session_id)?.view())
    }

    fn apply_setup_effect(
        &mut self,
        session_id: &str,
        setup_command_id: &str,
        effect: &SetupEffectSpec,
    ) -> Result<(), LifecycleError> {
        let requirements =
            SetupRequirements::for_participants(self.session(session_id)?.participant_ids.len());
        let record = self
            .session_journal_mut(session_id)?
            .ensure_command_effect(
                setup_command_id,
                effect.key,
                effect.effect_type,
                effect.target_kind,
                session_id.to_string(),
                "{}",
            )?;
        if record.status != EffectStatus::Applied {
            self.session_journal_mut(session_id)?
                .mark_command_effect_applied(setup_command_id, effect.key)?;
        }
        let projection = &mut self.session_mut(session_id)?.setup_projection;
        match effect.key {
            "seed_ruleset_content" => projection.content_rows_seeded = requirements.content_rows,
            "seed_participants" => {
                projection.participant_rows_seeded = requirements.participant_rows;
            }
            "seed_towns" => projection.town_rows_seeded = requirements.town_rows,
            "seed_champions" => projection.champion_rows_seeded = requirements.champion_rows,
            "seed_map_chunks" => projection.map_chunk_rows_seeded = requirements.map_chunk_rows,
            "seed_occupancy" => projection.occupancy_rows_seeded = requirements.occupancy_rows,
            "seed_visibility" => {
                projection.visibility_chunk_rows_seeded = requirements.visibility_chunk_rows;
            }
            _ => {}
        }
        if !projection
            .completed_effect_keys
            .iter()
            .any(|key| key == effect.key)
        {
            projection
                .completed_effect_keys
                .push(effect.key.to_string());
        }
        Ok(())
    }

    fn ensure_setup_event(
        &mut self,
        session_id: &str,
        setup_command_id: &str,
    ) -> Result<(), LifecycleError> {
        if self
            .session(session_id)?
            .setup_projection
            .setup_event_seq
            .is_some()
        {
            return Ok(());
        }
        let event = self
            .session_journal_mut(session_id)?
            .append_event(GameEventDraft {
                session_id: session_id.to_string(),
                command_id: Some(setup_command_id.to_string()),
                actor_participant_id: None,
                turn_number: 1,
                event_key: "setup:complete".to_string(),
                audience: EventAudience::Public,
                event_type: "session_started".to_string(),
                subject_kind: Some("session".to_string()),
                subject_id_text: Some(session_id.to_string()),
                payload_json: format!(
                    "{{\"ruleset\":\"{}\",\"version\":{},\"map_width\":{},\"map_height\":{}}}",
                    FIRST_PLAYABLE_RULESET_SLUG,
                    FIRST_PLAYABLE_RULESET_VERSION,
                    FIRST_PLAYABLE_MAP_WIDTH,
                    FIRST_PLAYABLE_MAP_HEIGHT
                ),
            })?
            .event;
        self.session_mut(session_id)?
            .setup_projection
            .setup_event_seq = Some(event.event_seq);
        Ok(())
    }

    fn player_for_principal(&self, principal: Principal) -> Result<&PlayerRecord, LifecycleError> {
        self.players
            .iter()
            .find(|player| player.view.principal == principal)
            .ok_or(LifecycleError::PlayerNotRegistered)
    }

    fn session(&self, session_id: &str) -> Result<&SessionRecord, LifecycleError> {
        self.sessions
            .iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| LifecycleError::SessionNotFound {
                session_id: session_id.to_string(),
            })
    }

    fn session_mut(&mut self, session_id: &str) -> Result<&mut SessionRecord, LifecycleError> {
        self.sessions
            .iter_mut()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| LifecycleError::SessionNotFound {
                session_id: session_id.to_string(),
            })
    }

    fn participant_for_player(
        &self,
        session_id: &str,
        player_id: &str,
    ) -> Result<&ParticipantRecord, LifecycleError> {
        self.participants
            .iter()
            .find(|participant| {
                participant.view.session_id == session_id && participant.view.player_id == player_id
            })
            .ok_or(LifecycleError::ParticipantNotFound)
    }

    fn participant_mut(
        &mut self,
        participant_id: &str,
    ) -> Result<&mut ParticipantRecord, LifecycleError> {
        self.participants
            .iter_mut()
            .find(|participant| participant.view.participant_id == participant_id)
            .ok_or(LifecycleError::ParticipantNotFound)
    }

    fn participants_for_session(
        &self,
        session_id: &str,
    ) -> impl Iterator<Item = &ParticipantRecord> {
        self.participants
            .iter()
            .filter(move |participant| participant.view.session_id == session_id)
    }

    fn session_journal_mut(
        &mut self,
        session_id: &str,
    ) -> Result<&mut SessionCommandJournal, LifecycleError> {
        self.session_journals
            .iter_mut()
            .find(|(id, _)| id == session_id)
            .map(|(_, journal)| journal)
            .ok_or_else(|| LifecycleError::SessionNotFound {
                session_id: session_id.to_string(),
            })
    }

    fn player_has_active_session(&self, player_id: &str) -> bool {
        self.participants.iter().any(|participant| {
            participant.view.player_id == player_id
                && self.sessions.iter().any(|session| {
                    session.session_id == participant.view.session_id
                        && matches!(session.state.as_str(), "lobby" | "starting" | "active")
                })
        })
    }

    fn active_session_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|session| matches!(session.state.as_str(), "lobby" | "starting" | "active"))
            .count()
    }

    fn player_id_for_principal(&self, principal: Principal) -> String {
        if principal == self.fixture.principals.player_one {
            self.fixture.ids.player_one_id.clone()
        } else if principal == self.fixture.principals.player_two {
            self.fixture.ids.player_two_id.clone()
        } else {
            format!("player:{}", short_hash(&principal.to_text()))
        }
    }

    fn participant_id_for_slot(&self, session_id: &str, slot_index: u8, player_id: &str) -> String {
        if session_id == self.fixture.ids.session_id && slot_index == 0 {
            self.fixture.ids.participant_one_id.clone()
        } else if session_id == self.fixture.ids.session_id && slot_index == 1 {
            self.fixture.ids.participant_two_id.clone()
        } else {
            format!(
                "participant:{session_id}:{slot_index}:{}",
                short_hash(player_id)
            )
        }
    }
}

impl HeadlessBackend for LifecycleBackend {
    type Error = LifecycleError;

    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, Self::Error> {
        self.register_player_internal(caller, display_name, client_nonce)
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, Self::Error> {
        self.create_session_internal(caller, client_nonce, scenario_seed)
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.join_session_internal(caller, session_id, client_nonce)
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.mark_ready_internal(caller, session_id, client_nonce)
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.start_session_internal(caller, session_id, client_nonce)
    }

    fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, Self::Error> {
        let player = self.player_for_principal(caller)?;
        self.participant_for_player(session_id, &player.view.player_id)?;
        let session = self.session(session_id)?;
        if session.state != "active" {
            return Err(LifecycleError::SessionNotActive);
        }

        Ok(ActiveMatchView {
            session_id: session_id.to_string(),
            current_turn: session.current_turn,
            turn_duration_ms: TURN_DURATION_MS,
            sync_required: false,
            visible_participant_ids: session.participant_ids.clone(),
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LifecycleError {
    #[error(transparent)]
    Command(#[from] CommandCoreError),
    #[error("player is not registered")]
    PlayerNotRegistered,
    #[error("participant not found")]
    ParticipantNotFound,
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("unknown scenario seed")]
    UnknownScenarioSeed,
    #[error("active session limit reached")]
    ActiveSessionLimit,
    #[error("session cannot be joined")]
    SessionNotJoinable,
    #[error("session is not active")]
    SessionNotActive,
    #[error("session is already active")]
    SessionAlreadyActive,
    #[error("player cap reached")]
    PlayerCapReached,
    #[error("caller is not the session creator")]
    NotSessionCreator,
    #[error("participants are not ready")]
    ParticipantsNotReady,
    #[error("setup was interrupted")]
    SetupInterrupted,
    #[error("setup did not complete all required rows/events")]
    SetupIncomplete,
    #[error("setup has not started")]
    SetupNotStarted,
}

fn client_nonce_u64(scope: &str, client_nonce: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(client_nonce.as_bytes());
    let digest = hasher.finalize();
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(raw)
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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
    use candid::Principal;

    use super::{LifecycleBackend, LifecycleError, SETUP_EFFECTS};
    use crate::driver::{HeadlessBackend, HeadlessGameDriver};
    use crate::fixtures::first_playable_fixture;

    #[test]
    fn lifecycle_backend_passes_gate_a_headless_smoke() {
        let fixture = first_playable_fixture();
        let backend = LifecycleBackend::new(fixture.clone());
        let mut driver = HeadlessGameDriver::new(backend, fixture.clone());

        let view = driver
            .create_join_start_inspect()
            .expect("gate A lifecycle path should succeed");
        let backend = driver.into_backend();
        let setup = backend
            .setup_projection(&fixture.ids.session_id)
            .expect("setup projection should exist");

        assert_eq!(view.session_id, fixture.ids.session_id);
        assert_eq!(view.current_turn, 1);
        assert_eq!(
            view.visible_participant_ids,
            vec![
                fixture.ids.participant_one_id,
                fixture.ids.participant_two_id
            ]
        );
        assert!(setup.is_complete_for(2));
        assert_eq!(backend.lobby_commands(), 7);
    }

    #[test]
    fn duplicate_lobby_commands_return_existing_rows() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        let first = backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("first registration should succeed");
        let retry = backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("duplicate registration should return existing player");

        assert_eq!(first, retry);
        assert_eq!(backend.player_count(), 1);
        assert_eq!(backend.lobby_commands(), 1);
    }

    #[test]
    fn duplicate_lobby_nonce_payload_mismatch_is_rejected() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("first registration should succeed");

        let error = backend
            .register_player(
                fixture.principals.player_one,
                "Different",
                &fixture.command_nonces.register_player_one,
            )
            .expect_err("same nonce with different payload should fail");

        assert!(matches!(
            error,
            LifecycleError::Command(
                crate::command::CommandCoreError::LobbyDuplicateNoncePayloadMismatch { .. }
            )
        ));
    }

    #[test]
    fn invalid_callers_and_session_state_are_rejected() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("p1 registers");
        backend
            .register_player(
                fixture.principals.player_two,
                "Mayhem Two",
                &fixture.command_nonces.register_player_two,
            )
            .expect("p2 registers");
        let session = backend
            .create_session(
                fixture.principals.player_one,
                &fixture.command_nonces.create_session,
                &fixture.scenario_seed,
            )
            .expect("session is created");
        backend
            .join_session(
                fixture.principals.player_two,
                &session.session_id,
                &fixture.command_nonces.join_session,
            )
            .expect("p2 joins");

        assert_eq!(
            backend
                .start_session(
                    fixture.principals.player_two,
                    &session.session_id,
                    "nonce:start:not-owner",
                )
                .expect_err("non-creator cannot start"),
            LifecycleError::NotSessionCreator
        );
        assert_eq!(
            backend
                .start_session(
                    fixture.principals.player_one,
                    &session.session_id,
                    &fixture.command_nonces.start_session,
                )
                .expect_err("not-ready participants cannot start"),
            LifecycleError::ParticipantsNotReady
        );
        assert_eq!(
            backend
                .get_game_view(fixture.principals.player_one, &session.session_id)
                .expect_err("lobby session is not active"),
            LifecycleError::SessionNotActive
        );
    }

    #[test]
    fn player_cap_and_active_session_limit_are_enforced() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        let third = Principal::self_authenticating(&[0x33; 32]);

        backend
            .register_player(fixture.principals.player_one, "Misery One", "n:p1")
            .expect("p1 registers");
        backend
            .register_player(fixture.principals.player_two, "Mayhem Two", "n:p2")
            .expect("p2 registers");
        backend
            .register_player(third, "Third", "n:p3")
            .expect("third registers");
        let session = backend
            .create_session(
                fixture.principals.player_one,
                "n:create",
                &fixture.scenario_seed,
            )
            .expect("session is created");
        assert_eq!(
            backend
                .create_session(
                    fixture.principals.player_one,
                    "n:create2",
                    &fixture.scenario_seed
                )
                .expect_err("creator cannot open another active/lobby session"),
            LifecycleError::ActiveSessionLimit
        );
        backend
            .join_session(
                fixture.principals.player_two,
                &session.session_id,
                "n:join:p2",
            )
            .expect("p2 joins");
        assert_eq!(
            backend
                .join_session(third, &session.session_id, "n:join:p3")
                .expect_err("third player cannot exceed cap"),
            LifecycleError::PlayerCapReached
        );
    }

    #[test]
    fn canister_active_session_limit_is_enforced() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());

        for index in 0..crate::cleanup::ACTIVE_SESSION_LIMIT {
            let caller = synthetic_principal(index as u16);
            backend
                .register_player(
                    caller,
                    &format!("Player {index}"),
                    &format!("nonce:r:{index}"),
                )
                .expect("registration should fit under active session test");
            backend
                .create_session(caller, &format!("nonce:c:{index}"), &fixture.scenario_seed)
                .expect("session should fit under active session cap");
        }

        let extra = synthetic_principal(crate::cleanup::ACTIVE_SESSION_LIMIT as u16 + 1);
        backend
            .register_player(extra, "Extra", "nonce:r:extra")
            .expect("extra player registration should still be allowed");
        let error = backend
            .create_session(extra, "nonce:c:extra", &fixture.scenario_seed)
            .expect_err("101st active session should be rejected");

        assert!(matches!(error, LifecycleError::ActiveSessionLimit));
        assert_eq!(
            backend.active_session_count(),
            crate::cleanup::ACTIVE_SESSION_LIMIT as usize
        );
    }

    #[test]
    fn setup_recovery_finishes_interrupted_setup_without_duplicate_effects() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        backend
            .register_player(fixture.principals.player_one, "Misery One", "n:p1")
            .expect("p1 registers");
        backend
            .register_player(fixture.principals.player_two, "Mayhem Two", "n:p2")
            .expect("p2 registers");
        let session = backend
            .create_session(
                fixture.principals.player_one,
                "n:create",
                &fixture.scenario_seed,
            )
            .expect("session is created");
        backend
            .join_session(fixture.principals.player_two, &session.session_id, "n:join")
            .expect("p2 joins");
        backend
            .mark_ready(
                fixture.principals.player_one,
                &session.session_id,
                "n:ready:p1",
            )
            .expect("p1 ready");
        backend
            .mark_ready(
                fixture.principals.player_two,
                &session.session_id,
                "n:ready:p2",
            )
            .expect("p2 ready");

        backend.set_setup_trap_after_effects(Some(3));
        assert_eq!(
            backend
                .start_session(
                    fixture.principals.player_one,
                    &session.session_id,
                    "n:start"
                )
                .expect_err("setup should trap at deterministic phase"),
            LifecycleError::SetupInterrupted
        );
        assert_eq!(
            backend.get_session(&session.session_id).unwrap().state,
            "starting"
        );
        assert_eq!(
            backend
                .setup_projection(&session.session_id)
                .unwrap()
                .completed_effect_keys
                .len(),
            3
        );

        backend.set_setup_trap_after_effects(None);
        let recovered = backend
            .recover_session_setup(&session.session_id)
            .expect("recovery should finish setup");
        let journal = backend
            .session_command_journal(&session.session_id)
            .expect("journal should exist");
        let setup = backend
            .setup_projection(&session.session_id)
            .expect("setup projection should exist");

        assert_eq!(recovered.state, "active");
        assert!(setup.is_complete_for(2));
        assert_eq!(journal.effects().len(), SETUP_EFFECTS.len());
        assert_eq!(journal.events().len(), 1);
        assert_eq!(
            journal.commands()[0].status,
            crate::command::CommandStatus::Applied
        );
        assert_eq!(
            journal.commands()[0].phase,
            crate::command::CommandPhase::Complete
        );
    }

    #[test]
    fn query_helpers_return_player_participant_session_and_empty_history() {
        let fixture = first_playable_fixture();
        let mut backend = LifecycleBackend::new(fixture.clone());
        backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("p1 registers");
        let session = backend
            .create_session(
                fixture.principals.player_one,
                &fixture.command_nonces.create_session,
                &fixture.scenario_seed,
            )
            .expect("session is created");
        let player = backend
            .get_my_player(fixture.principals.player_one)
            .expect("player query should work");
        let participant = backend
            .get_my_participant(fixture.principals.player_one, &session.session_id)
            .expect("participant query should work");
        let history = backend
            .get_match_history(fixture.principals.player_one, 0, 10)
            .expect("history shell should work");

        assert_eq!(player.player_id, fixture.ids.player_one_id);
        assert_eq!(backend.get_session(&session.session_id).unwrap(), session);
        assert_eq!(participant.participant_id, fixture.ids.participant_one_id);
        assert!(history.is_empty());
    }

    fn synthetic_principal(index: u16) -> Principal {
        let mut seed = [0x5A_u8; 32];
        seed[0] = (index & 0x00FF) as u8;
        seed[1] = (index >> 8) as u8;
        Principal::self_authenticating(&seed)
    }
}
