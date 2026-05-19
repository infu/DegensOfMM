//! Repository boundary for durable canister-owned background work.

use domm_degens_schema::schema::{Battle, GameCommand, GameSession, SystemJob};
use icydb::{
    Create,
    db::query::FieldRef,
    types::{Id, Timestamp},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult, RepositoryPage};

pub(crate) const SYSTEM_JOB_BY_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_job_key",
    entity: "SystemJob",
    indexed_fields: &["job_key"],
    bounded_limit: Some(1),
};

pub(crate) const SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_status_due",
    entity: "SystemJob",
    indexed_fields: &["status", "due_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const SYSTEM_JOBS_BY_STATUS_LEASE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_status_lease",
    entity: "SystemJob",
    indexed_fields: &["status", "lease_expires_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const SYSTEM_JOBS_BY_SESSION_STATUS_DUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_session_status_due",
    entity: "SystemJob",
    indexed_fields: &["session_id", "status", "due_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const SYSTEM_JOBS_BY_BATTLE_STATUS_DUE_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_battle_status_due",
    entity: "SystemJob",
    indexed_fields: &["battle_id", "status", "due_at"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const SYSTEM_JOBS_BY_COMMAND_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "system_jobs.by_command",
    entity: "SystemJob",
    indexed_fields: &["command_id"],
    bounded_limit: Some(domm_game::MAX_LIST_LIMIT),
};

pub(crate) const STATUS_SCHEDULED: &str = "scheduled";
pub(crate) const STATUS_RUNNING: &str = "running";
pub(crate) const STATUS_COMPLETED: &str = "completed";
pub(crate) const STATUS_FAILED: &str = "failed";

#[derive(Clone, Debug)]
pub(crate) struct SystemJobDraft {
    pub job_key: String,
    pub job_kind: String,
    pub session_id: Id<GameSession>,
    pub battle_id: Option<Id<Battle>>,
    pub turn_number: Option<u32>,
    pub due_at: Timestamp,
    pub command_id: Option<Id<GameCommand>>,
    pub cursor_json: Option<String>,
}

pub(crate) fn create_system_job(draft: SystemJobDraft) -> RepoResult<SystemJob> {
    let input: Create<SystemJob> = Create::<SystemJob> {
        job_key: Some(draft.job_key),
        job_kind: Some(draft.job_kind),
        session_id: Some(draft.session_id.key()),
        battle_id: Some(draft.battle_id.map(|id| id.key())),
        turn_number: Some(draft.turn_number),
        due_at: Some(draft.due_at),
        status: Some(STATUS_SCHEDULED.to_string()),
        lease_owner: Some(None),
        lease_expires_at: Some(None),
        attempt_count: Some(0),
        generation: Some(0),
        command_id: Some(draft.command_id.map(|id| id.key())),
        cursor_json: Some(draft.cursor_json),
        last_error: Some(None),
    };

    foundation::create("system_jobs.create_system_job", input)
}

pub(crate) fn upsert_system_job(draft: SystemJobDraft) -> RepoResult<SystemJob> {
    if let Some(mut job) = find_system_job_by_key(&draft.job_key)? {
        if matches!(job.status.as_str(), STATUS_COMPLETED) {
            return Ok(job);
        }
        job.job_kind = draft.job_kind;
        job.session_id = draft.session_id.key();
        job.battle_id = draft.battle_id.map(|id| id.key());
        job.turn_number = draft.turn_number;
        job.due_at = draft.due_at;
        job.command_id = draft.command_id.map(|id| id.key());
        job.cursor_json = draft.cursor_json;
        job.status = STATUS_SCHEDULED.to_string();
        job.lease_owner = None;
        job.lease_expires_at = None;
        job.last_error = None;
        return update_system_job(job);
    }

    create_system_job(draft)
}

pub(crate) fn load_system_job(id: Id<SystemJob>) -> RepoResult<Option<SystemJob>> {
    foundation::load_by_id("system_jobs.load_system_job", id)
}

pub(crate) fn update_system_job(job: SystemJob) -> RepoResult<SystemJob> {
    foundation::update("system_jobs.update_system_job", job)
}

pub(crate) fn find_system_job_by_key(job_key: &str) -> RepoResult<Option<SystemJob>> {
    foundation::storage_operation(SYSTEM_JOB_BY_KEY_LOOKUP.name, || {
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("job_key").eq(job_key))
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn page_due_system_jobs(
    now: Timestamp,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(STATUS_SCHEDULED))
            .filter(FieldRef::new("due_at").lte(now))
            .order_asc("due_at")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_expired_running_system_jobs(
    now: Timestamp,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_STATUS_LEASE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(STATUS_RUNNING))
            .filter(FieldRef::new("lease_expires_at").lte(now))
            .order_asc("lease_expires_at")
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn next_scheduled_system_job() -> RepoResult<Option<SystemJob>> {
    foundation::storage_operation(SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP.name, || {
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(STATUS_SCHEDULED))
            .order_asc("due_at")
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn next_expired_running_system_job(now: Timestamp) -> RepoResult<Option<SystemJob>> {
    foundation::storage_operation(SYSTEM_JOBS_BY_STATUS_LEASE_LOOKUP.name, || {
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(STATUS_RUNNING))
            .filter(FieldRef::new("lease_expires_at").lte(now))
            .order_asc("lease_expires_at")
            .order_asc("id")
            .limit(1)
            .try_entity()
    })
}

pub(crate) fn page_system_jobs(
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        "system_jobs.page_all",
        crate::db().load::<SystemJob>().order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_system_jobs_by_status(
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_system_jobs_by_session(
    session_id: Id<GameSession>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_SESSION_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_system_jobs_by_session_status(
    session_id: Id<GameSession>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_SESSION_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("session_id").eq(session_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_system_jobs_by_battle_status(
    battle_id: Id<Battle>,
    status: &str,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_BATTLE_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("battle_id").eq(battle_id.key()))
            .filter(FieldRef::new("status").eq(status))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn page_system_jobs_by_command(
    command_id: Id<GameCommand>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<SystemJob>> {
    let limit = foundation::validate_list_limit(limit)?;
    foundation::execute_page(
        SYSTEM_JOBS_BY_COMMAND_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("command_id").eq(command_id.key()))
            .order_asc("id"),
        limit,
        cursor,
    )
}

pub(crate) fn claim_system_job(
    job_key: &str,
    now: Timestamp,
    lease_owner: &str,
    lease_expires_at: Timestamp,
) -> RepoResult<Option<SystemJob>> {
    let Some(mut job) = find_system_job_by_key(job_key)? else {
        return Ok(None);
    };
    if job.status == STATUS_SCHEDULED && job.due_at > now {
        return Ok(None);
    }
    if !job_is_claimable(&job, now) {
        return Ok(None);
    }

    job.status = STATUS_RUNNING.to_string();
    job.lease_owner = Some(lease_owner.to_string());
    job.lease_expires_at = Some(lease_expires_at);
    job.attempt_count = job.attempt_count.saturating_add(1);
    job.last_error = None;
    update_system_job(job).map(Some)
}

pub(crate) fn complete_system_job(mut job: SystemJob) -> RepoResult<SystemJob> {
    job.status = STATUS_COMPLETED.to_string();
    job.lease_owner = None;
    job.lease_expires_at = None;
    job.last_error = None;
    update_system_job(job)
}

pub(crate) fn fail_system_job(
    mut job: SystemJob,
    retryable: bool,
    error: String,
) -> RepoResult<SystemJob> {
    job.status = if retryable {
        STATUS_SCHEDULED.to_string()
    } else {
        STATUS_FAILED.to_string()
    };
    job.lease_owner = None;
    job.lease_expires_at = None;
    job.last_error = Some(error);
    update_system_job(job)
}

pub(crate) fn reschedule_system_job(
    mut job: SystemJob,
    due_at: Timestamp,
    cursor_json: Option<String>,
) -> RepoResult<SystemJob> {
    job.status = STATUS_SCHEDULED.to_string();
    job.due_at = due_at;
    job.cursor_json = cursor_json;
    job.lease_owner = None;
    job.lease_expires_at = None;
    update_system_job(job)
}

fn job_is_claimable(job: &SystemJob, now: Timestamp) -> bool {
    match job.status.as_str() {
        STATUS_SCHEDULED => true,
        STATUS_RUNNING => job.lease_expires_at.is_some_and(|expires| expires <= now),
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn due_jobs_plan_text(now: Timestamp, limit: u32) -> RepoResult<String> {
    foundation::explain_text(
        SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP.name,
        crate::db()
            .load::<SystemJob>()
            .filter(FieldRef::new("status").eq(STATUS_SCHEDULED))
            .filter(FieldRef::new("due_at").lte(now))
            .order_asc("due_at")
            .order_asc("id")
            .limit(limit),
    )
}
