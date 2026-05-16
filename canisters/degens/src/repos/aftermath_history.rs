//! Repository boundary for battle aftermath, victory, and retained match-history rows.

use domm_degens_schema::schema::{GameSession, PlayerAccount, PlayerMatchSummary};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const MATCH_HISTORY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "history.by_player_finished_at",
    entity: "PlayerMatchSummary",
    indexed_fields: &["player_id", "finished_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const MATCH_SUMMARY_SESSION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "history.by_session",
    entity: "PlayerMatchSummary",
    indexed_fields: &["session_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn page_match_history(
    player_id: Id<PlayerAccount>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<PlayerMatchSummary>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        MATCH_HISTORY_LOOKUP.name,
        crate::db()
            .load::<PlayerMatchSummary>()
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .order_desc("finished_at")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_match_summaries_for_session(
    session_id: Id<GameSession>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<PlayerMatchSummary>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        MATCH_SUMMARY_SESSION_LOOKUP.name,
        crate::db()
            .load::<PlayerMatchSummary>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("player_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn match_history_plan_text(
    player_id: Id<PlayerAccount>,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        MATCH_HISTORY_LOOKUP.name,
        crate::db()
            .load::<PlayerMatchSummary>()
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .order_desc("finished_at")
            .order_asc("id")
            .limit(limit),
    )
}
