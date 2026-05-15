use crate::battle::build_first_playable_battle_state;
use crate::champion::build_first_playable_champion_state;
use crate::content::FIRST_PLAYABLE_MAX_TURNS;
use crate::economy::build_first_playable_economy_state;
use crate::fixtures::first_playable_fixture;
use crate::map::build_first_playable_map_state;
use crate::neutral::build_first_playable_neutral_state;
use crate::town::build_first_playable_town_state;

use super::types::{AftermathError, AftermathState, MatchSessionRecord};

pub fn build_first_playable_aftermath_state() -> Result<AftermathState, AftermathError> {
    let fixture = first_playable_fixture();
    Ok(AftermathState {
        session: MatchSessionRecord {
            session_id: fixture.ids.session_id,
            state: "active".to_string(),
            current_turn: 8,
            max_turns: FIRST_PLAYABLE_MAX_TURNS,
            winner_participant_id: None,
            finish_reason: None,
            last_command_id: Some("command:fixture:start".to_string()),
        },
        battle: build_first_playable_battle_state()?,
        champions: build_first_playable_champion_state(),
        town: build_first_playable_town_state(),
        economy: build_first_playable_economy_state(),
        map: build_first_playable_map_state(),
        neutral: build_first_playable_neutral_state(),
        player_match_summaries: Vec::new(),
        match_history: Vec::new(),
        aftermath_reports: Vec::new(),
        aftermath_events: Vec::new(),
        applied_commands: Vec::new(),
    })
}
