//! Repository boundary for champions, army stacks, artifacts, equipment, spells, and statuses.

use domm_degens_schema::schema::{
    ArtifactEquipment, ArtifactInstance, Battle, Champion, ChampionArmyStack, GameParticipant,
    GameSession,
};
use icydb::{db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const CHAMPIONS_BY_OWNER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.by_owner_status",
    entity: "Champion",
    indexed_fields: &["participant_id", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const CHAMPION_STACKS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.army_stacks_by_champion",
    entity: "ChampionArmyStack",
    indexed_fields: &["champion_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const ARTIFACT_EQUIPMENT_SLOT_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "artifacts.equipment_by_champion_slot",
    entity: "ArtifactEquipment",
    indexed_fields: &["champion_id", "slot"],
    bounded_limit: Some(1),
};

pub(crate) const ARTIFACTS_BY_SESSION_STATE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "artifacts.by_session_state",
    entity: "ArtifactInstance",
    indexed_fields: &["session_id", "state"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) fn page_champions_by_owner_status(
    owner_participant_id: Id<GameParticipant>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Champion>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        CHAMPIONS_BY_OWNER_LOOKUP.name,
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("participant_id").eq(owner_participant_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_champion_army_stacks(
    champion_id: Id<Champion>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ChampionArmyStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        CHAMPION_STACKS_LOOKUP.name,
        crate::db()
            .load::<ChampionArmyStack>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("slot_index")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_equipment_by_champion_slot(
    champion_id: Id<Champion>,
    slot: &str,
) -> RepoResult<Option<ArtifactEquipment>> {
    foundation::storage_result(
        ARTIFACT_EQUIPMENT_SLOT_LOOKUP.name,
        crate::db()
            .load::<ArtifactEquipment>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .filter(FieldRef::new("slot").eq(slot))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_session_artifacts_by_state(
    session_id: Id<GameSession>,
    state: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ArtifactInstance>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        ARTIFACTS_BY_SESSION_STATE_LOOKUP.name,
        crate::db()
            .load::<ArtifactInstance>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("state").eq(state))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_champions_in_battle(
    battle_id: Id<Battle>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<Champion>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "champions.by_battle",
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("in_battle_id").eq(battle_id.key()))
            .order_asc("id"),
        limit,
        cursor,
    )
}

#[cfg(test)]
pub(crate) fn champions_by_owner_plan_text(
    owner_participant_id: Id<GameParticipant>,
    status: &str,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        CHAMPIONS_BY_OWNER_LOOKUP.name,
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("participant_id").eq(owner_participant_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id")
            .limit(limit),
    )
}
