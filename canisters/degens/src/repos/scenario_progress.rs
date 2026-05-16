//! Repository boundary for checkpoint 24 quest, objective, event, and rule rows.

use domm_degens_schema::schema::{
    GameParticipant, GameSession, ObjectiveProgress, QuestState, ScenarioRuleState,
    WorldEventState, WorldObject,
};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const OBJECTIVE_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.objective_by_key",
    entity: "ObjectiveProgress",
    indexed_fields: &["session_id", "objective_key"],
    bounded_limit: Some(1),
};

pub(crate) const OBJECTIVES_BY_PARTICIPANT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.objectives_by_participant",
    entity: "ObjectiveProgress",
    indexed_fields: &["session_id", "participant_id"],
    bounded_limit: Some(domm_game::MAX_OBJECTIVE_ROWS_PER_SESSION),
};

pub(crate) const OBJECTIVES_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.objectives_by_status",
    entity: "ObjectiveProgress",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_OBJECTIVE_ROWS_PER_SESSION),
};

pub(crate) const QUEST_BY_PARTICIPANT_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.quest_by_participant_key",
    entity: "QuestState",
    indexed_fields: &["session_id", "participant_id", "quest_key"],
    bounded_limit: Some(1),
};

pub(crate) const QUESTS_BY_PARTICIPANT_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.quests_by_participant_status",
    entity: "QuestState",
    indexed_fields: &["session_id", "participant_id", "status"],
    bounded_limit: Some(domm_game::MAX_ACTIVE_QUESTS_PER_PARTICIPANT),
};

pub(crate) const WORLD_EVENT_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.world_event_by_key",
    entity: "WorldEventState",
    indexed_fields: &["session_id", "event_key"],
    bounded_limit: Some(1),
};

pub(crate) const WORLD_EVENTS_BY_WINDOW_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.world_events_by_window",
    entity: "WorldEventState",
    indexed_fields: &["session_id", "event_window"],
    bounded_limit: Some(domm_game::MAX_WORLD_EVENT_ROWS_PER_SESSION),
};

pub(crate) const SCENARIO_RULE_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.rule_by_key",
    entity: "ScenarioRuleState",
    indexed_fields: &["session_id", "rule_key"],
    bounded_limit: Some(1),
};

pub(crate) const SCENARIO_RULES_BY_STATE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.rules_by_victory_state",
    entity: "ScenarioRuleState",
    indexed_fields: &["session_id", "victory_state"],
    bounded_limit: Some(domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION),
};

pub(crate) const SCENARIO_RULES_BY_STATUS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "scenario.rules_by_status",
    entity: "ScenarioRuleState",
    indexed_fields: &["session_id", "status"],
    bounded_limit: Some(domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION),
};

