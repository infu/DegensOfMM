//! Diagnostic durable `SystemJob` dispatch.

use domm_degens_schema::schema::SystemJob;
use domm_game::ApiError;
use icydb::types::Timestamp;

#[cfg(not(feature = "benchmark"))]
use crate::services::account_lobby_session;
use crate::{
    repos::system_jobs,
    services::{battle, movement, scenario_progress},
};

const TIMER_LEASE_MS: i64 = 30_000;
const MAX_JOBS_PER_TICK: u32 = 8;

pub(crate) fn schedule_nearest_due_job() -> Result<(), ApiError> {
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn repair_and_schedule_after_install_or_upgrade() -> Result<(), ApiError> {
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn run_due_jobs_now() -> Result<u32, ApiError> {
    run_due_jobs(None)
}

#[allow(dead_code)]
pub(crate) fn run_due_jobs_until_idle(max_ticks: u32) -> Result<u32, ApiError> {
    let mut total = 0_u32;
    for _ in 0..max_ticks {
        let processed = run_due_jobs(None)?;
        total = total.saturating_add(processed);
        if processed == 0 {
            break;
        }
    }
    Ok(total)
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn run_due_job_by_key(job_key: &str) -> Result<u32, ApiError> {
    run_job_by_key(job_key)
}

fn run_due_jobs(first_job_key: Option<String>) -> Result<u32, ApiError> {
    if let Some(job_key) = first_job_key {
        return run_job_by_key(&job_key);
    }

    let mut processed = 0_u32;
    let now = Timestamp::now();
    let page = system_jobs::page_due_system_jobs(now, MAX_JOBS_PER_TICK, None)?;
    for job in page.items {
        if processed >= MAX_JOBS_PER_TICK {
            break;
        }
        processed += run_job_by_key(&job.job_key)?;
    }
    if processed < MAX_JOBS_PER_TICK {
        let remaining = MAX_JOBS_PER_TICK.saturating_sub(processed);
        let page = system_jobs::page_expired_running_system_jobs(now, remaining, None)?;
        for job in page.items {
            if processed >= MAX_JOBS_PER_TICK {
                break;
            }
            processed += run_job_by_key(&job.job_key)?;
        }
    }
    Ok(processed)
}

fn run_job_by_key(job_key: &str) -> Result<u32, ApiError> {
    let now = Timestamp::now();
    let lease_expires_at = Timestamp::from_millis(now.as_millis().saturating_add(TIMER_LEASE_MS));
    let Some(job) = system_jobs::claim_system_job(job_key, now, "system_job", lease_expires_at)?
    else {
        return Ok(0);
    };

    dispatch_claimed_job(job)?;
    Ok(1)
}

fn dispatch_claimed_job(job: SystemJob) -> Result<(), ApiError> {
    #[cfg(feature = "benchmark")]
    {
        let method = format!("system_job:{}", job.job_kind);
        return crate::metrics::benchmark_timer(method, || dispatch_claimed_job_inner(job));
    }

    #[cfg(not(feature = "benchmark"))]
    dispatch_claimed_job_inner(job)
}

fn dispatch_claimed_job_inner(job: SystemJob) -> Result<(), ApiError> {
    match job.job_kind.as_str() {
        #[cfg(not(feature = "benchmark"))]
        "setup_session" => {
            account_lobby_session::process_setup_session_job(job)?;
        }
        #[cfg(feature = "benchmark")]
        "setup_session" => {}
        "turn_deadline" | "turn_resolution" => {
            movement::process_turn_resolution_job(job)?;
        }
        "battle_timeout" => {
            battle::process_battle_timeout_job(job)?;
        }
        "battle_round_advance" => {
            battle::process_battle_round_advance_job(job)?;
        }
        "scenario_objectives" | "world_events" | "advanced_victory" => {
            scenario_progress::process_scenario_maintenance_job(job)?;
        }
        _ => {
            system_jobs::fail_system_job(job, false, "unknown system job kind".to_string())?;
        }
    }
    Ok(())
}
