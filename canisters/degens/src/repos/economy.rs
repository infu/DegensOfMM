//! Repository boundary for balances, ledgers, income sources, and turn summaries.

use domm_degens_schema::schema::{
    GameCommand, GameParticipant, GameSession, ResourceLedgerEntry, ResourceLedgerTurnSummary,
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

pub(crate) fn find_resource_ledger_entry(
    command_id: Id<GameCommand>,
    ledger_key: &str,
) -> RepoResult<Option<ResourceLedgerEntry>> {
    foundation::storage_result(
        "economy.ledger_by_command_key",
        crate::db()
            .load::<ResourceLedgerEntry>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .filter(FieldRef::new("ledger_key").eq(ledger_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_resource_ledger_entry(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    command_id: Id<GameCommand>,
    ledger_key: String,
    turn_number: u32,
    resource_key: String,
    delta: i64,
    balance_after: u64,
    reason: String,
    status: String,
) -> RepoResult<ResourceLedgerEntry> {
    let input: Create<ResourceLedgerEntry> = Create::<ResourceLedgerEntry> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        command_id: Some(command_id.key()),
        ledger_key: Some(ledger_key),
        turn_number: Some(turn_number),
        resource_key: Some(resource_key),
        delta: Some(delta),
        balance_after: Some(balance_after),
        reason: Some(reason),
        status: Some(status),
    };

    foundation::create("economy.create_resource_ledger_entry", input)
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
