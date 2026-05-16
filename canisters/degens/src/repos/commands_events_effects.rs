//! Repository boundary for commands, pending effects, idempotency keys, and event logs.

use domm_degens_schema::schema::{GameCommand, GameEvent, GameSession, PendingEffect};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const GAME_COMMAND_IDEMPOTENCY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "commands.game_command_idempotency",
    entity: "GameCommand",
    indexed_fields: &["session_id", "actor_kind", "actor_id_text", "client_nonce"],
    bounded_limit: Some(1),
};

pub(crate) const LOBBY_COMMAND_IDEMPOTENCY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "commands.lobby_command_idempotency",
    entity: "LobbyCommand",
    indexed_fields: &["actor_principal", "client_nonce"],
    bounded_limit: Some(1),
};

pub(crate) const EVENT_FEED_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "events.by_session_audience_seq",
    entity: "GameEvent",
    indexed_fields: &["session_id", "audience_key", "event_seq"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const PENDING_EFFECT_DUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "effects.pending_by_status_due_turn",
    entity: "PendingEffect",
    indexed_fields: &["session_id", "status", "due_turn"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn find_game_command_by_idempotency(
    session_id: Id<GameSession>,
    actor_kind: &str,
    actor_id_text: &str,
    client_nonce: u64,
) -> RepoResult<Option<GameCommand>> {
    foundation::storage_result(
        GAME_COMMAND_IDEMPOTENCY_LOOKUP.name,
        crate::db()
            .load::<GameCommand>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("actor_kind").eq(actor_kind))
            .filter(FieldRef::new("actor_id_text").eq(actor_id_text))
            .filter(FieldRef::new("client_nonce").eq(client_nonce))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_game_command(id: Id<GameCommand>) -> RepoResult<Option<GameCommand>> {
    foundation::load_by_id("commands.load_game_command", id)
}

pub(crate) fn events_after(
    session_id: Id<GameSession>,
    audience_key: &str,
    after_event_seq: u64,
    limit: u32,
) -> RepoResult<Vec<GameEvent>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_result(
        EVENT_FEED_LOOKUP.name,
        crate::db()
            .load::<GameEvent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("audience_key").eq(audience_key))
            .filter(FieldRef::new("event_seq").gt(after_event_seq))
            .order_asc("event_seq")
            .order_asc("id")
            .limit(limit)
            .entities(),
    )
}

pub(crate) fn page_pending_effects_due(
    session_id: Id<GameSession>,
    status: &str,
    due_turn: u32,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<PendingEffect>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        PENDING_EFFECT_DUE_LOOKUP.name,
        crate::db()
            .load::<PendingEffect>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .filter(FieldRef::new("due_turn").lte(due_turn))
            .order_asc("due_turn")
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn game_command_idempotency_plan_text(
    session_id: Id<GameSession>,
    actor_kind: &str,
    actor_id_text: &str,
    client_nonce: u64,
) -> RepoResult<String> {
    foundation::explain_text(
        GAME_COMMAND_IDEMPOTENCY_LOOKUP.name,
        crate::db()
            .load::<GameCommand>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("actor_kind").eq(actor_kind))
            .filter(FieldRef::new("actor_id_text").eq(actor_id_text))
            .filter(FieldRef::new("client_nonce").eq(client_nonce))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn event_feed_plan_text(
    session_id: Id<GameSession>,
    audience_key: &str,
    after_event_seq: u64,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        EVENT_FEED_LOOKUP.name,
        crate::db()
            .load::<GameEvent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("audience_key").eq(audience_key))
            .filter(FieldRef::new("event_seq").gt(after_event_seq))
            .order_asc("event_seq")
            .order_asc("id")
            .limit(limit),
    )
}
