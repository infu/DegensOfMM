use candid::Principal;

use crate::aftermath::{
    AftermathState, apply_battle_aftermath, resolve_neutral_battle_for_fixture,
    seed_resolved_champion_defeat_battle, seed_resolved_town_capture_battle,
};
use crate::battle::{
    BattleCommandBudget, BattleCoord, battle_view_for_participant, submit_battle_action,
    sync_battle,
};
use crate::driver::HeadlessBackend;
use crate::fixtures::ScenarioFixture;
use crate::strategic::StrategicFixtureBackend;

use super::types::{
    PlayableBattleView, PlayableCall, PlayableCommandReceipt, PlayableError, PlayableEventPage,
    PlayableMatchView,
};

#[derive(Clone, Debug)]
pub struct PlayableFixtureBackend {
    fixture: ScenarioFixture,
    strategic: crate::strategic::StrategicFixtureBackend,
    aftermath: Option<AftermathState>,
    calls: Vec<PlayableCall>,
    recovery_retry_count: u32,
    playable_command_count: u32,
    playable_query_count: u32,
}

impl PlayableFixtureBackend {
    #[must_use]
    pub fn new(fixture: ScenarioFixture) -> Self {
        Self {
            strategic: crate::strategic::StrategicFixtureBackend::new(fixture.clone()),
            fixture,
            aftermath: None,
            calls: Vec::new(),
            recovery_retry_count: 0,
            playable_command_count: 0,
            playable_query_count: 0,
        }
    }

    #[must_use]
    pub fn from_strategic(fixture: ScenarioFixture, strategic: StrategicFixtureBackend) -> Self {
        let calls = strategic
            .calls()
            .iter()
            .cloned()
            .map(PlayableCall::Strategic)
            .collect();
        Self {
            fixture,
            strategic,
            aftermath: None,
            calls,
            recovery_retry_count: 0,
            playable_command_count: 0,
            playable_query_count: 0,
        }
    }

    #[must_use]
    pub fn calls(&self) -> &[PlayableCall] {
        &self.calls
    }

    pub fn prepare_battle_public(
        &mut self,
        caller: Principal,
        session_id: &str,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        self.strategic.get_game_view(caller, session_id)?;
        let state = self.strategic.export_aftermath_state()?;
        let battle_id = state.battle.battles[0].battle_id.clone();
        let current_round = state.battle.battles[0].current_round;
        let active_stack_id = state.battle.battles[0].active_stack_id.clone();
        self.aftermath = Some(state);
        self.record_command(PlayableCall::PrepareBattle { caller });
        Ok(PlayableCommandReceipt {
            command_kind: "prepare_battle".to_string(),
            command_id: format!("command:playable:{session_id}:prepare-battle"),
            battle_id: Some(battle_id),
            current_round,
            active_stack_id,
            replayed: false,
            event_count: self.total_event_count(),
        })
    }

