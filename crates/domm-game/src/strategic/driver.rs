use candid::Principal;

use crate::driver::{ActiveMatchView, HeadlessBackend, PlayerView, SessionView};
use crate::fixtures::{ScenarioFixture, TURN_DURATION_MS, first_playable_fixture};
use crate::movement::MoveCoord;
use crate::town::RecruitTarget;

use super::backend::StrategicFixtureBackend;
use super::types::{
    StrategicCommandReceipt, StrategicError, StrategicGameView, StrategicGateReport,
    StrategicStepView,
};

pub trait StrategicBackend: HeadlessBackend<Error = StrategicError> {
    fn inspect_strategic_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<StrategicGameView, StrategicError>;

    fn submit_move_intent_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: u64,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn sync_session_turn_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn apply_movement_object_interactions_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn materialize_income_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn build_town_structure_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn recruit_units_public(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        turn_number: u32,
        client_nonce: u64,
    ) -> Result<StrategicCommandReceipt, StrategicError>;

    fn apply_neutral_encounters_public(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<StrategicCommandReceipt, StrategicError>;
}

pub struct StrategicHeadlessDriver<B> {
    backend: B,
    fixture: ScenarioFixture,
    step_views: Vec<StrategicStepView>,
}

impl<B> StrategicHeadlessDriver<B> {
    #[must_use]
    pub fn new(backend: B, fixture: ScenarioFixture) -> Self {
        Self {
            backend,
            fixture,
            step_views: Vec::new(),
        }
    }

    pub fn into_backend(self) -> B {
        self.backend
    }
}

impl<B: StrategicBackend> StrategicHeadlessDriver<B> {
    pub fn run_first_playable_gate(&mut self) -> Result<StrategicGateReport, StrategicError> {
        let player_one = self.fixture.principals.player_one;
        let player_two = self.fixture.principals.player_two;
        let register_player_one = self.fixture.command_nonces.register_player_one.clone();
        let register_player_two = self.fixture.command_nonces.register_player_two.clone();
        let create_session = self.fixture.command_nonces.create_session.clone();
        let join_session = self.fixture.command_nonces.join_session.clone();
        let mark_ready_player_one = self.fixture.command_nonces.mark_ready_player_one.clone();
        let mark_ready_player_two = self.fixture.command_nonces.mark_ready_player_two.clone();
        let start_session = self.fixture.command_nonces.start_session.clone();
        let scenario_seed = self.fixture.scenario_seed.clone();

        self.backend
            .register_player(player_one, "Misery One", &register_player_one)?;
        self.backend
            .register_player(player_two, "Mayhem Two", &register_player_two)?;
        let created = self
            .backend
            .create_session(player_one, &create_session, &scenario_seed)?;
        let joined = self
            .backend
            .join_session(player_two, &created.session_id, &join_session)?;
        self.backend
            .mark_ready(player_one, &joined.session_id, &mark_ready_player_one)?;
        self.backend
            .mark_ready(player_two, &joined.session_id, &mark_ready_player_two)?;
        self.backend
            .start_session(player_one, &joined.session_id, &start_session)?;
        let _active: ActiveMatchView =
            self.backend.get_game_view(player_one, &joined.session_id)?;
        self.capture_step("started", &joined.session_id)?;

        self.backend.submit_move_intent_public(
            player_one,
            &joined.session_id,
            "champion:west",
            vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
            20_001,
            1_000,
        )?;
        self.backend
            .sync_session_turn_public(player_one, &joined.session_id, TURN_DURATION_MS)?;
        self.backend.apply_movement_object_interactions_public(
            player_one,
            &joined.session_id,
            TURN_DURATION_MS,
        )?;
        self.capture_step("pickup", &joined.session_id)?;

        self.sync_until_turn(player_one, &joined.session_id, 3)?;
        self.backend
            .materialize_income_public(player_one, &joined.session_id, 3, 20_002)?;
        self.capture_step("income", &joined.session_id)?;

        self.backend.build_town_structure_public(
            player_one,
            &joined.session_id,
            "town:west",
            "freehold-training-yard",
            3,
            20_003,
        )?;
        self.capture_step("built", &joined.session_id)?;

        self.sync_until_turn(player_one, &joined.session_id, 8)?;
        self.backend.recruit_units_public(
            player_one,
            &joined.session_id,
            "town:west",
            "mudhook-levy",
            4,
            RecruitTarget::TownGarrison { slot_index: None },
            8,
            20_004,
        )?;
        self.capture_step("recruited", &joined.session_id)?;

        self.backend.submit_move_intent_public(
            player_one,
            &joined.session_id,
            "champion:west",
            vec![
                MoveCoord::new(10, 23),
                MoveCoord::new(11, 23),
                MoveCoord::new(12, 23),
                MoveCoord::new(12, 22),
            ],
            20_005,
            TURN_DURATION_MS * 7 + 1_000,
        )?;
        self.backend.sync_session_turn_public(
            player_one,
            &joined.session_id,
            TURN_DURATION_MS * 8,
        )?;
        self.backend
            .apply_neutral_encounters_public(player_one, &joined.session_id)?;
        self.capture_step("battle_trigger", &joined.session_id)?;

        let final_view = self
            .step_views
            .last()
            .expect("gate records at least one view")
            .view
            .clone();
        let max_query_bytes = self
            .step_views
            .iter()
            .map(|step| step.view.approximate_query_bytes)
            .max()
            .unwrap_or(0);
        Ok(StrategicGateReport {
            session_id: joined.session_id,
            step_views: self.step_views.clone(),
            command_count: final_view.command_count,
            event_count: final_view.event_count,
            query_count: final_view.query_count,
            max_query_bytes,
            concerns: Vec::new(),
            final_view,
        })
    }

    fn sync_until_turn(
        &mut self,
        caller: Principal,
        session_id: &str,
        target_turn: u32,
    ) -> Result<(), StrategicError> {
        loop {
            let view = self.backend.inspect_strategic_view(caller, session_id)?;
            if view.current_turn >= target_turn {
                return Ok(());
            }
            let deadline = u64::from(view.current_turn) * TURN_DURATION_MS;
            self.backend
                .sync_session_turn_public(caller, session_id, deadline)?;
        }
    }

    fn capture_step(&mut self, step_key: &str, session_id: &str) -> Result<(), StrategicError> {
        let view = self
            .backend
            .inspect_strategic_view(self.fixture.principals.player_one, session_id)?;
        self.step_views.push(StrategicStepView {
            step_key: step_key.to_string(),
            view,
        });
        Ok(())
    }
}

pub fn run_first_playable_strategic_gate() -> Result<StrategicGateReport, StrategicError> {
    let fixture = first_playable_fixture();
    let backend = StrategicFixtureBackend::new(fixture.clone());
    let mut driver = StrategicHeadlessDriver::new(backend, fixture);
    driver.run_first_playable_gate()
}

#[allow(dead_code)]
fn _public_dto_markers(_player: PlayerView, _session: SessionView) {}
