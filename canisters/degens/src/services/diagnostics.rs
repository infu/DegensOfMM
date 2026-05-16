//! Controller-gated diagnostics service boundary.

use domm_degens_schema::schema::{
    AiActorState, ArtifactDefinition, ArtifactEquipment, ArtifactInstance, Battle, BattleObstacle,
    BattleOccupancy, BattleStack, BuildingDefinition, Champion, ChampionArmyStack,
    ChampionClassDefinition, ChampionObjectVisit, ChampionSpell, CommandEffect, FactionDefinition,
    GameCommand, GameEvent, GameEventTurnSummary, GameParticipant, GameSession, LobbyCommand,
    MapChunk, MapObjectDefinition, MapOccupancy, MovementIntent, MovementSnapshot, NavalRouteState,
    NeutralArmy, NeutralArmyStack, ObjectiveProgress, ParticipantKnownObject,
    ParticipantObjectVisit, PendingEffect, PlayerAccount, PlayerMatchSummary, ProceduralMapState,
    QuestState, ResourceLedgerEntry, ResourceLedgerTurnSummary, RulesetDefinition,
    ScenarioRuleState, SiegeRuleState, SkirmishSettingsState, SpellDefinition, TerrainDefinition,
    Town, TownBuilding, TownGarrisonStack, TownRecruitPool, UnitDefinition, VisibilityChunk,
    WorldEventState, WorldObject,
};
use domm_game::ApiError;
use icydb::{db::PersistedRow, traits::EntityValue};

use crate::{
    contract::{DiagnosticRowCount, DiagnosticStorageSnapshot},
    repos::foundation,
};

const MAX_DIAGNOSTIC_ENTITY_COUNTS: usize = 16;
const MAX_DIAGNOSTIC_ROWS_PER_ENTITY: u32 = 512;

pub(crate) fn get_diagnostic_storage_snapshot(
    entity_names: Vec<String>,
) -> Result<DiagnosticStorageSnapshot, ApiError> {
    crate::auth::require_controller("get_diagnostic_storage_snapshot")?;

    if entity_names.is_empty() {
        return Err(ApiError::new(
            "diagnostic_entity_required",
            "diagnostic storage snapshots require at least one entity name",
            false,
        ));
    }
    if entity_names.len() > MAX_DIAGNOSTIC_ENTITY_COUNTS {
        return Err(ApiError::new(
            "diagnostic_entity_limit_exceeded",
            "diagnostic storage snapshots are limited to 16 entity counts",
            false,
        ));
    }

    let mut row_counts = Vec::new();
    for entity_name in entity_names {
        push_named_count(&mut row_counts, &entity_name)?;
    }

    let total_rows = row_counts.iter().map(|row| row.count).sum();
    Ok(DiagnosticStorageSnapshot {
        row_counts,
        total_rows,
        stable_memory_pages: canic_cdk::api::stable_size(),
    })
}

