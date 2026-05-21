//! Repository boundary for champions, army stacks, artifacts, equipment, spells, and statuses.

use domm_degens_schema::schema::{
    ArtifactDefinition, ArtifactEquipment, ArtifactInstance, Battle, Champion, ChampionArmyStack,
    ChampionClassDefinition, ChampionSpell, GameCommand, GameParticipant, GameSession,
    SpellDefinition, UnitDefinition,
};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const CHAMPIONS_BY_OWNER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.by_owner_status",
    entity: "Champion",
    indexed_fields: &["participant_id", "status"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const CHAMPIONS_BY_SESSION_OWNER_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.by_session_owner_status",
    entity: "Champion",
    indexed_fields: &["session_id", "participant_id", "status"],
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

pub(crate) const CHAMPION_COORD_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.by_session_xy",
    entity: "Champion",
    indexed_fields: &["session_id", "x", "y"],
    bounded_limit: Some(1),
};

pub(crate) const CHAMPION_SPELLS_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "champions.spells_by_champion",
    entity: "ChampionSpell",
    indexed_fields: &["champion_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_champion(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    class_def_id: Id<ChampionClassDefinition>,
    name: String,
    class_key: String,
    status: String,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    level: u16,
    experience: u64,
    might: i16,
    guard: i16,
    wisdom: i16,
    command: i16,
    mana: u16,
    mana_max: u16,
    mana_turn: u32,
    skill_points: u16,
    skill_keys: Vec<String>,
    movement_max: u16,
    movement_remaining: u16,
    movement_turn: u32,
    vision_radius: u8,
    defeated_turn: u32,
) -> RepoResult<Champion> {
    let input: Create<Champion> = Create::<Champion> {
        session_id: Some(session_id.key()),
        participant_id: Some(participant_id.key()),
        class_def_id: Some(class_def_id.key()),
        name: Some(name),
        class_key: Some(class_key),
        status: Some(status),
        in_battle_id: Some(None),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        level: Some(level),
        experience: Some(experience),
        might: Some(might),
        guard: Some(guard),
        wisdom: Some(wisdom),
        command: Some(command),
        mana: Some(mana),
        mana_max: Some(mana_max),
        mana_turn: Some(mana_turn),
        skill_points: Some(skill_points),
        skill_keys: Some(skill_keys),
        movement_max: Some(movement_max),
        movement_remaining: Some(movement_remaining),
        movement_turn: Some(movement_turn),
        vision_radius: Some(vision_radius),
        defeated_turn: Some(defeated_turn),
        last_command_id: Some(None),
    };

    foundation::create("champions.create_champion", input)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_champion_with_id(
    champion_id: Id<Champion>,
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    class_def_id: Id<ChampionClassDefinition>,
    name: String,
    class_key: String,
    status: String,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    level: u16,
    experience: u64,
    might: i16,
    guard: i16,
    wisdom: i16,
    command: i16,
    mana: u16,
    mana_max: u16,
    mana_turn: u32,
    skill_points: u16,
    skill_keys: Vec<String>,
    movement_max: u16,
    movement_remaining: u16,
    movement_turn: u32,
    vision_radius: u8,
    defeated_turn: u32,
) -> RepoResult<Champion> {
    let now = Timestamp::now();
    let champion = Champion {
        id: champion_id.key(),
        session_id: session_id.key(),
        participant_id: participant_id.key(),
        class_def_id: class_def_id.key(),
        name,
        class_key,
        status,
        in_battle_id: None,
        x,
        y,
        chunk_x,
        chunk_y,
        level,
        experience,
        might,
        guard,
        wisdom,
        command,
        mana,
        mana_max,
        mana_turn,
        skill_points,
        skill_keys,
        movement_max,
        movement_remaining,
        movement_turn,
        vision_radius,
        defeated_turn,
        last_command_id: None,
        created_at: now,
        updated_at: now,
    };
    foundation::insert("champions.insert_champion", champion)
}

pub(crate) fn load_champion(id: Id<Champion>) -> RepoResult<Option<Champion>> {
    foundation::load_by_id("champions.load_champion", id)
}

pub(crate) fn update_champion(champion: Champion) -> RepoResult<Champion> {
    foundation::update("champions.update_champion", champion)
}

pub(crate) fn find_champion_by_session_xy(
    session_id: Id<GameSession>,
    x: u16,
    y: u16,
) -> RepoResult<Option<Champion>> {
    foundation::storage_result(
        CHAMPION_COORD_LOOKUP.name,
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("x").eq(x))
            .filter(FieldRef::new("y").eq(y))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

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

pub(crate) fn list_champions_by_session_owner_status(
    session_id: Id<GameSession>,
    owner_participant_id: Id<GameParticipant>,
    status: &str,
    limit: u32,
) -> RepoResult<Vec<Champion>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_result(
        CHAMPIONS_BY_SESSION_OWNER_LOOKUP.name,
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(owner_participant_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id")
            .limit(limit)
            .entities(),
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

pub(crate) fn list_champion_army_stacks(
    champion_id: Id<Champion>,
    limit: u32,
) -> RepoResult<Vec<ChampionArmyStack>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::storage_result(
        CHAMPION_STACKS_LOOKUP.name,
        crate::db()
            .load::<ChampionArmyStack>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("slot_index")
            .order_asc("id")
            .limit(limit)
            .entities(),
    )
}

pub(crate) fn load_champion_army_stack(
    id: Id<ChampionArmyStack>,
) -> RepoResult<Option<ChampionArmyStack>> {
    foundation::load_by_id("champions.load_army_stack", id)
}

pub(crate) fn find_champion_army_stack(
    champion_id: Id<Champion>,
    slot_index: u8,
) -> RepoResult<Option<ChampionArmyStack>> {
    foundation::storage_result(
        "champions.army_stack_by_champion_slot",
        crate::db()
            .load::<ChampionArmyStack>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .filter(FieldRef::new("slot_index").eq(slot_index))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_champion_army_stack(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    unit_id: Id<UnitDefinition>,
    slot_index: u8,
    quantity: u32,
    front_hp: u16,
    status: String,
) -> RepoResult<ChampionArmyStack> {
    let input: Create<ChampionArmyStack> = Create::<ChampionArmyStack> {
        session_id: Some(session_id.key()),
        champion_id: Some(champion_id.key()),
        unit_id: Some(unit_id.key()),
        slot_index: Some(slot_index),
        quantity: Some(quantity),
        front_hp: Some(front_hp),
        status: Some(status),
        last_command_id: Some(None),
    };

    foundation::create("champions.create_champion_army_stack", input)
}

pub(crate) fn update_champion_army_stack(
    stack: ChampionArmyStack,
) -> RepoResult<ChampionArmyStack> {
    foundation::update("champions.update_army_stack", stack)
}

pub(crate) fn page_champion_spells(
    champion_id: Id<Champion>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ChampionSpell>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        CHAMPION_SPELLS_LOOKUP.name,
        crate::db()
            .load::<ChampionSpell>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn find_champion_spell(
    champion_id: Id<Champion>,
    spell_id: Id<SpellDefinition>,
) -> RepoResult<Option<ChampionSpell>> {
    foundation::storage_result(
        "champions.spell_by_champion_spell",
        crate::db()
            .load::<ChampionSpell>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .filter(FieldRef::new("spell_id").eq(spell_id.key()))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_champion_spell(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    spell_id: Id<SpellDefinition>,
    spell_slug: &str,
    learned_turn: u32,
    command_id: Id<GameCommand>,
) -> RepoResult<ChampionSpell> {
    let input: Create<ChampionSpell> = Create::<ChampionSpell> {
        session_id: Some(session_id.key()),
        champion_id: Some(champion_id.key()),
        spell_id: Some(spell_id.key()),
        spell_slug: Some(Some(spell_slug.to_string())),
        learned_turn: Some(learned_turn),
        last_command_id: Some(Some(command_id.key())),
    };

    foundation::create("champions.create_champion_spell", input)
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_artifact_instance(
    session_id: Id<GameSession>,
    artifact_def_id: Id<ArtifactDefinition>,
    owner_champion_id: Option<Id<Champion>>,
    slot: Option<String>,
    x: u16,
    y: u16,
    chunk_x: u16,
    chunk_y: u16,
    state: String,
) -> RepoResult<ArtifactInstance> {
    let input: Create<ArtifactInstance> = Create::<ArtifactInstance> {
        session_id: Some(session_id.key()),
        artifact_def_id: Some(artifact_def_id.key()),
        owner_champion_id: Some(owner_champion_id.map(|id| id.key())),
        slot: Some(slot),
        x: Some(x),
        y: Some(y),
        chunk_x: Some(chunk_x),
        chunk_y: Some(chunk_y),
        state: Some(state),
        last_command_id: Some(None),
    };

    foundation::create("artifacts.create_artifact_instance", input)
}

pub(crate) fn load_artifact_instance(
    id: Id<ArtifactInstance>,
) -> RepoResult<Option<ArtifactInstance>> {
    foundation::load_by_id("artifacts.load_artifact_instance", id)
}

pub(crate) fn update_artifact_instance(artifact: ArtifactInstance) -> RepoResult<ArtifactInstance> {
    foundation::update("artifacts.update_artifact_instance", artifact)
}

pub(crate) fn create_artifact_equipment(
    session_id: Id<GameSession>,
    champion_id: Id<Champion>,
    artifact_id: Id<ArtifactInstance>,
    slot: String,
    equipped_turn: u32,
) -> RepoResult<ArtifactEquipment> {
    let input: Create<ArtifactEquipment> = Create::<ArtifactEquipment> {
        session_id: Some(session_id.key()),
        champion_id: Some(champion_id.key()),
        artifact_id: Some(artifact_id.key()),
        slot: Some(slot),
        equipped_turn: Some(equipped_turn),
        last_command_id: Some(None),
    };

    foundation::create("artifacts.create_artifact_equipment", input)
}

pub(crate) fn update_artifact_equipment(
    equipment: ArtifactEquipment,
) -> RepoResult<ArtifactEquipment> {
    foundation::update("artifacts.update_artifact_equipment", equipment)
}

pub(crate) fn page_artifact_equipment_by_champion(
    champion_id: Id<Champion>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<ArtifactEquipment>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "artifacts.equipment_by_champion",
        crate::db()
            .load::<ArtifactEquipment>()
            .filter(FieldRef::new("champion_id").eq(champion_id.key()))
            .order_asc("slot")
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

#[cfg(test)]
pub(crate) fn champions_by_session_owner_plan_text(
    session_id: Id<GameSession>,
    owner_participant_id: Id<GameParticipant>,
    status: &str,
    limit: u32,
) -> RepoResult<String> {
    foundation::explain_text(
        CHAMPIONS_BY_SESSION_OWNER_LOOKUP.name,
        crate::db()
            .load::<Champion>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("participant_id").eq(owner_participant_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id")
            .limit(limit),
    )
}
