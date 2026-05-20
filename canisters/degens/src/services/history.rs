use candid::Principal as CandidPrincipal;
use domm_degens_schema::schema::GameSession;
use domm_game::{ApiError, MatchHistoryEntry, MatchHistoryPage, PageInfo};
use icydb::{
    traits::EntityValue,
    types::{Id, Principal},
};

use crate::repos::aftermath_history;

use super::account_lobby_session;

pub(crate) fn get_match_history(
    caller: CandidPrincipal,
    cursor: u32,
    limit: u32,
) -> Result<MatchHistoryPage, ApiError> {
    if caller == CandidPrincipal::anonymous() {
        return Err(ApiError::new(
            "anonymous_not_allowed",
            "anonymous callers cannot read match history",
            false,
        ));
    }
    let player = account_lobby_session::find_player_by_principal(Principal::from(caller))?
        .ok_or_else(|| {
            ApiError::new(
                "player_not_registered",
                "caller does not have a registered player",
                false,
            )
        })?;
    let limit = crate::repos::foundation::validate_list_limit(limit)?;
    let fetch_limit = cursor.saturating_add(limit).min(domm_game::MAX_LIST_LIMIT);
    let page = aftermath_history::page_match_history(player.id(), fetch_limit, None)?;
    let entries = page
        .items
        .into_iter()
        .filter(|summary| summary.result != "pending")
        .skip(cursor as usize)
        .take(limit as usize)
        .map(|summary| MatchHistoryEntry {
            session_id: Id::<GameSession>::from_key(summary.session_id).to_string(),
            result: summary.result,
            opponent_name: summary.opponent_name,
            turns_played: summary.turns_played,
            summary_json: summary.summary_json,
        })
        .collect::<Vec<_>>();
    let has_more = entries.len() == limit as usize && page.next_cursor.is_some();

    Ok(MatchHistoryPage {
        entries,
        page_info: PageInfo {
            next_cursor: has_more.then_some(cursor.saturating_add(limit)),
            has_more,
            limit,
        },
    })
}
