use std::any::TypeId;

use domm_degens_schema::schema::{
    AiActorState, ArtifactDefinition, ArtifactEquipment, ArtifactInstance, Battle, BattleObstacle,
    BattleOccupancy, BattleStack, BuildingDefinition, COMMIT_MEMORY_ID, Champion,
    ChampionArmyStack, ChampionClassDefinition, ChampionObjectVisit, ChampionSpell, CommandEffect,
    DATA_MEMORY_ID, DegensCanister, DegensStore, FactionDefinition, GameCommand, GameEvent,
    GameEventTurnSummary, GameParticipant, GameSession, INDEX_MEMORY_ID, LobbyCommand, MapChunk,
    MapObjectDefinition, MapOccupancy, MovementIntent, NeutralArmy, NeutralArmyStack,
    ParticipantKnownObject, ParticipantObjectVisit, PendingEffect, PlayerAccount,
    PlayerMatchSummary, ResourceLedgerEntry, ResourceLedgerTurnSummary, RulesetDefinition,
    SCHEMA_MEMORY_ID, SpellDefinition, TerrainDefinition, Town, TownBuilding, TownGarrisonStack,
    TownRecruitPool, UnitDefinition, VisibilityChunk, WorldObject,
};
use icydb::{
    model::{
        EntityModel,
        field::{FieldKind, RelationStrength},
    },
    traits::EntitySchema,
};

#[test]
fn schema_canister_and_store_types_are_available() {
    let canister = TypeId::of::<DegensCanister>();
    let store = TypeId::of::<DegensStore>();

    assert_ne!(canister, store);
}

#[test]
fn schema_memory_ids_match_spec_checkpoint_zero_baseline() {
    assert_eq!(DATA_MEMORY_ID, 20);
    assert_eq!(INDEX_MEMORY_ID, 21);
    assert_eq!(SCHEMA_MEMORY_ID, 22);
    assert_eq!(COMMIT_MEMORY_ID, 119);
}

#[test]
fn part_two_entity_surface_is_registered() {
    let names = entity_models()
        .iter()
        .map(|model| model.name())
        .collect::<Vec<_>>();

    assert_eq!(names.len(), 45);
    assert!(names.contains(&"PlayerAccount"));
    assert!(names.contains(&"GameSession"));
    assert!(names.contains(&"GameCommand"));
    assert!(names.contains(&"PendingEffect"));
}

#[test]
fn important_unique_indexes_match_checkpoint_one_invariants() {
    assert_unique_index(PlayerAccount::MODEL, &["account_principal"]);
    assert_unique_index(PlayerAccount::MODEL, &["username"]);
    assert_unique_index(GameParticipant::MODEL, &["session_id", "player_id"]);
    assert_unique_index(GameParticipant::MODEL, &["session_id", "slot_index"]);
    assert_unique_index(
        GameCommand::MODEL,
        &["session_id", "actor_kind", "actor_id_text", "client_nonce"],
    );
    assert_unique_index(LobbyCommand::MODEL, &["actor_principal", "client_nonce"]);
    assert_unique_index(GameEvent::MODEL, &["session_id", "event_seq"]);
    assert_unique_index(GameEvent::MODEL, &["session_id", "event_key"]);
    assert_unique_index(MapOccupancy::MODEL, &["session_id", "x", "y", "layer"]);
    assert_unique_index(
        MapOccupancy::MODEL,
        &[
            "session_id",
            "occupant_kind",
            "occupant_id_text",
            "occupant_cell_index",
        ],
    );
    assert_unique_index(
        MovementIntent::MODEL,
        &["session_id", "champion_id", "turn_number"],
    );
    assert_unique_index(ResourceLedgerEntry::MODEL, &["command_id", "ledger_key"]);
    assert_unique_index(CommandEffect::MODEL, &["command_id", "effect_key"]);
    assert_unique_index(
        BattleOccupancy::MODEL,
        &["battle_id", "battle_x", "battle_y"],
    );
    assert_unique_index(BattleOccupancy::MODEL, &["battle_stack_id"]);
}

#[test]
fn relation_strengths_match_cleanup_assumptions() {
    assert_relation_strength(
        GameParticipant::MODEL,
        "session_id",
        RelationStrength::Strong,
    );
    assert_relation_strength(
        GameParticipant::MODEL,
        "player_id",
        RelationStrength::Strong,
    );
    assert_relation_strength(
        GameSession::MODEL,
        "last_command_id",
        RelationStrength::Weak,
    );
    assert_relation_strength(
        PlayerMatchSummary::MODEL,
        "session_id",
        RelationStrength::Weak,
    );
    assert_relation_strength(GameEvent::MODEL, "command_id", RelationStrength::Weak);
    assert_relation_strength(TownBuilding::MODEL, "town_id", RelationStrength::Strong);
    assert_relation_strength(Champion::MODEL, "last_command_id", RelationStrength::Weak);
    assert_relation_strength(
        PendingEffect::MODEL,
        "target_participant_id",
        RelationStrength::Strong,
    );
}

fn entity_models() -> Vec<&'static EntityModel> {
    vec![
        PlayerAccount::MODEL,
        RulesetDefinition::MODEL,
        GameSession::MODEL,
        GameParticipant::MODEL,
        ResourceLedgerEntry::MODEL,
        ResourceLedgerTurnSummary::MODEL,
        PlayerMatchSummary::MODEL,
        AiActorState::MODEL,
        FactionDefinition::MODEL,
        ChampionClassDefinition::MODEL,
        TerrainDefinition::MODEL,
        UnitDefinition::MODEL,
        BuildingDefinition::MODEL,
        SpellDefinition::MODEL,
        ArtifactDefinition::MODEL,
        MapObjectDefinition::MODEL,
        MapChunk::MODEL,
        VisibilityChunk::MODEL,
        MapOccupancy::MODEL,
        WorldObject::MODEL,
        ParticipantObjectVisit::MODEL,
        ChampionObjectVisit::MODEL,
        ParticipantKnownObject::MODEL,
        Town::MODEL,
        TownBuilding::MODEL,
        TownRecruitPool::MODEL,
        TownGarrisonStack::MODEL,
        Champion::MODEL,
        ChampionArmyStack::MODEL,
        ChampionSpell::MODEL,
        ArtifactInstance::MODEL,
        ArtifactEquipment::MODEL,
        NeutralArmy::MODEL,
        NeutralArmyStack::MODEL,
        Battle::MODEL,
        BattleObstacle::MODEL,
        BattleStack::MODEL,
        BattleOccupancy::MODEL,
        GameCommand::MODEL,
        LobbyCommand::MODEL,
        MovementIntent::MODEL,
        CommandEffect::MODEL,
        GameEvent::MODEL,
        GameEventTurnSummary::MODEL,
        PendingEffect::MODEL,
    ]
}

fn assert_unique_index(model: &EntityModel, fields: &[&str]) {
    let Some(index) = model
        .indexes()
        .iter()
        .find(|index| index.fields() == fields)
    else {
        panic!("{} missing index over {fields:?}", model.name());
    };

    assert!(
        index.is_unique(),
        "{} index over {fields:?} should be unique",
        model.name()
    );
}

fn assert_relation_strength(model: &EntityModel, field_name: &str, expected: RelationStrength) {
    let Some(field) = model
        .fields()
        .iter()
        .find(|field| field.name() == field_name)
    else {
        panic!("{} missing field {field_name}", model.name());
    };

    match field.kind() {
        FieldKind::Relation { strength, .. } => assert_eq!(strength, expected),
        other => panic!(
            "{}.{field_name} should be a relation, got {other:?}",
            model.name()
        ),
    }
}