pub(crate) fn find_objective_by_key(
    session_id: Id<GameSession>,
    objective_key: &str,
) -> RepoResult<Option<ObjectiveProgress>> {
    foundation::storage_result(
        OBJECTIVE_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<ObjectiveProgress>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("objective_key").eq(objective_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_objectives_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<ObjectiveProgress>> {
    foundation::execute_page(
        OBJECTIVES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<ObjectiveProgress>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("objective_key")
            .order_asc("id"),
        domm_game::MAX_OBJECTIVE_ROWS_PER_SESSION,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_objective_progress(
    session_id: Id<GameSession>,
    participant_id: Option<Id<GameParticipant>>,
    object_id: Option<Id<WorldObject>>,
    objective_key: String,
    objective_type: String,
    progress_value: u32,
    required_value: u32,
    status: String,
    visible_to: String,
    last_scored_turn: u32,
) -> RepoResult<ObjectiveProgress> {
    let input: Create<ObjectiveProgress> = Create::<ObjectiveProgress> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.map(|id| id.key())),
        object_id: Some(object_id.map(|id| id.key())),
        objective_key: Some(objective_key),
        objective_type: Some(objective_type),
        progress_value: Some(progress_value),
        required_value: Some(required_value),
        status: Some(status),
        visible_to: Some(visible_to),
        last_scored_turn: Some(last_scored_turn),
        last_command_id: Some(None),
    };

    foundation::create("scenario.create_objective_progress", input)
}

pub(crate) fn update_objective_progress(row: ObjectiveProgress) -> RepoResult<ObjectiveProgress> {
    foundation::update("scenario.update_objective_progress", row)
}

pub(crate) fn find_quest_by_participant_key(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    quest_key: &str,
) -> RepoResult<Option<QuestState>> {
    foundation::storage_result(
        QUEST_BY_PARTICIPANT_KEY_LOOKUP.name,
        crate::db()
            .load::<QuestState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("quest_key").eq(quest_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_quests_by_participant_status(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    status: &str,
) -> RepoResult<RepositoryPage<QuestState>> {
    foundation::execute_page(
        QUESTS_BY_PARTICIPANT_STATUS_LOOKUP.name,
        crate::db()
            .load::<QuestState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("quest_key")
            .order_asc("id"),
        domm_game::MAX_ACTIVE_QUESTS_PER_PARTICIPANT,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_quest_state(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    quest_key: String,
    title: String,
    objective_key: String,
    status: String,
    progress_value: u32,
    required_value: u32,
    reward_gold: u32,
) -> RepoResult<QuestState> {
    let input: Create<QuestState> = Create::<QuestState> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        quest_key: Some(quest_key),
        title: Some(title),
        objective_key: Some(objective_key),
        status: Some(status),
        progress_value: Some(progress_value),
        required_value: Some(required_value),
        reward_gold: Some(reward_gold),
        accepted_turn: Some(0),
        claimed_turn: Some(0),
        accepted_command_id: Some(None),
        claimed_command_id: Some(None),
        last_command_id: Some(None),
    };

    foundation::create("scenario.create_quest_state", input)
}

pub(crate) fn update_quest_state(row: QuestState) -> RepoResult<QuestState> {
    foundation::update("scenario.update_quest_state", row)
}

pub(crate) fn find_world_event_by_key(
    session_id: Id<GameSession>,
    event_key: &str,
) -> RepoResult<Option<WorldEventState>> {
    foundation::storage_result(
        WORLD_EVENT_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<WorldEventState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("event_key").eq(event_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_world_events_by_window(
    session_id: Id<GameSession>,
    event_window: &str,
) -> RepoResult<RepositoryPage<WorldEventState>> {
    foundation::execute_page(
        WORLD_EVENTS_BY_WINDOW_LOOKUP.name,
        crate::db()
            .load::<WorldEventState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("event_window").eq(event_window))
            .order_asc("starts_turn")
            .order_asc("id"),
        domm_game::MAX_WORLD_EVENT_ROWS_PER_SESSION,
        None,
    )
}

pub(crate) fn page_world_events_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<WorldEventState>> {
    foundation::execute_page(
        "scenario.world_events_by_status",
        crate::db()
            .load::<WorldEventState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("starts_turn")
            .order_asc("id"),
        domm_game::MAX_WORLD_EVENT_ROWS_PER_SESSION,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_world_event_state(
    session_id: Id<GameSession>,
    event_key: String,
    event_type: String,
    event_window: String,
    starts_turn: u32,
    ends_turn: u32,
    status: String,
    payload_json: String,
) -> RepoResult<WorldEventState> {
    let input: Create<WorldEventState> = Create::<WorldEventState> {
        session_id: Some(session_id.key()),
        event_key: Some(event_key),
        event_type: Some(event_type),
        event_window: Some(event_window),
        starts_turn: Some(starts_turn),
        ends_turn: Some(ends_turn),
        status: Some(status),
        payload_json: Some(payload_json),
        last_command_id: Some(None),
    };

    foundation::create("scenario.create_world_event_state", input)
}

pub(crate) fn update_world_event_state(row: WorldEventState) -> RepoResult<WorldEventState> {
    foundation::update("scenario.update_world_event_state", row)
}

pub(crate) fn find_scenario_rule_by_key(
    session_id: Id<GameSession>,
    rule_key: &str,
) -> RepoResult<Option<ScenarioRuleState>> {
    foundation::storage_result(
        SCENARIO_RULE_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<ScenarioRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("rule_key").eq(rule_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_scenario_rules_by_state(
    session_id: Id<GameSession>,
    victory_state: &str,
) -> RepoResult<RepositoryPage<ScenarioRuleState>> {
    foundation::execute_page(
        SCENARIO_RULES_BY_STATE_LOOKUP.name,
        crate::db()
            .load::<ScenarioRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("victory_state").eq(victory_state))
            .order_asc("rule_key")
            .order_asc("id"),
        domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION,
        None,
    )
}

pub(crate) fn page_scenario_rules_by_status(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<RepositoryPage<ScenarioRuleState>> {
    foundation::execute_page(
        SCENARIO_RULES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<ScenarioRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("rule_key")
            .order_asc("id"),
        domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_scenario_rule_state(
    session_id: Id<GameSession>,
    rule_key: String,
    rule_type: String,
    status: String,
    victory_state: String,
    required_value: u32,
    current_value: u32,
    owner_participant_id: Option<Id<GameParticipant>>,
    winner_participant_id: Option<Id<GameParticipant>>,
    disabled_reason: Option<String>,
    last_checked_turn: u32,
) -> RepoResult<ScenarioRuleState> {
    let input: Create<ScenarioRuleState> = Create::<ScenarioRuleState> {
        session_id: Some(session_id.key()),
        rule_key: Some(rule_key),
        rule_type: Some(rule_type),
        status: Some(status),
        victory_state: Some(victory_state),
        required_value: Some(required_value),
        current_value: Some(current_value),
        owner_participant_id: Some(owner_participant_id.map(|id| id.key())),
        winner_participant_id: Some(winner_participant_id.map(|id| id.key())),
        disabled_reason: Some(disabled_reason),
        last_checked_turn: Some(last_checked_turn),
        last_command_id: Some(None),
    };

    foundation::create("scenario.create_scenario_rule_state", input)
}

pub(crate) fn update_scenario_rule_state(row: ScenarioRuleState) -> RepoResult<ScenarioRuleState> {
    foundation::update("scenario.update_scenario_rule_state", row)
}

#[cfg(test)]
pub(crate) fn objective_plan_text(
    session_id: Id<GameSession>,
    objective_key: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        OBJECTIVE_BY_KEY_LOOKUP.name,
        crate::db()
            .load::<ObjectiveProgress>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("objective_key").eq(objective_key))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn quest_plan_text(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    quest_key: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        QUEST_BY_PARTICIPANT_KEY_LOOKUP.name,
        crate::db()
            .load::<QuestState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(participant_id.key()))
            .filter(FieldRef::new("quest_key").eq(quest_key))
            .order_asc("id")
            .limit(1),
    )
}

#[cfg(test)]
pub(crate) fn world_event_plan_text(
    session_id: Id<GameSession>,
    event_window: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        WORLD_EVENTS_BY_WINDOW_LOOKUP.name,
        crate::db()
            .load::<WorldEventState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("event_window").eq(event_window))
            .order_asc("starts_turn")
            .order_asc("id")
            .limit(domm_game::MAX_WORLD_EVENT_ROWS_PER_SESSION),
    )
}

#[cfg(test)]
pub(crate) fn scenario_rule_plan_text(
    session_id: Id<GameSession>,
    victory_state: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        SCENARIO_RULES_BY_STATE_LOOKUP.name,
        crate::db()
            .load::<ScenarioRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("victory_state").eq(victory_state))
            .order_asc("rule_key")
            .order_asc("id")
            .limit(domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION),
    )
}

#[cfg(test)]
pub(crate) fn scenario_rule_status_plan_text(
    session_id: Id<GameSession>,
    status: &str,
) -> RepoResult<String> {
    foundation::explain_text(
        SCENARIO_RULES_BY_STATUS_LOOKUP.name,
        crate::db()
            .load::<ScenarioRuleState>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("rule_key")
            .order_asc("id")
            .limit(domm_game::MAX_SCENARIO_RULE_ROWS_PER_SESSION),
    )
}