    pub fn inspect_battle_public(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<PlayableBattleView, PlayableError> {
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let state = self.aftermath()?;
        let view = battle_view_for_participant(&state.battle, battle_id, &participant_id, now_ms)?;
        let event_count = state.battle.events.len() as u32;
        self.record_query(PlayableCall::InspectBattle {
            caller,
            battle_id: battle_id.to_string(),
        });
        Ok(PlayableBattleView {
            battle_id: battle_id.to_string(),
            battle_state: view.state,
            active_stack_id: view.active_stack_id,
            legal_action_count: view.legal_actions_for_caller.len() as u32,
            event_count,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_battle_action_public(
        &mut self,
        caller: Principal,
        battle_id: &str,
        battle_stack_id: &str,
        action: &str,
        target_stack_id: Option<&str>,
        destination: Option<BattleCoord>,
        client_nonce: &str,
        now_ms: u64,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let participant_id = self.participant_for_caller(caller)?.to_string();
        let was_replay = self.aftermath()?.battle.commands.iter().any(|command| {
            !command.system
                && command.battle_id == battle_id
                && command.actor_participant_id.as_deref() == Some(participant_id.as_str())
                && command.battle_stack_id.as_deref() == Some(battle_stack_id)
                && command.client_nonce == client_nonce
        });
        let receipt = submit_battle_action(
            &mut self.aftermath_mut()?.battle,
            battle_id,
            &participant_id,
            battle_stack_id,
            action,
            target_stack_id,
            destination,
            client_nonce,
            now_ms,
        )?;
        if was_replay {
            self.recovery_retry_count = self.recovery_retry_count.saturating_add(1);
        }
        self.record_command(PlayableCall::SubmitBattleAction {
            caller,
            battle_id: battle_id.to_string(),
            battle_stack_id: battle_stack_id.to_string(),
            action: action.to_string(),
        });
        Ok(PlayableCommandReceipt {
            command_kind: "submit_battle_action".to_string(),
            command_id: receipt.command_id,
            battle_id: Some(battle_id.to_string()),
            current_round: receipt.current_round,
            active_stack_id: receipt.active_stack_id,
            replayed: was_replay,
            event_count: self.total_event_count(),
        })
    }

    pub fn sync_battle_public(
        &mut self,
        caller: Principal,
        battle_id: &str,
        now_ms: u64,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let outcome = sync_battle(
            &mut self.aftermath_mut()?.battle,
            battle_id,
            now_ms,
            BattleCommandBudget::default(),
        )?;
        self.record_command(PlayableCall::SyncBattle {
            caller,
            battle_id: battle_id.to_string(),
        });
        Ok(PlayableCommandReceipt {
            command_kind: "sync_battle".to_string(),
            command_id: format!("command:playable:{battle_id}:sync:{now_ms}"),
            battle_id: Some(battle_id.to_string()),
            current_round: self.aftermath()?.battle.battle(battle_id)?.current_round,
            active_stack_id: outcome.active_stack_id,
            replayed: outcome.recovered_commands > 0,
            event_count: self.total_event_count(),
        })
    }

    pub fn resolve_neutral_battle_public(
        &mut self,
        caller: Principal,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let command_id = "command:playable:resolve-neutral";
        let battle_id = resolve_neutral_battle_for_fixture(self.aftermath_mut()?, command_id)?;
        self.record_command(PlayableCall::ResolveNeutralBattle {
            caller,
            battle_id: battle_id.clone(),
        });
        Ok(self.receipt_for_battle("resolve_neutral_battle", command_id, &battle_id, false)?)
    }

    pub fn apply_battle_aftermath_public(
        &mut self,
        caller: Principal,
        battle_id: &str,
        command_id: &str,
        finished_at: u64,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        apply_battle_aftermath(self.aftermath_mut()?, battle_id, command_id, finished_at)?;
        self.record_command(PlayableCall::ApplyBattleAftermath {
            caller,
            battle_id: battle_id.to_string(),
        });
        Ok(self.receipt_for_battle("apply_battle_aftermath", command_id, battle_id, false)?)
    }

    pub fn resolve_town_capture_public(
        &mut self,
        caller: Principal,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let battle_id = seed_resolved_town_capture_battle(self.aftermath_mut()?);
        self.record_command(PlayableCall::ResolveTownCapture { caller });
        Ok(self.receipt_for_battle(
            "resolve_town_capture",
            "command:playable:resolve-town-capture",
            &battle_id,
            false,
        )?)
    }

    pub fn resolve_champion_defeat_public(
        &mut self,
        caller: Principal,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let battle_id = seed_resolved_champion_defeat_battle(self.aftermath_mut()?);
        self.record_command(PlayableCall::ResolveChampionDefeat { caller });
        Ok(self.receipt_for_battle(
            "resolve_champion_defeat",
            "command:playable:resolve-champion-defeat",
            &battle_id,
            false,
        )?)
    }

    pub fn refresh_events_public(
        &mut self,
        caller: Principal,
        cursor: u32,
        limit: u32,
    ) -> Result<PlayableEventPage, PlayableError> {
        let total_event_count = self.total_event_count();
        let remaining = total_event_count.saturating_sub(cursor);
        let events_returned = remaining.min(limit);
        let next_cursor =
            (cursor + events_returned < total_event_count).then_some(cursor + events_returned);
        self.record_query(PlayableCall::RefreshEvents { caller });
        Ok(PlayableEventPage {
            cursor,
            next_cursor,
            events_returned,
            total_event_count,
        })
    }

    pub fn inspect_match_public(
        &mut self,
        caller: Principal,
    ) -> Result<PlayableMatchView, PlayableError> {
        self.record_query(PlayableCall::InspectMatch { caller });
        self.match_view()
    }

    fn participant_for_caller(&self, caller: Principal) -> Result<&str, PlayableError> {
        if caller == self.fixture.principals.player_one {
            Ok(&self.fixture.ids.participant_one_id)
        } else if caller == self.fixture.principals.player_two {
            Ok(&self.fixture.ids.participant_two_id)
        } else {
            Err(PlayableError::UnknownCaller)
        }
    }

    fn aftermath(&self) -> Result<&AftermathState, PlayableError> {
        self.aftermath
            .as_ref()
            .ok_or(PlayableError::MissingAftermathState)
    }

    fn aftermath_mut(&mut self) -> Result<&mut AftermathState, PlayableError> {
        self.aftermath
            .as_mut()
            .ok_or(PlayableError::MissingAftermathState)
    }

    fn record_command(&mut self, call: PlayableCall) {
        self.calls.push(call);
        self.playable_command_count = self.playable_command_count.saturating_add(1);
    }

    fn record_query(&mut self, call: PlayableCall) {
        self.calls.push(call);
        self.playable_query_count = self.playable_query_count.saturating_add(1);
    }

    fn receipt_for_battle(
        &self,
        command_kind: &str,
        command_id: &str,
        battle_id: &str,
        replayed: bool,
    ) -> Result<PlayableCommandReceipt, PlayableError> {
        let battle = self.aftermath()?.battle.battle(battle_id)?;
        Ok(PlayableCommandReceipt {
            command_kind: command_kind.to_string(),
            command_id: command_id.to_string(),
            battle_id: Some(battle_id.to_string()),
            current_round: battle.current_round,
            active_stack_id: battle.active_stack_id.clone(),
            replayed,
            event_count: self.total_event_count(),
        })
    }

    fn match_view(&self) -> Result<PlayableMatchView, PlayableError> {
        let state = self.aftermath()?;
        Ok(PlayableMatchView {
            session_id: state.session.session_id.clone(),
            current_turn: state.session.current_turn,
            final_session_state: state.session.state.clone(),
            winner_participant_id: state.session.winner_participant_id.clone(),
            champion_status: state
                .champions
                .champion("champion:west")
                .map_err(crate::aftermath::AftermathError::from)?
                .status
                .clone(),
            captured_town_owner: state
                .town
                .town("town:east")
                .map_err(crate::aftermath::AftermathError::from)?
                .owner_participant_id
                .clone(),
            defeated_neutral_state: state
                .neutral
                .army("neutral:west-mine")
                .map_err(crate::aftermath::AftermathError::from)?
                .state
                .clone(),
            defeated_champion_status: state
                .champions
                .champion("champion:east")
                .map_err(crate::aftermath::AftermathError::from)?
                .status
                .clone(),
            match_summary_count: state.player_match_summaries.len() as u32,
            match_history_count: state.match_history.len() as u32,
            command_count: self.total_command_count(),
            event_count: self.total_event_count(),
            query_count: self.total_query_count(),
            max_query_bytes: self.max_query_bytes(),
            storage_row_count: self.storage_row_count(),
            recovery_retry_count: self.recovery_retry_count,
        })
    }

    fn total_command_count(&self) -> u32 {
        self.strategic
            .command_count()
            .saturating_add(self.playable_command_count)
    }

    fn total_event_count(&self) -> u32 {
        let aftermath_events = self.aftermath.as_ref().map_or(0, |state| {
            state.battle.events.len() as u32 + state.aftermath_events.len() as u32
        });
        self.strategic
            .event_count()
            .saturating_add(aftermath_events)
    }

    fn total_query_count(&self) -> u32 {
        self.strategic
            .query_count()
            .saturating_add(self.playable_query_count)
    }

    fn max_query_bytes(&self) -> u32 {
        let row_bytes = self.storage_row_count().saturating_mul(24);
        512 + row_bytes.min(16_384)
    }

    fn storage_row_count(&self) -> u32 {
        self.aftermath
            .as_ref()
            .map_or(0, storage_rows_for_aftermath)
    }
}

fn storage_rows_for_aftermath(state: &AftermathState) -> u32 {
    let battle_rows = state.battle.battles.len()
        + state.battle.stacks.len()
        + state.battle.occupancy.len()
        + state.battle.obstacles.len()
        + state.battle.commands.len()
        + state.battle.events.len();
    let champion_rows = state.champions.champions.len()
        + state.champions.army_stacks.len()
        + state.champions.artifact_instances.len()
        + state.champions.artifact_equipment.len();
    let town_rows = state.town.towns.len()
        + state.town.buildings.len()
        + state.town.recruit_pools.len()
        + state.town.garrison_stacks.len()
        + state.town.champion_stacks.len()
        + state.town.champions.len();
    let economy_rows = state.economy.participants.len()
        + state.economy.income_sources.len()
        + state.economy.resource_piles.len()
        + state.economy.ledger_entries.len()
        + state.economy.turn_summaries.len();
    let map_rows = state.map.chunks.len()
        + state.map.visibility_chunks.len()
        + state.map.known_objects.len()
        + state.map.occupancy_rows.len()
        + state.map.subjects.len();
    let neutral_rows =
        state.neutral.armies.len() + state.neutral.stacks.len() + state.neutral.encounters.len();
    let summary_rows = 1
        + state.player_match_summaries.len()
        + state.match_history.len()
        + state.aftermath_reports.len()
        + state.aftermath_events.len();
    (battle_rows
        + champion_rows
        + town_rows
        + economy_rows
        + map_rows
        + neutral_rows
        + summary_rows) as u32
}
