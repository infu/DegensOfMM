use candid::Principal;
use domm_game::{
    ApiEventPage, ApiMetrics, ApiTownView, BattleActionInput, BattleView, BuildPreview,
    CommandResponse, CommandStatusView, ContentManifestResponse, FixtureApiBackend, GameView,
    GameViewRequest, LobbyCommandResponse, MatchHistoryPage, MoveCoord, PlayableMatchView,
    RecruitPreview, RecruitTarget, SessionView, run_first_playable_backend_gate,
};

use crate::types::ProbeError;

pub trait WebClientBackend {
    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse;

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> LobbyCommandResponse;

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse;

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse;

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse;

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

    fn submit_move_intent(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse;

    fn sync_session_turn(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse;

    fn preview_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
    ) -> Result<BuildPreview, ProbeError>;

    fn submit_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse;

    fn preview_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        turn_number: u32,
    ) -> Result<RecruitPreview, ProbeError>;

    fn submit_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse;

    fn get_town_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
    ) -> Result<ApiTownView, ProbeError>;

    fn get_battle_state(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<BattleView, ProbeError>;

    fn submit_battle_action(
        &mut self,
        caller: Principal,
        input: BattleActionInput,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse;

    fn sync_battle(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse;

    fn get_content_manifest(
        &self,
        ruleset_slug: &str,
        version: u32,
    ) -> Result<ContentManifestResponse, ProbeError>;

    fn get_command_status(&self, command_id_or_client_nonce: &str) -> Option<CommandStatusView>;

    fn get_events_after(
        &self,
        session_id: &str,
        audience_key: &str,
        after_seq: u64,
        limit: u32,
    ) -> ApiEventPage;

    fn get_match_history(
        &self,
        caller: Principal,
        cursor: u32,
        limit: u32,
    ) -> Result<MatchHistoryPage, ProbeError>;

    fn start_first_playable_session(&mut self) -> SessionView;

    fn first_fixture_battle_id(&self) -> String;

    fn has_first_fixture_battle_id(&self) -> bool;

    fn finish_first_playable_match(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<PlayableMatchView, ProbeError>;

    fn metrics(&self) -> ApiMetrics;
}

impl WebClientBackend for FixtureApiBackend {
    fn register_player(
        &mut self,
        caller: Principal,
        display_name: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        Self::register_player(self, caller, display_name, client_nonce)
    }

    fn create_session(
        &mut self,
        caller: Principal,
        client_nonce: &str,
        scenario_seed: &str,
    ) -> LobbyCommandResponse {
        Self::create_session(self, caller, client_nonce, scenario_seed)
    }

    fn join_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        Self::join_session(self, caller, session_id, client_nonce)
    }

    fn mark_ready(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        Self::mark_ready(self, caller, session_id, client_nonce)
    }

    fn start_session(
        &mut self,
        caller: Principal,
        session_id: &str,
        client_nonce: &str,
    ) -> LobbyCommandResponse {
        Self::start_session(self, caller, session_id, client_nonce)
    }

    fn default_game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<GameView, ProbeError> {
        Self::get_default_game_view(self, caller, session_id).map_err(ProbeError::from)
    }

    fn game_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        request: GameViewRequest,
    ) -> Result<GameView, ProbeError> {
        Self::get_game_view(self, caller, session_id, request).map_err(ProbeError::from)
    }

    fn submit_move_intent(
        &mut self,
        caller: Principal,
        session_id: &str,
        champion_id: &str,
        path: Vec<MoveCoord>,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        Self::submit_move_intent(
            self,
            caller,
            session_id,
            champion_id,
            path,
            client_nonce,
            now_ms,
        )
    }

    fn sync_session_turn(
        &mut self,
        caller: Principal,
        session_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        Self::sync_session_turn(self, caller, session_id, now_ms, client_nonce)
    }

    fn preview_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
    ) -> Result<BuildPreview, ProbeError> {
        Self::preview_build_town_structure(
            self,
            caller,
            session_id,
            town_id,
            building_slug,
            turn_number,
        )
        .map_err(ProbeError::from)
    }

    fn submit_build_town_structure(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        building_slug: &str,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        Self::submit_build_town_structure(
            self,
            caller,
            session_id,
            town_id,
            building_slug,
            turn_number,
            client_nonce,
        )
    }

    fn preview_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: &RecruitTarget,
        turn_number: u32,
    ) -> Result<RecruitPreview, ProbeError> {
        Self::preview_recruit_units(
            self,
            caller,
            session_id,
            town_id,
            unit_slug,
            quantity,
            target,
            turn_number,
        )
        .map_err(ProbeError::from)
    }

    fn submit_recruit_units(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
        unit_slug: &str,
        quantity: u32,
        target: RecruitTarget,
        turn_number: u32,
        client_nonce: &str,
    ) -> CommandResponse {
        Self::submit_recruit_units(
            self,
            caller,
            session_id,
            town_id,
            unit_slug,
            quantity,
            target,
            turn_number,
            client_nonce,
        )
    }

    fn get_town_view(
        &mut self,
        caller: Principal,
        session_id: &str,
        town_id: &str,
    ) -> Result<ApiTownView, ProbeError> {
        Self::get_town_view(self, caller, session_id, town_id).map_err(ProbeError::from)
    }

    fn get_battle_state(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<BattleView, ProbeError> {
        Self::get_battle_state(self, caller, battle_id, now_ms).map_err(ProbeError::from)
    }

    fn submit_battle_action(
        &mut self,
        caller: Principal,
        input: BattleActionInput,
        client_nonce: &str,
        now_ms: u64,
    ) -> CommandResponse {
        Self::submit_battle_action(self, caller, input, client_nonce, now_ms)
    }

    fn sync_battle(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
        client_nonce: &str,
    ) -> CommandResponse {
        Self::sync_battle(self, caller, battle_id, now_ms, client_nonce)
    }

    fn get_content_manifest(
        &self,
        ruleset_slug: &str,
        version: u32,
    ) -> Result<ContentManifestResponse, ProbeError> {
        Self::get_content_manifest(self, ruleset_slug, version).map_err(ProbeError::from)
    }

    fn get_command_status(&self, command_id_or_client_nonce: &str) -> Option<CommandStatusView> {
        Self::get_command_status(self, command_id_or_client_nonce)
    }

    fn get_events_after(
        &self,
        session_id: &str,
        audience_key: &str,
        after_seq: u64,
        limit: u32,
    ) -> ApiEventPage {
        Self::get_events_after(self, session_id, audience_key, after_seq, limit)
    }

    fn get_match_history(
        &self,
        caller: Principal,
        cursor: u32,
        limit: u32,
    ) -> Result<MatchHistoryPage, ProbeError> {
        Self::get_match_history(self, caller, cursor, limit).map_err(ProbeError::from)
    }

    fn start_first_playable_session(&mut self) -> SessionView {
        Self::start_first_playable_session(self)
    }

    fn first_fixture_battle_id(&self) -> String {
        Self::first_fixture_battle_id(self)
    }

    fn has_first_fixture_battle_id(&self) -> bool {
        true
    }

    fn finish_first_playable_match(
        &mut self,
        _caller: Principal,
        _session_id: &str,
    ) -> Result<PlayableMatchView, ProbeError> {
        Ok(run_first_playable_backend_gate()?.final_view)
    }

    fn metrics(&self) -> ApiMetrics {
        Self::metrics(self)
    }
}
