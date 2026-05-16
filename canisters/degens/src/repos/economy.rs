//! Repository boundary for balances, ledgers, income sources, and turn summaries.

use domm_degens_schema::schema::{
    GameParticipant, GameSession, ResourceLedgerEntry, ResourceLedgerTurnSummary,
};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const RESOURCE_LEDGER_BY_PARTICIPANT_TURN_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy.ledger_by_participant_turn",
    entity: "ResourceLedgerEntry",
    indexed_fields: &["participant_id", "turn_number"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const RESOURCE_TURN_SUMMARY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "economy.turn_summary",
    entity: "ResourceLedgerTurnSummary",
    indexed_fields: &["session_id", "participant_id", "turn_number"],
    bounded_limit: Some(1),
};

pub(crate) fn page_resource_ledger_by_participant_turn(
    participant_id: Id<GameParticipant>,
    turn_number: u32,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ResourceLedgerEntry>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        RESOURCE_LEDGER_BY_PARTICIPANT_TURN_LOOKUP.name,
        crate::db()
            .load::<ResourceLedgerEntry>()
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("resource_key")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_resource_turn_summary(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
) -> RepoResult<Option<ResourceLedgerTurnSummary>> {
    foundation::storage_result(
        RESOURCE_TURN_SUMMARY_LOOKUP.name,
        crate::db()
            .load::<ResourceLedgerTurnSummary>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("turn_number").eq(turn_number))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_resource_turn_summary(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    turn_number: u32,
    summary_json: String,
) -> RepoResult<ResourceLedgerTurnSummary> {
    let input: Create<ResourceLedgerTurnSummary> = Create::<ResourceLedgerTurnSummary> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        turn_number: Some(turn_number),
        summary_json: Some(summary_json),
    };

    foundation::create("economy.create_resource_turn_summary", input)
}