fn push_named_count(
    row_counts: &mut Vec<DiagnosticRowCount>,
    entity: &str,
) -> Result<(), ApiError> {
    match entity {
        "AiActorState" => push_count::<AiActorState>(row_counts, entity),
        "ArtifactDefinition" => push_count::<ArtifactDefinition>(row_counts, entity),
        "ArtifactEquipment" => push_count::<ArtifactEquipment>(row_counts, entity),
        "ArtifactInstance" => push_count::<ArtifactInstance>(row_counts, entity),
        "Battle" => push_count::<Battle>(row_counts, entity),
        "BattleObstacle" => push_count::<BattleObstacle>(row_counts, entity),
        "BattleOccupancy" => push_count::<BattleOccupancy>(row_counts, entity),
        "BattleStack" => push_count::<BattleStack>(row_counts, entity),
        "BuildingDefinition" => push_count::<BuildingDefinition>(row_counts, entity),
        "Champion" => push_count::<Champion>(row_counts, entity),
        "ChampionArmyStack" => push_count::<ChampionArmyStack>(row_counts, entity),
        "ChampionClassDefinition" => push_count::<ChampionClassDefinition>(row_counts, entity),
        "ChampionObjectVisit" => push_count::<ChampionObjectVisit>(row_counts, entity),
        "ChampionSpell" => push_count::<ChampionSpell>(row_counts, entity),
        "CommandEffect" => push_count::<CommandEffect>(row_counts, entity),
        "FactionDefinition" => push_count::<FactionDefinition>(row_counts, entity),
        "GameCommand" => push_count::<GameCommand>(row_counts, entity),
        "GameEvent" => push_count::<GameEvent>(row_counts, entity),
        "GameEventTurnSummary" => push_count::<GameEventTurnSummary>(row_counts, entity),
        "GameParticipant" => push_count::<GameParticipant>(row_counts, entity),
        "GameSession" => push_count::<GameSession>(row_counts, entity),
        "LobbyCommand" => push_count::<LobbyCommand>(row_counts, entity),
        "MapChunk" => push_count::<MapChunk>(row_counts, entity),
        "MapObjectDefinition" => push_count::<MapObjectDefinition>(row_counts, entity),
        "MapOccupancy" => push_count::<MapOccupancy>(row_counts, entity),
        "MovementIntent" => push_count::<MovementIntent>(row_counts, entity),
        "MovementSnapshot" => push_count::<MovementSnapshot>(row_counts, entity),
        "NavalRouteState" => push_count::<NavalRouteState>(row_counts, entity),
        "NeutralArmy" => push_count::<NeutralArmy>(row_counts, entity),
        "NeutralArmyStack" => push_count::<NeutralArmyStack>(row_counts, entity),
        "ObjectiveProgress" => push_count::<ObjectiveProgress>(row_counts, entity),
        "ParticipantKnownObject" => push_count::<ParticipantKnownObject>(row_counts, entity),
        "ParticipantObjectVisit" => push_count::<ParticipantObjectVisit>(row_counts, entity),
        "PendingEffect" => push_count::<PendingEffect>(row_counts, entity),
        "PlayerAccount" => push_count::<PlayerAccount>(row_counts, entity),
        "PlayerMatchSummary" => push_count::<PlayerMatchSummary>(row_counts, entity),
        "ProceduralMapState" => push_count::<ProceduralMapState>(row_counts, entity),
        "QuestState" => push_count::<QuestState>(row_counts, entity),
        "ResourceLedgerEntry" => push_count::<ResourceLedgerEntry>(row_counts, entity),
        "ResourceLedgerTurnSummary" => push_count::<ResourceLedgerTurnSummary>(row_counts, entity),
        "RulesetDefinition" => push_count::<RulesetDefinition>(row_counts, entity),
        "ScenarioRuleState" => push_count::<ScenarioRuleState>(row_counts, entity),
        "SiegeRuleState" => push_count::<SiegeRuleState>(row_counts, entity),
        "SkirmishSettingsState" => push_count::<SkirmishSettingsState>(row_counts, entity),
        "SpellDefinition" => push_count::<SpellDefinition>(row_counts, entity),
        "TerrainDefinition" => push_count::<TerrainDefinition>(row_counts, entity),
        "Town" => push_count::<Town>(row_counts, entity),
        "TownBuilding" => push_count::<TownBuilding>(row_counts, entity),
        "TownGarrisonStack" => push_count::<TownGarrisonStack>(row_counts, entity),
        "TownRecruitPool" => push_count::<TownRecruitPool>(row_counts, entity),
        "UnitDefinition" => push_count::<UnitDefinition>(row_counts, entity),
        "VisibilityChunk" => push_count::<VisibilityChunk>(row_counts, entity),
        "WorldEventState" => push_count::<WorldEventState>(row_counts, entity),
        "WorldObject" => push_count::<WorldObject>(row_counts, entity),
        _ => Err(ApiError::new(
            "unknown_diagnostic_entity",
            format!("unknown diagnostic entity name: {entity}"),
            false,
        )),
    }
}

fn push_count<E>(row_counts: &mut Vec<DiagnosticRowCount>, entity: &str) -> Result<(), ApiError>
where
    E: PersistedRow<Canister = domm_degens_schema::schema::DegensCanister> + EntityValue,
{
    let rows = foundation::storage_result(
        "diagnostics.count_entity",
        crate::db()
            .load::<E>()
            .order_asc("id")
            .limit(MAX_DIAGNOSTIC_ROWS_PER_ENTITY)
            .entities(),
    )?;
    row_counts.push(DiagnosticRowCount {
        entity: entity.to_string(),
        count: rows.len() as u32,
    });
    Ok(())
}
