use std::any::TypeId;

use domm_degens_schema::schema::{
    AiActorState, ArtifactDefinition, ArtifactEquipment, ArtifactInstance, Battle, BattleObstacle,
    BattleOccupancy, BattleStack, BuildingDefinition, COMMIT_MEMORY_ID, Champion,
    ChampionArmyStack, ChampionClassDefinition, ChampionObjectVisit, ChampionSpell, CommandEffect,
    DATA_MEMORY_ID, DegensCanister, DegensStore, FactionDefinition, GameCommand, GameEvent,
    GameEventTurnSummary, GameParticipant, GameSession, INDEX_MEMORY_ID, LobbyCommand, MapChunk,
    MapObjectDefinition, MapOccupancy, MovementIntent, MovementSnapshot, NeutralArmy,
    NeutralArmyStack, ObjectiveProgress, ParticipantKnownObject, ParticipantObjectVisit,
    PendingEffect, PlayerAccount, PlayerMatchSummary, QuestState, ResourceLedgerEntry,
    ResourceLedgerTurnSummary, RulesetDefinition, SCHEMA_MEMORY_ID, ScenarioRuleState,
    SpellDefinition, TerrainDefinition, Town, TownBuilding, TownGarrisonStack, TownRecruitPool,
    UnitDefinition, VisibilityChunk, WorldEventState, WorldObject,
};
use icydb::{
    model::{
        EntityModel,
        field::{FieldDatabaseDefault, FieldInsertGeneration, FieldKind, RelationStrength},
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

    assert_eq!(names.len(), 50);
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
    assert_unique_index(
        MovementSnapshot::MODEL,
        &["command_id", "intent_id", "step_index"],
    );
    assert_unique_index(ResourceLedgerEntry::MODEL, &["command_id", "ledger_key"]);
    assert_unique_index(CommandEffect::MODEL, &["command_id", "effect_key"]);
    assert_unique_index(PendingEffect::MODEL, &["session_id", "effect_key"]);
    assert_unique_index(
        GameEventTurnSummary::MODEL,
        &["session_id", "audience_key", "turn_number"],
    );
    assert_unique_index(
        BattleOccupancy::MODEL,
        &["battle_id", "battle_x", "battle_y"],
    );
    assert_unique_index(BattleOccupancy::MODEL, &["battle_stack_id"]);
    assert_unique_index(ObjectiveProgress::MODEL, &["session_id", "objective_key"]);
    assert_unique_index(
        QuestState::MODEL,
        &["session_id", "participant_id", "quest_key"],
    );
    assert_unique_index(WorldEventState::MODEL, &["session_id", "event_key"]);
    assert_unique_index(ScenarioRuleState::MODEL, &["session_id", "rule_key"]);
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
    assert_relation_strength(
        MovementSnapshot::MODEL,
        "command_id",
        RelationStrength::Weak,
    );
    assert_relation_strength(TownBuilding::MODEL, "town_id", RelationStrength::Strong);
    assert_relation_strength(Champion::MODEL, "last_command_id", RelationStrength::Weak);
    assert_relation_strength(
        PendingEffect::MODEL,
        "target_participant_id",
        RelationStrength::Strong,
    );
}

#[test]
fn weak_history_relations_remain_safe_for_retained_rows() {
    for (model, field_name) in [
        (GameSession::MODEL, "last_command_id"),
        (GameParticipant::MODEL, "last_command_id"),
        (GameParticipant::MODEL, "last_resource_command_id"),
        (PlayerMatchSummary::MODEL, "session_id"),
        (PlayerMatchSummary::MODEL, "player_id"),
        (ResourceLedgerEntry::MODEL, "command_id"),
        (ResourceLedgerTurnSummary::MODEL, "participant_id"),
        (Battle::MODEL, "attacker_champion_id"),
        (Battle::MODEL, "defender_champion_id"),
        (Battle::MODEL, "defender_town_id"),
        (Battle::MODEL, "defender_neutral_army_id"),
        (Battle::MODEL, "winner_participant_id"),
        (BattleStack::MODEL, "owner_participant_id"),
        (GameCommand::MODEL, "actor_player_id"),
        (GameCommand::MODEL, "actor_participant_id"),
        (GameCommand::MODEL, "champion_id"),
        (MovementSnapshot::MODEL, "command_id"),
        (GameEvent::MODEL, "command_id"),
        (GameEvent::MODEL, "actor_participant_id"),
        (CommandEffect::MODEL, "command_id"),
        (PendingEffect::MODEL, "source_command_id"),
        (ArtifactInstance::MODEL, "owner_champion_id"),
        (ObjectiveProgress::MODEL, "last_command_id"),
        (QuestState::MODEL, "accepted_command_id"),
        (QuestState::MODEL, "claimed_command_id"),
        (QuestState::MODEL, "last_command_id"),
        (WorldEventState::MODEL, "last_command_id"),
        (ScenarioRuleState::MODEL, "owner_participant_id"),
        (ScenarioRuleState::MODEL, "winner_participant_id"),
        (ScenarioRuleState::MODEL, "last_command_id"),
    ] {
        assert_relation_strength(model, field_name, RelationStrength::Weak);
    }
}

#[test]
fn hot_entity_fields_are_append_only_prefixes() {
    assert_field_prefix(
        GameSession::MODEL,
        &[
            "id",
            "ruleset_id",
            "created_by_player_id",
            "name",
            "state",
            "seed",
            "map_width",
            "map_height",
            "chunk_size",
            "simultaneous_turns",
            "turn_duration_ms",
            "max_turns",
            "turn_catchup_cap",
            "current_turn",
            "next_event_seq",
            "turn_started_at",
            "turn_deadline_at",
            "winner_participant_id",
            "finish_reason",
            "last_command_id",
        ],
    );
    assert_field_prefix(
        GameCommand::MODEL,
        &[
            "id",
            "session_id",
            "actor_kind",
            "actor_id_text",
            "actor_player_id",
            "actor_participant_id",
            "champion_id",
            "turn_number",
            "client_nonce",
            "command_type",
            "status",
            "phase",
            "payload_hash",
            "payload_json",
            "result_json",
            "error_code",
            "error_message",
            "error_details_json",
            "retryable",
            "applied_at",
            "failed_at",
        ],
    );
    assert_field_prefix(
        MovementIntent::MODEL,
        &[
            "id",
            "session_id",
            "turn_number",
            "actor_participant_id",
            "champion_id",
            "command_id",
            "status",
            "path_json",
            "path_hash",
            "submitted_at",
            "resolved_at",
        ],
    );
    assert_field_prefix(
        MovementSnapshot::MODEL,
        &[
            "id",
            "session_id",
            "command_id",
            "intent_id",
            "champion_id",
            "participant_id",
            "turn_number",
            "step_index",
            "from_x",
            "from_y",
            "to_x",
            "to_y",
            "movement_cost",
            "remaining_after",
            "outcome",
            "interaction_kind",
            "interaction_id_text",
        ],
    );
    assert_field_prefix(
        GameEvent::MODEL,
        &[
            "id",
            "session_id",
            "command_id",
            "actor_participant_id",
            "turn_number",
            "event_seq",
            "event_key",
            "audience_key",
            "event_type",
            "subject_kind",
            "subject_id_text",
            "payload_json",
        ],
    );
}

#[test]
fn database_defaults_and_generation_contracts_are_explicit() {
    assert_generated_insert(PlayerAccount::MODEL, "id", FieldInsertGeneration::Ulid);
    assert_generated_insert(GameSession::MODEL, "id", FieldInsertGeneration::Ulid);
    assert_generated_insert(
        GameSession::MODEL,
        "turn_started_at",
        FieldInsertGeneration::Timestamp,
    );
    assert_no_database_default(PlayerAccount::MODEL, "id");
    assert_no_database_default(GameSession::MODEL, "turn_started_at");
    assert_database_default(GameSession::MODEL, "next_event_seq");
    assert_database_default(GameSession::MODEL, "state");
    assert_database_default(GameParticipant::MODEL, "status");
    assert_database_default(GameEvent::MODEL, "audience_key");
}

#[test]
fn composite_indexes_keep_stable_declarations_and_ordinals() {
    assert_index_ordinal(
        GameCommand::MODEL,
        &["session_id", "actor_kind", "actor_id_text", "client_nonce"],
        0,
    );
    assert_index_ordinal(
        GameCommand::MODEL,
        &["session_id", "status", "created_at"],
        1,
    );
    assert_index_ordinal(GameEvent::MODEL, &["session_id", "event_seq"], 0);
    assert_index_ordinal(
        GameEvent::MODEL,
        &["session_id", "audience_key", "event_seq"],
        6,
    );
    assert_index_ordinal(
        MovementIntent::MODEL,
        &["session_id", "champion_id", "turn_number"],
        1,
    );
    assert_index_ordinal(
        MovementSnapshot::MODEL,
        &["command_id", "intent_id", "step_index"],
        0,
    );
    assert_index_ordinal(
        MapOccupancy::MODEL,
        &[
            "session_id",
            "occupant_kind",
            "occupant_id_text",
            "occupant_cell_index",
        ],
        3,
    );
}

#[test]
fn deletion_policy_lists_strong_children_before_targets() {
    let policy = deletion_policy_order();

    for edge in [
        DeleteEdge::new(AiActorState::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(ResourceLedgerEntry::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            ResourceLedgerEntry::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(TownBuilding::MODEL, "town_id", Town::MODEL),
        DeleteEdge::new(TownBuilding::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(TownRecruitPool::MODEL, "town_id", Town::MODEL),
        DeleteEdge::new(TownRecruitPool::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(TownGarrisonStack::MODEL, "town_id", Town::MODEL),
        DeleteEdge::new(TownGarrisonStack::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(Town::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(Town::MODEL, "owner_participant_id", GameParticipant::MODEL),
        DeleteEdge::new(Champion::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(Champion::MODEL, "participant_id", GameParticipant::MODEL),
        DeleteEdge::new(ChampionArmyStack::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(ChampionArmyStack::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(ChampionSpell::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(ChampionSpell::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(ArtifactInstance::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            ArtifactEquipment::MODEL,
            "artifact_id",
            ArtifactInstance::MODEL,
        ),
        DeleteEdge::new(ArtifactEquipment::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(ArtifactEquipment::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(NeutralArmy::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            NeutralArmyStack::MODEL,
            "neutral_army_id",
            NeutralArmy::MODEL,
        ),
        DeleteEdge::new(NeutralArmyStack::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(Battle::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(BattleObstacle::MODEL, "battle_id", Battle::MODEL),
        DeleteEdge::new(BattleStack::MODEL, "battle_id", Battle::MODEL),
        DeleteEdge::new(BattleOccupancy::MODEL, "battle_id", Battle::MODEL),
        DeleteEdge::new(
            BattleOccupancy::MODEL,
            "battle_stack_id",
            BattleStack::MODEL,
        ),
        DeleteEdge::new(MovementIntent::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(MovementSnapshot::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(MovementSnapshot::MODEL, "intent_id", MovementIntent::MODEL),
        DeleteEdge::new(MovementSnapshot::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(
            MovementSnapshot::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(
            MovementIntent::MODEL,
            "actor_participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(MovementIntent::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(PendingEffect::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(PendingEffect::MODEL, "target_champion_id", Champion::MODEL),
        DeleteEdge::new(
            PendingEffect::MODEL,
            "target_participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(GameParticipant::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(MapChunk::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(VisibilityChunk::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            VisibilityChunk::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(MapOccupancy::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(WorldObject::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            WorldObject::MODEL,
            "owner_participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(ObjectiveProgress::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            ObjectiveProgress::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(ObjectiveProgress::MODEL, "object_id", WorldObject::MODEL),
        DeleteEdge::new(QuestState::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(QuestState::MODEL, "participant_id", GameParticipant::MODEL),
        DeleteEdge::new(WorldEventState::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(ScenarioRuleState::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            ParticipantObjectVisit::MODEL,
            "session_id",
            GameSession::MODEL,
        ),
        DeleteEdge::new(
            ParticipantObjectVisit::MODEL,
            "object_id",
            WorldObject::MODEL,
        ),
        DeleteEdge::new(
            ParticipantObjectVisit::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(ChampionObjectVisit::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(ChampionObjectVisit::MODEL, "object_id", WorldObject::MODEL),
        DeleteEdge::new(ChampionObjectVisit::MODEL, "champion_id", Champion::MODEL),
        DeleteEdge::new(
            ParticipantKnownObject::MODEL,
            "session_id",
            GameSession::MODEL,
        ),
        DeleteEdge::new(
            ParticipantKnownObject::MODEL,
            "participant_id",
            GameParticipant::MODEL,
        ),
        DeleteEdge::new(GameCommand::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(CommandEffect::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(GameEvent::MODEL, "session_id", GameSession::MODEL),
        DeleteEdge::new(
            GameEventTurnSummary::MODEL,
            "session_id",
            GameSession::MODEL,
        ),
        DeleteEdge::new(
            ResourceLedgerTurnSummary::MODEL,
            "session_id",
            GameSession::MODEL,
        ),
    ] {
        assert_relation_strength(edge.child, edge.field_name, RelationStrength::Strong);
        assert_delete_order(policy, edge.child.name(), edge.target.name());
    }

    assert_delete_order(
        policy,
        ParticipantKnownObject::MODEL.name(),
        WorldObject::MODEL.name(),
    );
    assert_delete_order(
        policy,
        ParticipantObjectVisit::MODEL.name(),
        WorldObject::MODEL.name(),
    );
    assert_delete_order(
        policy,
        ChampionObjectVisit::MODEL.name(),
        WorldObject::MODEL.name(),
    );
}

#[test]
fn schema_drift_policy_fails_closed_for_unsupported_changes() {
    for drift in [
        SchemaDrift::StableMemoryIdChanged,
        SchemaDrift::FieldRenamed,
        SchemaDrift::PrimitiveTypeChanged,
        SchemaDrift::RelationStrengthChanged,
        SchemaDrift::IndexRemoved,
        SchemaDrift::RequiredFieldAddedWithoutDatabaseDefault,
        SchemaDrift::ManyFieldDatabaseDefaultAdded,
    ] {
        assert_eq!(drift.reconciliation(), DriftReconciliation::Reject);
    }

    for drift in [
        SchemaDrift::NullableFieldAppended,
        SchemaDrift::RequiredLiteralDefaultFieldAppended,
        SchemaDrift::CompositeIndexAppended,
    ] {
        assert_eq!(drift.reconciliation(), DriftReconciliation::Accept);
    }
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
        ObjectiveProgress::MODEL,
        QuestState::MODEL,
        WorldEventState::MODEL,
        ScenarioRuleState::MODEL,
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
        MovementSnapshot::MODEL,
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

fn assert_field_prefix(model: &EntityModel, expected_prefix: &[&str]) {
    let actual = model
        .fields()
        .iter()
        .map(|field| field.name())
        .collect::<Vec<_>>();
    assert!(
        actual.starts_with(expected_prefix),
        "{} field order must remain append-only; expected prefix {expected_prefix:?}, got {actual:?}",
        model.name()
    );
}

fn assert_database_default(model: &EntityModel, field_name: &str) {
    let field = field(model, field_name);
    assert!(
        matches!(
            field.database_default(),
            FieldDatabaseDefault::EncodedSlotPayload(bytes) if !bytes.is_empty()
        ),
        "{}.{field_name} should expose a persisted database default",
        model.name()
    );
}

fn assert_no_database_default(model: &EntityModel, field_name: &str) {
    assert_eq!(
        field(model, field_name).database_default(),
        FieldDatabaseDefault::None,
        "{}.{field_name} should keep construction defaults out of persisted defaults",
        model.name()
    );
}

fn assert_generated_insert(model: &EntityModel, field_name: &str, expected: FieldInsertGeneration) {
    assert_eq!(
        field(model, field_name).insert_generation(),
        Some(expected),
        "{}.{field_name} should expose the expected insert generation contract",
        model.name()
    );
}

fn assert_index_ordinal(model: &EntityModel, fields: &[&str], expected_ordinal: u16) {
    let Some(index) = model
        .indexes()
        .iter()
        .find(|index| index.fields() == fields)
    else {
        panic!("{} missing index over {fields:?}", model.name());
    };
    assert_eq!(
        index.ordinal(),
        expected_ordinal,
        "{} index over {fields:?} should keep its stable ordinal",
        model.name()
    );
}

fn field<'a>(model: &'a EntityModel, field_name: &str) -> &'a icydb::model::field::FieldModel {
    model
        .fields()
        .iter()
        .find(|field| field.name() == field_name)
        .unwrap_or_else(|| panic!("{} missing field {field_name}", model.name()))
}

#[derive(Clone, Copy)]
struct DeleteEdge {
    child: &'static EntityModel,
    field_name: &'static str,
    target: &'static EntityModel,
}

impl DeleteEdge {
    const fn new(
        child: &'static EntityModel,
        field_name: &'static str,
        target: &'static EntityModel,
    ) -> Self {
        Self {
            child,
            field_name,
            target,
        }
    }
}

fn deletion_policy_order() -> &'static [&'static str] {
    &[
        "ArtifactEquipment",
        "BattleOccupancy",
        "BattleObstacle",
        "BattleStack",
        "TownBuilding",
        "TownRecruitPool",
        "TownGarrisonStack",
        "ChampionArmyStack",
        "ChampionSpell",
        "NeutralArmyStack",
        "ParticipantKnownObject",
        "ParticipantObjectVisit",
        "ChampionObjectVisit",
        "PendingEffect",
        "MovementSnapshot",
        "MovementIntent",
        "CommandEffect",
        "GameEvent",
        "GameEventTurnSummary",
        "ResourceLedgerEntry",
        "ResourceLedgerTurnSummary",
        "AiActorState",
        "MapOccupancy",
        "VisibilityChunk",
        "MapChunk",
        "ObjectiveProgress",
        "QuestState",
        "WorldEventState",
        "ScenarioRuleState",
        "WorldObject",
        "ArtifactInstance",
        "Battle",
        "Town",
        "Champion",
        "NeutralArmy",
        "GameCommand",
        "GameParticipant",
        "GameSession",
    ]
}

fn assert_delete_order(policy: &[&str], child: &str, target: &str) {
    let child_index = policy
        .iter()
        .position(|entity| *entity == child)
        .unwrap_or_else(|| panic!("deletion policy missing child entity {child}"));
    let target_index = policy
        .iter()
        .position(|entity| *entity == target)
        .unwrap_or_else(|| panic!("deletion policy missing target entity {target}"));
    assert!(
        child_index < target_index,
        "deletion policy must delete {child} before {target}: {policy:?}"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SchemaDrift {
    StableMemoryIdChanged,
    FieldRenamed,
    PrimitiveTypeChanged,
    RelationStrengthChanged,
    IndexRemoved,
    RequiredFieldAddedWithoutDatabaseDefault,
    ManyFieldDatabaseDefaultAdded,
    NullableFieldAppended,
    RequiredLiteralDefaultFieldAppended,
    CompositeIndexAppended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DriftReconciliation {
    Accept,
    Reject,
}

impl SchemaDrift {
    const fn reconciliation(self) -> DriftReconciliation {
        match self {
            Self::NullableFieldAppended
            | Self::RequiredLiteralDefaultFieldAppended
            | Self::CompositeIndexAppended => DriftReconciliation::Accept,
            Self::StableMemoryIdChanged
            | Self::FieldRenamed
            | Self::PrimitiveTypeChanged
            | Self::RelationStrengthChanged
            | Self::IndexRemoved
            | Self::RequiredFieldAddedWithoutDatabaseDefault
            | Self::ManyFieldDatabaseDefaultAdded => DriftReconciliation::Reject,
        }
    }
}
