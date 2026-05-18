//! Controller-gated diagnostics service boundary.

use domm_degens_schema::schema::{
    AiActorState, ArtifactDefinition, ArtifactEquipment, ArtifactInstance, Battle, BattleObstacle,
    BattleOccupancy, BattleParticipantRoundReady, BattleStack, BuildingDefinition, Champion,
    ChampionArmyStack, ChampionClassDefinition, ChampionHire, ChampionObjectVisit, ChampionSpell,
    CommandEffect, DwellingPool, DwellingRecruitment, FactionDefinition, GameCommand, GameEvent,
    GameEventTurnSummary, GameParticipant, GameSession, LobbyCommand, MapChunk,
    MapObjectDefinition, MapOccupancy, MovementIntent, MovementSnapshot, NavalRouteState,
    NeutralArmy, NeutralArmyStack, ObjectiveProgress, ParticipantKnownObject,
    ParticipantObjectVisit, ParticipantTurnReady, PendingEffect, PlayerAccount, PlayerMatchSummary,
    ProceduralMapState, QuestState, ResourceLedgerEntry, ResourceLedgerTurnSummary,
    RulesetDefinition, ScenarioRuleState, SiegeRuleState, SkirmishSettingsState, SpellDefinition,
    SystemJob, TavernOffer, TerrainDefinition, Town, TownBuilding, TownGarrisonStack,
    TownRecruitPool, UnitDefinition, VisibilityChunk, WorldEventState, WorldObject,
};
use domm_game::ApiError;
use icydb::{
    db::PersistedRow,
    traits::EntityValue,
    types::{Id, Timestamp, Ulid},
};

use crate::{
    contract::{
        DiagnosticRowCount, DiagnosticStorageSnapshot, DiagnosticSystemJobPage,
        DiagnosticSystemJobView,
    },
    repos::{foundation, system_jobs},
    services::system_jobs as system_job_service,
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

pub(crate) fn get_diagnostic_system_jobs(
    session_id: Option<String>,
    status: Option<String>,
    limit: u32,
    cursor: Option<String>,
) -> Result<DiagnosticSystemJobPage, ApiError> {
    crate::auth::require_controller("get_diagnostic_system_jobs")?;
    let limit = foundation::validate_list_limit(limit)?;
    let page = match (session_id, status) {
        (Some(session_id), Some(status)) => system_jobs::page_system_jobs_by_session_status(
            parse_session_id(&session_id)?,
            &status,
            limit,
            cursor,
        )?,
        (Some(session_id), None) => {
            system_jobs::page_system_jobs_by_session(parse_session_id(&session_id)?, limit, cursor)?
        }
        (None, Some(status)) => system_jobs::page_system_jobs_by_status(&status, limit, cursor)?,
        (None, None) => system_jobs::page_system_jobs(limit, cursor)?,
    };

    Ok(DiagnosticSystemJobPage {
        jobs: page.items.iter().map(system_job_view).collect(),
        next_cursor: page.next_cursor,
        limit: page.limit,
    })
}

pub(crate) fn force_diagnostic_system_job_running(
    job_key: String,
    lease_expires_at_ms: u64,
) -> Result<DiagnosticSystemJobView, ApiError> {
    crate::auth::require_controller("force_diagnostic_system_job_running")?;
    let Some(mut job) = system_jobs::find_system_job_by_key(&job_key)? else {
        return Err(ApiError::new(
            "system_job_not_found",
            format!("system job not found: {job_key}"),
            false,
        ));
    };

    job.status = system_jobs::STATUS_RUNNING.to_string();
    job.lease_owner = Some("diagnostic".to_string());
    job.lease_expires_at = Some(Timestamp::from_millis(u64_to_i64_saturating(
        lease_expires_at_ms,
    )));
    job.attempt_count = job.attempt_count.saturating_add(1);
    job.last_error = None;

    let updated = system_jobs::update_system_job(job)?;
    system_job_service::schedule_nearest_due_job()?;
    Ok(system_job_view(&updated))
}

pub(crate) fn run_diagnostic_system_jobs(max_ticks: u32) -> Result<u32, ApiError> {
    crate::auth::require_controller("run_diagnostic_system_jobs")?;
    if max_ticks == 0 || max_ticks > 32 {
        return Err(ApiError::new(
            "diagnostic_tick_limit_invalid",
            "diagnostic system job ticks must be between 1 and 32",
            false,
        ));
    }
    system_job_service::run_due_jobs_until_idle(max_ticks)
}

pub(crate) fn run_diagnostic_system_job(job_key: String) -> Result<u32, ApiError> {
    crate::auth::require_controller("run_diagnostic_system_job")?;
    system_job_service::run_due_job_by_key(&job_key)
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
        "BattleParticipantRoundReady" => {
            push_count::<BattleParticipantRoundReady>(row_counts, entity)
        }
        "BattleStack" => push_count::<BattleStack>(row_counts, entity),
        "BuildingDefinition" => push_count::<BuildingDefinition>(row_counts, entity),
        "Champion" => push_count::<Champion>(row_counts, entity),
        "ChampionArmyStack" => push_count::<ChampionArmyStack>(row_counts, entity),
        "ChampionClassDefinition" => push_count::<ChampionClassDefinition>(row_counts, entity),
        "ChampionHire" => push_count::<ChampionHire>(row_counts, entity),
        "ChampionObjectVisit" => push_count::<ChampionObjectVisit>(row_counts, entity),
        "ChampionSpell" => push_count::<ChampionSpell>(row_counts, entity),
        "CommandEffect" => push_count::<CommandEffect>(row_counts, entity),
        "DwellingPool" => push_count::<DwellingPool>(row_counts, entity),
        "DwellingRecruitment" => push_count::<DwellingRecruitment>(row_counts, entity),
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
        "ParticipantTurnReady" => push_count::<ParticipantTurnReady>(row_counts, entity),
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
        "SystemJob" => push_count::<SystemJob>(row_counts, entity),
        "TavernOffer" => push_count::<TavernOffer>(row_counts, entity),
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

fn system_job_view(job: &SystemJob) -> DiagnosticSystemJobView {
    DiagnosticSystemJobView {
        job_key: job.job_key.clone(),
        job_kind: job.job_kind.clone(),
        session_id: job.session_id.to_string(),
        battle_id: job.battle_id.map(|id| id.to_string()),
        turn_number: job.turn_number,
        due_at_ms: timestamp_ms(job.due_at),
        status: job.status.clone(),
        lease_owner: job.lease_owner.clone(),
        lease_expires_at_ms: job.lease_expires_at.map(timestamp_ms),
        attempt_count: job.attempt_count,
        command_id: job.command_id.map(|id| id.to_string()),
        cursor_json: job.cursor_json.clone(),
        last_error: job.last_error.clone(),
    }
}

fn timestamp_ms(timestamp: Timestamp) -> u64 {
    u64::try_from(timestamp.as_millis()).unwrap_or(0)
}

fn u64_to_i64_saturating(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn parse_session_id(value: &str) -> Result<Id<GameSession>, ApiError> {
    Ulid::from_str(value)
        .map(Id::from_key)
        .map_err(|_| ApiError::new("invalid_id", "session_id is not a valid Ulid", false))
}
