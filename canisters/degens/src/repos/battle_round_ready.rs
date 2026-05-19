//! Repository boundary for tactical battle-round readiness markers.

use domm_degens_schema::schema::{
    Battle, BattleParticipantRoundReady, GameCommand, GameParticipant, GameSession,
};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const BATTLE_ROUND_READY_UNIQUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battle_round_ready.by_battle_participant_round",
    entity: "BattleParticipantRoundReady",
    indexed_fields: &["battle_id", "participant_id", "round_number"],
    bounded_limit: Some(1),
};

pub(crate) const BATTLE_ROUND_READY_BY_SESSION_BATTLE_ROUND_LOOKUP: IndexedQueryPlan =
    IndexedQueryPlan {
        name: "battle_round_ready.by_session_battle_round",
        entity: "BattleParticipantRoundReady",
        indexed_fields: &["session_id", "battle_id", "round_number"],
        bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
    };

pub(crate) const BATTLE_ROUND_READY_BY_COMMAND_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "battle_round_ready.by_command",
    entity: "BattleParticipantRoundReady",
    indexed_fields: &["command_id"],
    bounded_limit: Some(1),
};

pub(crate) fn create_battle_round_ready(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    participant_id: Id<GameParticipant>,
    round_number: u16,
    command_id: Option<Id<GameCommand>>,
    ready_reason: String,
    _ended_at: Timestamp,
) -> RepoResult<BattleParticipantRoundReady> {
    let input: Create<BattleParticipantRoundReady> = Create::<BattleParticipantRoundReady> {
        session_id: Some(session_id.key()),
        battle_id: Some(battle_id.key()),
        participant_id: Some(participant_id.key()),
        round_number: Some(round_number),
        command_id: Some(command_id.map(|id| id.key())),
        ready_reason: Some(ready_reason),
    };

    foundation::create("battle_round_ready.create_battle_round_ready", input)
}

pub(crate) fn mark_battle_round_ready(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    participant_id: Id<GameParticipant>,
    round_number: u16,
    command_id: Option<Id<GameCommand>>,
    ready_reason: String,
    ended_at: Timestamp,
) -> RepoResult<BattleParticipantRoundReady> {
    if let Some(mut ready) = find_battle_round_ready(battle_id, participant_id, round_number)? {
        if ready.command_id.is_none() {
            ready.command_id = command_id.map(|id| id.key());
            ready.ready_reason = ready_reason;
            return update_battle_round_ready(ready);
        }
        return Ok(ready);
    }

    create_battle_round_ready(
        session_id,
        battle_id,
        participant_id,
        round_number,
        command_id,
        ready_reason,
        ended_at,
    )
}

pub(crate) fn load_battle_round_ready(
    id: Id<BattleParticipantRoundReady>,
) -> RepoResult<Option<BattleParticipantRoundReady>> {
    foundation::load_by_id("battle_round_ready.load_battle_round_ready", id)
}

pub(crate) fn update_battle_round_ready(
    ready: BattleParticipantRoundReady,
) -> RepoResult<BattleParticipantRoundReady> {
    foundation::update("battle_round_ready.update_battle_round_ready", ready)
}

pub(crate) fn find_battle_round_ready(
    battle_id: Id<Battle>,
    participant_id: Id<GameParticipant>,
    round_number: u16,
) -> RepoResult<Option<BattleParticipantRoundReady>> {
    foundation::storage_operation(BATTLE_ROUND_READY_UNIQUE_LOOKUP.name, || {
        crate::db()
            .load::<BattleParticipantRoundReady>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("round_number").eq(round_number))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn page_battle_round_ready(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    round_number: u16,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<BattleParticipantRoundReady>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        BATTLE_ROUND_READY_BY_SESSION_BATTLE_ROUND_LOOKUP.name,
        crate::db()
            .load::<BattleParticipantRoundReady>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("round_number").eq(round_number))
            .order_asc("participant_id")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_battle_round_ready_by_command(
    command_id: Id<GameCommand>,
) -> RepoResult<Option<BattleParticipantRoundReady>> {
    foundation::storage_operation(BATTLE_ROUND_READY_BY_COMMAND_LOOKUP.name, || {
        crate::db()
            .load::<BattleParticipantRoundReady>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

#[cfg(test)]
pub(crate) fn battle_round_ready_plan_text(
    session_id: Id<GameSession>,
    battle_id: Id<Battle>,
    round_number: u16,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        BATTLE_ROUND_READY_BY_SESSION_BATTLE_ROUND_LOOKUP.name,
        crate::db()
            .load::<BattleParticipantRoundReady>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("round_number").eq(round_number))
            .order_asc("participant_id")
            .order_asc("id")
            .limit(limit),
    )
}
