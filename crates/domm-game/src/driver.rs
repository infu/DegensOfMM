use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fixtures::ScenarioFixture;

/// Public player DTO used by the headless driver contract.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayerView {
    pub player_id: String,
    pub display_name: String,
    pub principal: Principal,
}

/// Public session DTO used by lobby and session smoke paths.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SessionView {
    pub session_id: String,
    pub state: String,
    pub participant_ids: Vec<String>,
}

/// Minimal render-facing match DTO for the checkpoint 0 smoke path.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ActiveMatchView {
    pub session_id: String,
    pub current_turn: u32,
    pub turn_duration_ms: u64,
    pub sync_required: bool,
    pub visible_participant_ids: Vec<String>,
}

/// Public command/query calls that a headless backend can perform.
pub trait HeadlessBackend {
    type Error;

    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, Self::Error>;

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, Self::Error>;

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error>;

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error>;

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error>;

    fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, Self::Error>;
}

/// Deterministic headless driver for public command/query smoke paths.
pub struct HeadlessGameDriver<B> {
    backend: B,
    fixture: ScenarioFixture,
}

impl<B> HeadlessGameDriver<B> {
    #[must_use]
    pub fn new(backend: B, fixture: ScenarioFixture) -> Self {
        Self { backend, fixture }
    }

    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: HeadlessBackend> HeadlessGameDriver<B> {
    pub fn create_join_start_inspect(&mut self) -> Result<ActiveMatchView, B::Error> {
        let principals = &self.fixture.principals;
        let nonces = &self.fixture.command_nonces;

        self.backend.register_player(
            principals.player_one,
            "Misery One",
            &nonces.register_player_one,
        )?;
        self.backend.register_player(
            principals.player_two,
            "Mayhem Two",
            &nonces.register_player_two,
        )?;

        let created = self.backend.create_session(
            principals.player_one,
            &nonces.create_session,
            &self.fixture.scenario_seed,
        )?;
        let joined = self.backend.join_session(
            principals.player_two,
            &created.session_id,
            &nonces.join_session,
        )?;

        self.backend.mark_ready(
            principals.player_one,
            &joined.session_id,
            &nonces.mark_ready_player_one,
        )?;
        self.backend.mark_ready(
            principals.player_two,
            &joined.session_id,
            &nonces.mark_ready_player_two,
        )?;
        self.backend.start_session(
            principals.player_one,
            &joined.session_id,
            &nonces.start_session,
        )?;

        self.backend
            .get_game_view(principals.player_one, &joined.session_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverCall {
    RegisterPlayer {
        caller: Principal,
        display_name: String,
        client_nonce: String,
    },
    CreateSession {
        caller: Principal,
        client_nonce: String,
        scenario_seed: String,
    },
    JoinSession {
        caller: Principal,
        session_id: String,
        client_nonce: String,
    },
    MarkReady {
        caller: Principal,
        session_id: String,
        client_nonce: String,
    },
    StartSession {
        caller: Principal,
        session_id: String,
        client_nonce: String,
    },
    GetGameView {
        caller: Principal,
        session_id: String,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DriverError {
    #[error("payload mismatch for nonce {client_nonce}")]
    PayloadMismatch { client_nonce: String },
    #[error("unknown fixture principal")]
    UnknownPrincipal,
    #[error("session is not active")]
    SessionNotActive,
}

#[derive(Clone, Debug)]
struct NonceRecord {
    payload: String,
}

/// Scripted backend that exercises driver ordering and idempotency without IcyDB.
#[derive(Clone, Debug)]
pub struct ScriptedBackend {
    fixture: ScenarioFixture,
    calls: Vec<DriverCall>,
    nonce_records: Vec<(String, NonceRecord)>,
    session_state: String,
    participant_ids: Vec<String>,
}

impl ScriptedBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            fixture,
            calls: Vec::new(),
            nonce_records: Vec::new(),
            session_state: "new".to_string(),
            participant_ids: Vec::new(),
        }
    }

    #[must_use]
    pub fn calls(&self) -> &[DriverCall] {
        &self.calls
    }

    fn remember_nonce(&mut self, client_nonce: &str, payload: String) -> Result<(), DriverError> {
        if let Some((_, record)) = self
            .nonce_records
            .iter()
            .find(|(nonce, _)| nonce == client_nonce)
        {
            if record.payload == payload {
                return Ok(());
            }

            return Err(DriverError::PayloadMismatch {
                client_nonce: client_nonce.to_string(),
            });
        }

        self.nonce_records
            .push((client_nonce.to_string(), NonceRecord { payload }));
        Ok(())
    }

    fn player_for_principal(&self, principal: Principal) -> Result<PlayerView, DriverError> {
        if principal == self.fixture.principals.player_one {
            Ok(PlayerView {
                player_id: self.fixture.ids.player_one_id.clone(),
                display_name: "Misery One".to_string(),
                principal,
            })
        } else if principal == self.fixture.principals.player_two {
            Ok(PlayerView {
                player_id: self.fixture.ids.player_two_id.clone(),
                display_name: "Mayhem Two".to_string(),
                principal,
            })
        } else {
            Err(DriverError::UnknownPrincipal)
        }
    }
}

impl HeadlessBackend for ScriptedBackend {
    type Error = DriverError;

    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> Result<PlayerView, Self::Error> {
        self.remember_nonce(
            client_nonce,
            format!("register_player:{caller}:{display_name}"),
        )?;
        self.calls.push(DriverCall::RegisterPlayer {
            caller,
            display_name: display_name.to_string(),
            client_nonce: client_nonce.to_string(),
        });
        self.player_for_principal(caller)
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> Result<SessionView, Self::Error> {
        self.remember_nonce(
            client_nonce,
            format!("create_session:{caller}:{scenario_seed}"),
        )?;
        self.calls.push(DriverCall::CreateSession {
            caller,
            client_nonce: client_nonce.to_string(),
            scenario_seed: scenario_seed.to_string(),
        });
        self.session_state = "lobby".to_string();
        self.participant_ids = vec![self.fixture.ids.participant_one_id.clone()];

        Ok(SessionView {
            session_id: self.fixture.ids.session_id.clone(),
            state: self.session_state.clone(),
            participant_ids: self.participant_ids.clone(),
        })
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.remember_nonce(client_nonce, format!("join_session:{caller}:{session_id}"))?;
        self.calls.push(DriverCall::JoinSession {
            caller,
            session_id: session_id.to_string(),
            client_nonce: client_nonce.to_string(),
        });
        if !self
            .participant_ids
            .contains(&self.fixture.ids.participant_two_id)
        {
            self.participant_ids
                .push(self.fixture.ids.participant_two_id.clone());
        }

        Ok(SessionView {
            session_id: session_id.to_string(),
            state: self.session_state.clone(),
            participant_ids: self.participant_ids.clone(),
        })
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.remember_nonce(client_nonce, format!("mark_ready:{caller}:{session_id}"))?;
        self.calls.push(DriverCall::MarkReady {
            caller,
            session_id: session_id.to_string(),
            client_nonce: client_nonce.to_string(),
        });

        Ok(SessionView {
            session_id: session_id.to_string(),
            state: self.session_state.clone(),
            participant_ids: self.participant_ids.clone(),
        })
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> Result<SessionView, Self::Error> {
        self.remember_nonce(client_nonce, format!("start_session:{caller}:{session_id}"))?;
        self.calls.push(DriverCall::StartSession {
            caller,
            session_id: session_id.to_string(),
            client_nonce: client_nonce.to_string(),
        });
        self.session_state = "active".to_string();

        Ok(SessionView {
            session_id: session_id.to_string(),
            state: self.session_state.clone(),
            participant_ids: self.participant_ids.clone(),
        })
    }

    fn get_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<ActiveMatchView, Self::Error> {
        self.calls.push(DriverCall::GetGameView {
            caller,
            session_id: session_id.to_string(),
        });
        if self.session_state != "active" {
            return Err(DriverError::SessionNotActive);
        }

        Ok(ActiveMatchView {
            session_id: session_id.to_string(),
            current_turn: 1,
            turn_duration_ms: self.fixture.clock.turn_duration_ms,
            sync_required: false,
            visible_participant_ids: self.participant_ids.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DriverCall, DriverError, HeadlessBackend, HeadlessGameDriver, ScriptedBackend};
    use crate::fixtures::first_playable_fixture;

    #[test]
    fn driver_create_join_start_inspect_smoke() {
        let fixture = first_playable_fixture();
        let backend = ScriptedBackend::new(fixture.clone());
        let mut driver = HeadlessGameDriver::new(backend, fixture.clone());

        let view = driver
            .create_join_start_inspect()
            .expect("scripted public smoke path should succeed");
        let backend = driver.into_backend();

        assert_eq!(view.session_id, fixture.ids.session_id);
        assert_eq!(view.current_turn, 1);
        assert!(!view.sync_required);
        assert_eq!(
            view.visible_participant_ids,
            vec![
                fixture.ids.participant_one_id,
                fixture.ids.participant_two_id
            ]
        );
        assert_eq!(backend.calls().len(), 8);
        assert!(matches!(
            backend.calls().last(),
            Some(DriverCall::GetGameView { .. })
        ));
    }

    #[test]
    fn scripted_backend_allows_exact_nonce_retry() {
        let fixture = first_playable_fixture();
        let mut backend = ScriptedBackend::new(fixture.clone());
        let first = backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("first command should succeed");
        let retry = backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("same nonce and payload should be idempotent");

        assert_eq!(first, retry);
    }

    #[test]
    fn scripted_backend_rejects_same_nonce_payload_mismatch() {
        let fixture = first_playable_fixture();
        let mut backend = ScriptedBackend::new(fixture.clone());
        backend
            .register_player(
                fixture.principals.player_one,
                "Misery One",
                &fixture.command_nonces.register_player_one,
            )
            .expect("first command should succeed");

        let err = backend
            .register_player(
                fixture.principals.player_one,
                "Different Name",
                &fixture.command_nonces.register_player_one,
            )
            .expect_err("same nonce with different payload should fail");

        assert_eq!(
            err,
            DriverError::PayloadMismatch {
                client_nonce: fixture.command_nonces.register_player_one
            }
        );
    }
}
