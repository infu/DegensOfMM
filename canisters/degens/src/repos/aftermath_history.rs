//! Repository boundary for battle aftermath, victory, and retained match-history rows.

use domm_degens_schema::schema::{GameSession, PlayerAccount, PlayerMatchSummary};
use icydb::{Create, db::query::FieldRef, types::Id};

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

pub(crate) const MATCH_SUMMARY_PLAYER_SESSION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "history.by_player_session",
    entity: "PlayerMatchSummary",
    indexed_fields: &["player_id", "session_id"],
    bounded_limit: Some(1),
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

pub(crate) fn find_match_summary_for_player_session(
    player_id: Id<PlayerAccount>,
    session_id: Id<GameSession>,
) -> RepoResult<Option<PlayerMatchSummary>> {
    foundation::storage_result(
        MATCH_SUMMARY_PLAYER_SESSION_LOOKUP.name,
        crate::db()
            .load::<PlayerMatchSummary>()
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_match_summary_shell(
    player_id: Id<PlayerAccount>,
    session_id: Id<GameSession>,
    result: String,
    opponent_name: Option<String>,
    turns_played: u32,
    summary_json: Option<String>,
) -> RepoResult<PlayerMatchSummary> {
    let input: Create<PlayerMatchSummary> = Create::<PlayerMatchSummary> {
        player_id: Some(player_id.key()),
        session_id: Some(session_id.key()),
        result: Some(result),
        opponent_name: Some(opponent_name),
        turns_played: Some(turns_played),
        summary_json: Some(summary_json),
    };

    foundation::create("history.create_match_summary_shell", input)
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
