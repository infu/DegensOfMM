//! Repository boundary for sessions, participants, setup state, and lobby lifecycle rows.

use domm_degens_schema::schema::{
    FactionDefinition, GameParticipant, GameSession, PlayerAccount, RulesetDefinition,
};
use icydb::{
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const SESSION_STATE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "sessions.by_state_current_turn",
    entity: "GameSession",
    indexed_fields: &["state", "current_turn"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const PARTICIPANT_SESSION_PLAYER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "sessions.participant_by_session_player",
    entity: "GameParticipant",
    indexed_fields: &["session_id", "player_id"],
    bounded_limit: Some(1),
};

pub(crate) const PARTICIPANT_SESSION_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "sessions.participants_by_session_status",
    entity: "GameParticipant",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const PARTICIPANT_PLAYER_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "sessions.participants_by_player_status",
    entity: "GameParticipant",
    indexed_fields: &["player_id", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn create_game_session(
    ruleset_id: Id<RulesetDefinition>,
    created_by_player_id: Id<PlayerAccount>,
    name: String,
    seed: u64,
    map_width: u16,
    map_height: u16,
    turn_deadline_at: Timestamp,
) -> RepoResult<GameSession> {
    let session = GameSession {
        ruleset_id: ruleset_id.key(),
        created_by_player_id: created_by_player_id.key(),
        name,
        state: "lobby".to_string(),
        seed,
        map_width,
        map_height,
        chunk_size: 16,
        simultaneous_turns: true,
        turn_duration_ms: 60_000,
        max_turns: 30,
        turn_catchup_cap: 10,
        current_turn: 1,
        next_event_seq: 1,
        turn_deadline_at,
        winner_participant_id: None,
        finish_reason: None,
        last_command_id: None,
        ..Default::default()
    };

    foundation::insert("sessions.create_game_session", session)
}

pub(crate) fn load_session(id: Id<GameSession>) -> RepoResult<Option<GameSession>> {
    foundation::load_by_id("sessions.load_session", id)
}

pub(crate) fn update_session(session: GameSession) -> RepoResult<GameSession> {
    foundation::update("sessions.update_session", session)
}

pub(crate) fn page_sessions_by_state(
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<GameSession>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<GameSession>()
            .filter(FieldRef::new("state").eq(state))
            .order_asc("current_turn")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn create_participant(
    session_id: Id<GameSession>,
    player_id: Id<PlayerAccount>,
    faction_id: Id<FactionDefinition>,
    slot_index: u8,
    color_key: String,
) -> RepoResult<GameParticipant> {
    let participant = new_participant(session_id, player_id, faction_id, slot_index, color_key);
    foundation::insert("sessions.create_participant", participant)
}

pub(crate) fn new_participant(
    session_id: Id<GameSession>,
    player_id: Id<PlayerAccount>,
    faction_id: Id<FactionDefinition>,
    slot_index: u8,
    color_key: String,
) -> GameParticipant {
    GameParticipant {
        session_id: session_id.key(),
        player_id: player_id.key(),
        faction_id: faction_id.key(),
        slot_index,
        team_index: 0,
        color_key,
        primary_color: None,
        secondary_color: None,
        status: "active".to_string(),
        gold: 10_000,
        wood: 10,
        stone: 10,
        iron: 3,
        crystal: 3,
        ember: 3,
        aether: 3,
        last_income_turn: 0,
        last_action_turn: 0,
        ready_turn: 0,
        last_command_id: None,
        last_resource_command_id: None,
        champion_ids: Vec::new(),
        ..Default::default()
    }
}

pub(crate) fn insert_participants_atomic(
    participants: impl IntoIterator<Item = GameParticipant>,
) -> RepoResult<Vec<GameParticipant>> {
    foundation::insert_many_atomic("sessions.insert_participants_atomic", participants)
}

pub(crate) fn load_participant(id: Id<GameParticipant>) -> RepoResult<Option<GameParticipant>> {
    foundation::load_by_id("sessions.load_participant", id)
}

pub(crate) fn update_participant(participant: GameParticipant) -> RepoResult<GameParticipant> {
    foundation::update("sessions.update_participant", participant)
}

pub(crate) fn ensure_participant_champion_id(
    mut participant: GameParticipant,
    champion_id: Id<domm_degens_schema::schema::Champion>,
) -> RepoResult<GameParticipant> {
    if participant
        .champion_ids
        .iter()
        .any(|id| *id == champion_id.key())
    {
        return Ok(participant);
    }
    participant.champion_ids.push(champion_id.key());
    foundation::update("sessions.ensure_participant_champion_id", participant)
}

pub(crate) fn find_participant_by_session_player(
    session_id: Id<GameSession>,
    player_id: Id<PlayerAccount>,
) -> RepoResult<Option<GameParticipant>> {
    foundation::storage_result(
        PARTICIPANT_SESSION_PLAYER_LOOKUP.name,
        crate::db()
            .load::<GameParticipant>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_participants_by_session_status(
    session_id: Id<GameSession>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<GameParticipant>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        PARTICIPANT_SESSION_STATUS_LOOKUP.name,
        crate::db()
            .load::<GameParticipant>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_participants_by_player_status(
    player_id: Id<PlayerAccount>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<GameParticipant>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        PARTICIPANT_PLAYER_STATUS_LOOKUP.name,
        crate::db()
            .load::<GameParticipant>()
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("session_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn participant_lookup_plan_text(
    session_id: Id<GameSession>,
    player_id: Id<PlayerAccount>,
) -> RepoResult<String> {
    foundation::explain_text(
        PARTICIPANT_SESSION_PLAYER_LOOKUP.name,
        crate::db()
            .load::<GameParticipant>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("player_id").eq(player_id.key()))
            .order_asc("id")
            .limit(1),
    )
}
