//! Canister service modules grouped by game domain.

pub(crate) mod account_lobby_session;
pub(crate) mod battle;
pub(crate) mod battle_aftermath;
pub(crate) mod battle_rows;
pub(crate) mod battle_start;
pub(crate) mod cleanup;
pub(crate) mod command_response;
pub(crate) mod content;
pub(crate) mod diagnostics;
pub(crate) mod events;
pub(crate) mod first_playable_setup;
pub(crate) mod game_view;
pub(crate) mod history;
pub(crate) mod movement;
pub(crate) mod render_projection;
pub(crate) mod session_context;
#[cfg(test)]
mod tests;
pub(crate) mod town;
