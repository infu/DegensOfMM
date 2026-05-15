use candid::Principal;
use domm_game::{FixtureApiBackend, GameView, GameViewRequest, ScenarioFixture, SessionView};

use crate::types::ProbeError;

pub trait ThinClientBackend {
    fn default_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<GameView, ProbeError>;

    fn game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        request: GameViewRequest,
    ) -> Result<GameView, ProbeError>;
}

pub struct FixtureProbeBackend {
    api: FixtureApiBackend,
}

impl FixtureProbeBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            api: FixtureApiBackend::new(fixture),
        }
    }

    pub fn start_first_playable_session(&mut self) -> SessionView {
        self.api.start_first_playable_session()
    }
}

impl ThinClientBackend for FixtureProbeBackend {
    fn default_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<GameView, ProbeError> {
        self.api
            .get_default_game_view(caller, session_id)
            .map_err(ProbeError::from)
    }

    fn game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        request: GameViewRequest,
    ) -> Result<GameView, ProbeError> {
        self.api
            .get_game_view(caller, session_id, request)
            .map_err(ProbeError::from)
    }
}
