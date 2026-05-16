use candid::CandidType;
use domm_game::{
    ApiEventView, CommandResponse, CommandStatusView, GameView, LobbyCommandResponse,
    MatchHistoryEntry, PlayableMatchView,
};
use serde::{Deserialize, Serialize};

use crate::RenderedViewport;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub key: String,
    pub label: String,
    pub complete: bool,
}

impl ChecklistItem {
    #[must_use]
    pub fn new(key: &str, label: &str) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            complete: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandLogEntry {
    pub command_id: String,
    pub command_type: String,
    pub client_nonce: String,
    pub status: String,
    pub error_code: Option<String>,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MatchResultPanel {
    pub session_id: String,
    pub result: String,
    pub winner_participant_id: Option<String>,
    pub turns_played: u32,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MatchHistoryPanel {
    pub entries_returned: u32,
    pub wins: u32,
    pub losses: u32,
    pub rows: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WebClientState {
    pub session_id: Option<String>,
    pub content_manifest_hash: Option<String>,
    pub game_view: Option<GameView>,
    pub rendered_viewport: Option<RenderedViewport>,
    pub event_feed: Vec<ApiEventView>,
    pub command_log: Vec<CommandLogEntry>,
    pub last_command_status: Option<CommandStatusView>,
    pub checklist: Vec<ChecklistItem>,
    pub match_result: Option<MatchResultPanel>,
    pub match_history: MatchHistoryPanel,
    pub rematch_available: bool,
    pub sync_required: bool,
    pub retry_replays: u32,
    pub discarded_nonces: u32,
    pub battle_panel_open: bool,
}

impl Default for WebClientState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebClientState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: None,
            content_manifest_hash: None,
            game_view: None,
            rendered_viewport: None,
            event_feed: Vec::new(),
            command_log: Vec::new(),
            last_command_status: None,
            checklist: first_match_checklist(),
            match_result: None,
            match_history: MatchHistoryPanel::default(),
            rematch_available: false,
            sync_required: false,
            retry_replays: 0,
            discarded_nonces: 0,
            battle_panel_open: false,
        }
    }

    pub fn mark_complete(&mut self, key: &str) {
        if let Some(item) = self.checklist.iter_mut().find(|item| item.key == key) {
            item.complete = true;
        }
    }

    #[must_use]
    pub fn checklist_complete(&self) -> bool {
        self.checklist.iter().all(|item| item.complete)
    }

    pub fn record_lobby_response(&mut self, response: &LobbyCommandResponse, replayed: bool) {
        self.command_log.push(CommandLogEntry {
            command_id: response.command_id.clone(),
            command_type: response.command_type.clone(),
            client_nonce: response.client_nonce.clone(),
            status: format!("{:?}", response.status),
            error_code: response.error.as_ref().map(|error| error.code.clone()),
            replayed,
        });
        self.last_command_status = Some(CommandStatusView {
            command_id: response.command_id.clone(),
            status: response.status,
            phase: response.phase,
            retryable: response.retryable,
            error_code: response.error.as_ref().map(|error| error.code.clone()),
            error_message: response.error.as_ref().map(|error| error.message.clone()),
            result_json: None,
        });
    }

    pub fn record_command_response(&mut self, response: &CommandResponse, replayed: bool) {
        self.command_log.push(CommandLogEntry {
            command_id: response.command_id.clone(),
            command_type: response.command_type.clone(),
            client_nonce: response.client_nonce.clone(),
            status: format!("{:?}", response.status),
            error_code: response.error.as_ref().map(|error| error.code.clone()),
            replayed,
        });
        self.last_command_status = Some(response.status_view());
    }

    pub fn apply_game_view(&mut self, game_view: GameView, rendered: RenderedViewport) {
        self.sync_required = game_view.render_time.sync_required;
        self.content_manifest_hash = Some(game_view.content_manifest_hash.clone());
        self.event_feed = game_view.events.clone();
        self.game_view = Some(game_view);
        self.rendered_viewport = Some(rendered);
    }

    pub fn apply_match_history(&mut self, entries: &[MatchHistoryEntry]) {
        let mut panel = MatchHistoryPanel {
            entries_returned: entries.len() as u32,
            ..MatchHistoryPanel::default()
        };
        for entry in entries {
            match entry.result.as_str() {
                "win" => panel.wins = panel.wins.saturating_add(1),
                "loss" => panel.losses = panel.losses.saturating_add(1),
                _ => {}
            }
            panel.rows.push(format!(
                "{}:{}:{}",
                entry.session_id, entry.result, entry.turns_played
            ));
        }
        self.match_history = panel;
    }

    pub fn apply_playable_result(&mut self, result: &PlayableMatchView) {
        let won = result
            .winner_participant_id
            .as_deref()
            .is_some_and(|winner| winner == result.captured_town_owner);
        self.match_result = Some(MatchResultPanel {
            session_id: result.session_id.clone(),
            result: if won { "win" } else { "finished" }.to_string(),
            winner_participant_id: result.winner_participant_id.clone(),
            turns_played: result.current_turn,
            summary: format!(
                "neutral={}, town_owner={}, defeated_champion={}",
                result.defeated_neutral_state,
                result.captured_town_owner,
                result.defeated_champion_status
            ),
        });
        self.match_history = MatchHistoryPanel {
            entries_returned: result.match_history_count,
            wins: u32::from(result.winner_participant_id.is_some()),
            losses: 0,
            rows: vec![format!(
                "{}:{}:{}",
                result.session_id, result.final_session_state, result.current_turn
            )],
        };
        self.rematch_available = true;
        self.mark_complete("result");
    }
}

fn first_match_checklist() -> Vec<ChecklistItem> {
    vec![
        ChecklistItem::new("lobby", "Start match"),
        ChecklistItem::new("map", "Inspect map"),
        ChecklistItem::new("pickup", "Collect nearby supplies"),
        ChecklistItem::new("build", "Build training yard"),
        ChecklistItem::new("recruit", "Recruit levies"),
        ChecklistItem::new("battle", "Trigger and resolve battle"),
        ChecklistItem::new("result", "Review match result"),
    ]
}
