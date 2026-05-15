use candid::Principal;
use domm_game::{
    ActiveMatchView, EventPage, FirstPlayableMapState, HeadlessBackend, LifecycleBackend,
    MapChunkPage, ObjectViewPage, ParticipantView, PlayerView, ScenarioFixture, SessionView,
    Viewport, build_first_playable_map_state_for_ids,
};

use crate::types::ProbeError;

pub trait ThinClientBackend {
    fn active_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, ProbeError>;

    fn my_participant(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ParticipantView, ProbeError>;

    fn viewport_chunks(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<MapChunkPage, ProbeError>;

    fn viewport_objects(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<ObjectViewPage, ProbeError>;

    fn events_after(
        &self,
        caller: Principal,
        session_id: &str,
        events_after_seq: u64,
        limit: usize,
    ) -> Result<EventPage, ProbeError>;
}

pub struct FixtureProbeBackend {
    lifecycle: LifecycleBackend,
    map_state: FirstPlayableMapState,
}

impl FixtureProbeBackend {
    pub fn new(fixture: ScenarioFixture) -> Result<Self, ProbeError> {
        let map_state = build_first_playable_map_state_for_ids(&fixture.ids)?;
        Ok(Self {
            lifecycle: LifecycleBackend::new(fixture),
            map_state,
        })
    }
}

impl HeadlessBackend for FixtureProbeBackend {
    type Error = ProbeError;

    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, Self::Error> {
        self.lifecycle
            .register_player(caller, display_name, client_nonce)
            .map_err(ProbeError::from)
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, Self::Error> {
        self.lifecycle
            .create_session(caller, client_nonce, scenario_seed)
            .map_err(ProbeError::from)
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.lifecycle
            .join_session(caller, session_id, client_nonce)
            .map_err(ProbeError::from)
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.lifecycle
            .mark_ready(caller, session_id, client_nonce)
            .map_err(ProbeError::from)
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.lifecycle
            .start_session(caller, session_id, client_nonce)
            .map_err(ProbeError::from)
    }

    fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, Self::Error> {
        self.lifecycle
            .get_game_view(caller, session_id)
            .map_err(ProbeError::from)
    }
}

impl ThinClientBackend for FixtureProbeBackend {
    fn active_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, ProbeError> {
        self.lifecycle
            .get_game_view(caller, session_id)
            .map_err(ProbeError::from)
    }

    fn my_participant(
        &self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ParticipantView, ProbeError> {
        self.lifecycle
            .get_my_participant(caller, session_id)
            .map_err(ProbeError::from)
    }

    fn viewport_chunks(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<MapChunkPage, ProbeError> {
        let participant = self.my_participant(caller, session_id)?;
        self.active_match(caller, session_id)?;
        Ok(self
            .map_state
            .map_chunk_views(&participant.participant_id, viewport, cursor, limit))
    }

    fn viewport_objects(
        &mut self,
        caller: Principal,
        session_id: &str,
        viewport: &Viewport,
        cursor: Option<u32>,
        limit: u32,
    ) -> Result<ObjectViewPage, ProbeError> {
        let participant = self.my_participant(caller, session_id)?;
        self.active_match(caller, session_id)?;
        Ok(self
            .map_state
            .object_views(&participant.participant_id, viewport, cursor, limit))
    }

    fn events_after(
        &self,
        caller: Principal,
        session_id: &str,
        events_after_seq: u64,
        limit: usize,
    ) -> Result<EventPage, ProbeError> {
        self.lifecycle
            .get_events(caller, session_id, events_after_seq, limit)
            .map_err(ProbeError::from)
    }
}
