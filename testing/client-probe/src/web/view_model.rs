use candid::CandidType;
use serde::{Deserialize, Serialize};

use super::state::WebClientState;

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WebClientViewModel {
    pub screen: String,
    pub session_badge: String,
    pub turn_badge: String,
    pub resources: String,
    pub map_rows: Vec<String>,
    pub champion_panel: Option<String>,
    pub town_panel: Option<String>,
    pub battle_panel: Option<String>,
    pub command_status: Option<String>,
    pub event_feed: Vec<String>,
    pub checklist: Vec<String>,
    pub match_result: Option<String>,
    pub match_history: Vec<String>,
    pub rematch_available: bool,
    pub sync_required: bool,
}

impl WebClientViewModel {
    #[must_use]
    pub fn from_state(state: &WebClientState) -> Self {
        let Some(game_view) = state.game_view.as_ref() else {
            return Self {
                screen: "lobby".to_string(),
                checklist: checklist_rows(state),
                ..Self::default()
            };
        };

        let resources = &game_view.participant.resources;
        let champion_panel = game_view.champions.first().map(|champion| {
            format!(
                "{} {} at {},{} movement {}/{}",
                champion
                    .name
                    .clone()
                    .unwrap_or(champion.champion_id.clone()),
                champion.status,
                champion.x,
                champion.y,
                champion.effective_movement,
                champion.movement_max
            )
        });
        let town_panel = game_view.towns.first().map(|town| {
            format!(
                "{} {} buildings={} recruit_pools={} garrison={}",
                town.town.name,
                town.town.status,
                town.buildings.len(),
                town.recruit_pools.len(),
                town.garrison_stacks.len()
            )
        });
        let battle_panel = game_view.battle_summary.as_ref().map(|battle| {
            format!(
                "{} {} round {} active={}",
                battle.battle_type,
                battle.state,
                battle.current_round,
                battle.active_stack_id.as_deref().unwrap_or("none")
            )
        });
        let command_status = state.last_command_status.as_ref().map(|status| {
            format!(
                "{} {:?}/{:?}",
                status.command_id, status.status, status.phase
            )
        });
        let match_result = state.match_result.as_ref().map(|result| {
            format!(
                "{} after {} turns: {}",
                result.result, result.turns_played, result.summary
            )
        });

        Self {
            screen: if state.match_result.is_some() {
                "result".to_string()
            } else if state.battle_panel_open {
                "battle".to_string()
            } else {
                "match".to_string()
            },
            session_badge: format!(
                "{} {}",
                game_view.session.session_id, game_view.session.state
            ),
            turn_badge: format!("turn {}", game_view.session.current_turn),
            resources: format!(
                "gold={} wood={} stone={} iron={} crystal={} ember={} aether={}",
                resources.gold,
                resources.wood,
                resources.stone,
                resources.iron,
                resources.crystal,
                resources.ember,
                resources.aether
            ),
            map_rows: state
                .rendered_viewport
                .as_ref()
                .map_or_else(Vec::new, |rendered| rendered.rows.clone()),
            champion_panel,
            town_panel,
            battle_panel,
            command_status,
            event_feed: state
                .event_feed
                .iter()
                .map(|event| format!("{}#{}", event.event_type, event.event_seq))
                .collect(),
            checklist: checklist_rows(state),
            match_result,
            match_history: state.match_history.rows.clone(),
            rematch_available: state.rematch_available,
            sync_required: state.sync_required,
        }
    }
}

fn checklist_rows(state: &WebClientState) -> Vec<String> {
    state
        .checklist
        .iter()
        .map(|item| {
            format!(
                "{}:{}",
                item.key,
                if item.complete { "complete" } else { "pending" }
            )
        })
        .collect()
}
