//! Repository boundary for commands, pending effects, idempotency keys, and event logs.

use std::cell::RefCell;

use domm_degens_schema::schema::{
    Champion, CommandEffect, GameCommand, GameEvent, GameParticipant, GameSession, LobbyCommand,
    PendingEffect, PlayerAccount,
};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Principal, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

thread_local! {
    static EVENT_FEED_CACHE: RefCell<Option<EventFeedCache>> = const { RefCell::new(None) };
}

struct EventFeedCache {
    session_key: String,
    audience_key: String,
    rows: Vec<GameEvent>,
}

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

pub(crate) const GAME_COMMAND_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "commands.game_command_by_session_status",
    entity: "GameCommand",
    indexed_fields: &["session_id", "status", "created_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const EVENTS_BY_TYPE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "events.by_session_event_type",
    entity: "GameEvent",
    indexed_fields: &["session_id", "event_type"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const PENDING_EFFECT_DUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "effects.pending_by_status_due_turn",
    entity: "PendingEffect",
    indexed_fields: &["session_id", "status", "due_turn"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const COMMAND_EFFECT_SESSION_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "effects.command_effect_by_session_status",
    entity: "CommandEffect",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn find_game_command_by_idempotency(
    session_id: Id<GameSession>,
    actor_kind: &str,
    actor_id_text: &str,
    client_nonce: u64,
) -> RepoResult<Option<GameCommand>> {
    foundation::storage_operation(GAME_COMMAND_IDEMPOTENCY_LOOKUP.name, || {
        crate::db()
            .load::<GameCommand>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("actor_kind").eq(actor_kind))
            .filter(FieldRef::new("actor_id_text").eq(actor_id_text))
            .filter(FieldRef::new("client_nonce").eq(client_nonce))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn load_game_command(id: Id<GameCommand>) -> RepoResult<Option<GameCommand>> {
    foundation::load_by_id("commands.load_game_command", id)
}

pub(crate) fn insert_game_command(command: GameCommand) -> RepoResult<GameCommand> {
    foundation::insert("commands.insert_game_command", command)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_game_command(
    session_id: Id<GameSession>,
    actor_kind: String,
    actor_id_text: String,
    actor_player_id: Option<Id<PlayerAccount>>,
    actor_participant_id: Option<Id<GameParticipant>>,
    champion_id: Option<Id<Champion>>,
    turn_number: u32,
    client_nonce: u64,
    command_type: String,
    payload_hash: String,
    payload_json: String,
) -> RepoResult<GameCommand> {
    let input: Create<GameCommand> = Create::<GameCommand> {
        session_id: Some(session_id.key()),
        actor_kind: Some(actor_kind),
        actor_id_text: Some(actor_id_text),
        actor_player_id: Some(actor_player_id.map(|id| id.key())),
        actor_participant_id: Some(actor_participant_id.map(|id| id.key())),
        champion_id: Some(champion_id.map(|id| id.key())),
        turn_number: Some(turn_number),
        client_nonce: Some(client_nonce),
        command_type: Some(command_type),
        status: Some("pending".to_string()),
        phase: Some("created".to_string()),
        payload_hash: Some(payload_hash),
        payload_json: Some(payload_json),
        result_json: Some(None),
        error_code: Some(None),
        error_message: Some(None),
        error_details_json: Some(None),
        retryable: Some(false),
        applied_at: Some(None),
        failed_at: Some(None),
    };

    foundation::create("commands.create_game_command", input)
}

pub(crate) fn update_game_command(command: GameCommand) -> RepoResult<GameCommand> {
    foundation::update("commands.update_game_command", command)
}

pub(crate) fn page_game_commands_by_session_status(
    session_id: Id<GameSession>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<GameCommand>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        GAME_COMMAND_STATUS_LOOKUP.name,
        crate::db()
            .load::<GameCommand>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("created_at")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_lobby_command_by_idempotency(
    actor_principal: Principal,
    client_nonce: u64,
) -> RepoResult<Option<LobbyCommand>> {
    foundation::storage_operation(LOBBY_COMMAND_IDEMPOTENCY_LOOKUP.name, || {
        crate::db()
            .load::<LobbyCommand>()
            .filter(FieldRef::new("actor_principal").eq(actor_principal))
            .filter(FieldRef::new("client_nonce").eq(client_nonce))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn load_lobby_command(id: Id<LobbyCommand>) -> RepoResult<Option<LobbyCommand>> {
    foundation::load_by_id("commands.load_lobby_command", id)
}

pub(crate) fn insert_lobby_command(command: LobbyCommand) -> RepoResult<LobbyCommand> {
    foundation::insert("commands.insert_lobby_command", command)
}

pub(crate) fn create_lobby_command(
    actor_principal: Principal,
    actor_player_id: Option<Id<PlayerAccount>>,
    client_nonce: u64,
    payload_hash: String,
    command_type: String,
    payload_json: String,
) -> RepoResult<LobbyCommand> {
    let input: Create<LobbyCommand> = Create::<LobbyCommand> {
        actor_principal: Some(actor_principal),
        actor_player_id: Some(actor_player_id.map(|id| id.key())),
        client_nonce: Some(client_nonce),
        payload_hash: Some(payload_hash),
        command_type: Some(command_type),
        status: Some("pending".to_string()),
        phase: Some("created".to_string()),
        payload_json: Some(payload_json),
        result_json: Some(None),
        error_code: Some(None),
        error_message: Some(None),
        error_details_json: Some(None),
        retryable: Some(false),
        applied_at: Some(None),
        failed_at: Some(None),
    };

    foundation::create("commands.create_lobby_command", input)
}

pub(crate) fn update_lobby_command(command: LobbyCommand) -> RepoResult<LobbyCommand> {
    foundation::update("commands.update_lobby_command", command)
}

pub(crate) fn events_after(
    session_id: Id<GameSession>,
    audience_key: &str,
    after_event_seq: u64,
    limit: u32,
) -> RepoResult<Vec<GameEvent>> {
    let limit = foundation::validate_list_limit(limit)?;
    if event_feed_complete(session_id, audience_key) {
        return Ok(cached_events_after(
            session_id,
            audience_key,
            after_event_seq,
            limit,
        ));
    }
    let events = foundation::storage_operation(EVENT_FEED_LOOKUP.name, || {
        crate::db()
            .load::<GameEvent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("audience_key").eq(audience_key))
            .filter(FieldRef::new("event_seq").gt(after_event_seq))
            .order_asc("event_seq")
            .order_asc("id")
            .limit(limit)
            .entities()
    })?;
    if after_event_seq == 0 && events.len() < limit as usize {
        mark_event_feed_complete(session_id, audience_key);
        replace_event_rows(&events);
    }
    Ok(events)
}

pub(crate) fn events_by_type(
    session_id: Id<GameSession>,
    event_type: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<GameEvent>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        EVENTS_BY_TYPE_LOOKUP.name,
        crate::db()
            .load::<GameEvent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("event_type").eq(event_type))
            .order_asc("event_seq")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_event_by_key(
    session_id: Id<GameSession>,
    event_key: &str,
) -> RepoResult<Option<GameEvent>> {
    foundation::storage_operation("events.by_session_event_key", || {
        crate::db()
            .load::<GameEvent>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("event_key").eq(event_key))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_game_event(
    session_id: Id<GameSession>,
    command_id: Option<Id<GameCommand>>,
    actor_participant_id: Option<Id<GameParticipant>>,
    turn_number: u32,
    event_seq: u64,
    event_key: String,
    audience_key: String,
    event_type: String,
    subject_kind: Option<String>,
    subject_id_text: Option<String>,
    payload_json: String,
) -> RepoResult<GameEvent> {
    let input: Create<GameEvent> = Create::<GameEvent> {
        session_id: Some(session_id.key()),
        command_id: Some(command_id.map(|id| id.key())),
        actor_participant_id: Some(actor_participant_id.map(|id| id.key())),
        turn_number: Some(turn_number),
        event_seq: Some(event_seq),
        event_key: Some(event_key),
        audience_key: Some(audience_key),
        event_type: Some(event_type),
        subject_kind: Some(subject_kind),
        subject_id_text: Some(subject_id_text),
        payload_json: Some(payload_json),
    };

    let event = foundation::create("events.create_game_event", input)?;
    remember_created_event(&event);
    Ok(event)
}

fn remember_created_event(event: &GameEvent) {
    let session_id = Id::<GameSession>::from_key(event.session_id);
    if event.event_seq == 1 {
        mark_event_feed_complete(session_id, &event.audience_key);
    }
    if event_feed_complete(session_id, &event.audience_key) {
        append_event_row(event);
    }
}

fn replace_event_rows(rows: &[GameEvent]) {
    if let Some(first) = rows.first() {
        let session_id = Id::<GameSession>::from_key(first.session_id);
        if event_feed_complete(session_id, &first.audience_key) {
            EVENT_FEED_CACHE.with_borrow_mut(|cache| {
                if let Some(cache) = cache.as_mut() {
                    cache.rows.clear();
                    cache.rows.extend_from_slice(rows);
                }
            });
        }
    }
}

fn append_event_row(event: &GameEvent) {
    EVENT_FEED_CACHE.with_borrow_mut(|cache| {
        if let Some(cache) = cache.as_mut() {
            cache.rows.push(event.clone());
        }
    });
}

fn cached_events_after(
    session_id: Id<GameSession>,
    audience_key: &str,
    after_event_seq: u64,
    limit: u32,
) -> Vec<GameEvent> {
    let limit = limit as usize;
    let session_key = session_id.to_string();
    EVENT_FEED_CACHE.with_borrow(|cache| {
        let Some(cache) = cache else {
            return Vec::new();
        };
        if cache.session_key != session_key || cache.audience_key != audience_key {
            return Vec::new();
        }
        cache
            .rows
            .iter()
            .filter(|row| row.event_seq > after_event_seq)
            .take(limit)
            .cloned()
            .collect()
    })
}

fn event_feed_complete(session_id: Id<GameSession>, audience_key: &str) -> bool {
    let session_key = session_id.to_string();
    EVENT_FEED_CACHE.with_borrow(|cache| {
        cache.as_ref().is_some_and(|cache| {
            cache.session_key == session_key && cache.audience_key == audience_key
        })
    })
}

pub(crate) fn mark_event_feed_complete_from_runtime(
    session_id: Id<GameSession>,
    audience_key: &str,
) {
    mark_event_feed_complete(session_id, audience_key);
}

fn mark_event_feed_complete(session_id: Id<GameSession>, audience_key: &str) {
    let session_key = session_id.to_string();
    EVENT_FEED_CACHE.with_borrow_mut(|cache| {
        let replace = match cache.as_ref() {
            Some(cache) => cache.session_key != session_key || cache.audience_key != audience_key,
            None => true,
        };
        if replace {
            *cache = Some(EventFeedCache {
                session_key,
                audience_key: audience_key.to_string(),
                rows: Vec::new(),
            });
        }
    });
}

pub(crate) fn find_command_effect(
    command_id: Id<GameCommand>,
    effect_key: &str,
) -> RepoResult<Option<CommandEffect>> {
    foundation::storage_operation("effects.command_effect_by_command_key", || {
        crate::db()
            .load::<CommandEffect>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .filter(FieldRef::new("effect_key").eq(effect_key))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn find_applied_command_effect_by_session_key(
    session_id: Id<GameSession>,
    effect_key: &str,
) -> RepoResult<Option<CommandEffect>> {
    foundation::storage_operation(COMMAND_EFFECT_SESSION_STATUS_LOOKUP.name, || {
        crate::db()
            .load::<CommandEffect>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq("applied"))
            .filter(FieldRef::new("effect_key").eq(effect_key))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn create_applied_command_effect(
    session_id: Id<GameSession>,
    command_id: Id<GameCommand>,
    effect_key: String,
    effect_type: String,
    target_kind: String,
    target_id_text: String,
    payload_json: String,
    applied_at: Timestamp,
) -> RepoResult<CommandEffect> {
    let input: Create<CommandEffect> = Create::<CommandEffect> {
        session_id: Some(session_id.key()),
        command_id: Some(command_id.key()),
        effect_key: Some(effect_key),
        effect_type: Some(effect_type),
        target_kind: Some(target_kind),
        target_id_text: Some(target_id_text),
        status: Some("applied".to_string()),
        payload_json: Some(payload_json),
        applied_at: Some(Some(applied_at)),
    };

    foundation::create("effects.create_applied_command_effect", input)
}

pub(crate) fn find_pending_effect(
    session_id: Id<GameSession>,
    effect_key: &str,
) -> RepoResult<Option<PendingEffect>> {
    foundation::storage_operation("effects.pending_by_session_key", || {
        crate::db()
            .load::<PendingEffect>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("effect_key").eq(effect_key))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_applied_pending_effect(
    session_id: Id<GameSession>,
    source_command_id: Option<Id<GameCommand>>,
    target_participant_id: Option<Id<GameParticipant>>,
    target_champion_id: Option<Id<Champion>>,
    effect_key: String,
    due_turn: u32,
    effect_type: String,
    payload_json: String,
    applied_at: Timestamp,
) -> RepoResult<PendingEffect> {
    let input: Create<PendingEffect> = Create::<PendingEffect> {
        session_id: Some(session_id.key()),
        source_command_id: Some(source_command_id.map(|id| id.key())),
        target_participant_id: Some(target_participant_id.map(|id| id.key())),
        target_champion_id: Some(target_champion_id.map(|id| id.key())),
        effect_key: Some(effect_key),
        due_turn: Some(due_turn),
        effect_type: Some(effect_type),
        status: Some("applied".to_string()),
        payload_json: Some(payload_json),
        applied_at: Some(Some(applied_at)),
    };

    foundation::create("effects.create_applied_pending_effect", input)
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
