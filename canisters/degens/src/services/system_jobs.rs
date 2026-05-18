//! Timer wakeups and durable `SystemJob` dispatch.

use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::time::Duration;

use domm_degens_schema::schema::{GameSession, SystemJob};
use domm_game::ApiError;
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp},
};

use crate::{
    repos::{
        battles, sessions,
        system_jobs::{self, SystemJobDraft},
    },
    services::{account_lobby_session, battle, movement, scenario_progress},
};

const TIMER_LEASE_MS: i64 = 30_000;
const MAX_JOBS_PER_TICK: u32 = 8;

thread_local! {
    static NEAREST_TIMER: RefCell<Option<ScheduledWakeup>> = const { RefCell::new(None) };
    #[cfg(target_arch = "wasm32")]
    static DUE_SCAN_REQUESTED: RefCell<bool> = const { RefCell::new(false) };
}

struct ScheduledWakeup {
    timer_id: canic_cdk::timers::TimerId,
    #[cfg(target_arch = "wasm32")]
    job_key: String,
    #[cfg(target_arch = "wasm32")]
    due_at_ms: i64,
}

pub(crate) fn schedule_job(draft: SystemJobDraft) -> Result<SystemJob, ApiError> {
    let job = system_jobs::upsert_system_job(draft)?;
    #[cfg(target_arch = "wasm32")]
    schedule_wakeup_for_upserted_job(&job)?;
    Ok(job)
}

pub(crate) fn schedule_nearest_due_job() -> Result<(), ApiError> {
    let Some(job) = next_claimable_or_scheduled_job()? else {
        clear_nearest_timer();
        return Ok(());
    };

    replace_timer(job.job_key, job.due_at);
    Ok(())
}

pub(crate) fn repair_and_schedule_after_install_or_upgrade() -> Result<(), ApiError> {
    repair_jobs_for_sessions_in_state("starting")?;
    repair_jobs_for_sessions_in_state("active")?;
    schedule_nearest_due_job()
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

pub(crate) fn run_due_job_by_key(job_key: &str) -> Result<u32, ApiError> {
    run_job_by_key(job_key)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn heartbeat_tick() {
    if !nearest_timer_is_due() && !take_due_scan_requested() {
        return;
    }
    if let Err(error) = run_due_jobs_now() {
        canic_cdk::eprintln!("system job heartbeat failed: {}", error.message);
    }
    if let Err(error) = schedule_nearest_due_job() {
        canic_cdk::eprintln!("system job heartbeat reschedule failed: {}", error.message);
    }
}

fn repair_jobs_for_sessions_in_state(state: &str) -> Result<(), ApiError> {
    let mut cursor = None;
    loop {
        let page = sessions::page_sessions_by_state(state, domm_game::MAX_LIST_LIMIT, cursor)?;
        for session in &page.items {
            repair_jobs_for_session(session)?;
        }
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }
    Ok(())
}

fn repair_jobs_for_session(session: &GameSession) -> Result<(), ApiError> {
    match session.state.as_str() {
        "starting" => {
            schedule_job(SystemJobDraft {
                job_key: format!("setup_session:{}", session.id()),
                job_kind: "setup_session".to_string(),
                session_id: session.id(),
                battle_id: None,
                turn_number: Some(session.current_turn),
                due_at: Timestamp::now(),
                command_id: None,
                cursor_json: None,
            })?;
        }
        "active" => {
            schedule_job(SystemJobDraft {
                job_key: format!("turn_deadline:{}:{}", session.id(), session.current_turn),
                job_kind: "turn_deadline".to_string(),
                session_id: session.id(),
                battle_id: None,
                turn_number: Some(session.current_turn),
                due_at: session.turn_deadline_at,
                command_id: None,
                cursor_json: None,
            })?;
            repair_battle_timeout_jobs(session.id())?;
        }
        _ => {}
    }
    Ok(())
}

fn repair_battle_timeout_jobs(session_id: Id<GameSession>) -> Result<(), ApiError> {
    let mut cursor = None;
    loop {
        let page = battles::page_battles_by_session_state(
            session_id,
            "active",
            domm_game::MAX_ACTIVE_BATTLES_PER_SESSION,
            cursor,
        )?;
        for battle in &page.items {
            if let Some(deadline) = battle.action_deadline_at {
                schedule_job(SystemJobDraft {
                    job_key: format!("battle_timeout:{}:{}", battle.id(), deadline.as_millis()),
                    job_kind: "battle_timeout".to_string(),
                    session_id,
                    battle_id: Some(battle.id()),
                    turn_number: Some(battle.created_turn),
                    due_at: deadline,
                    command_id: None,
                    cursor_json: None,
                })?;
            }
        }
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn replace_timer(job_key: String, due_at: Timestamp) {
    clear_nearest_timer();

    let due_at_ms = due_at.as_millis();
    let delay = timer_delay(due_at);
    let callback_key = job_key.clone();
    let timer_id = canic_cdk::timers::set_timer(delay, async move {
        clear_fired_timer(&callback_key, due_at_ms);
        if let Err(error) = run_due_jobs(Some(callback_key)) {
            canic_cdk::eprintln!("system job timer failed: {}", error.message);
        }
        if let Err(error) = schedule_nearest_due_job() {
            canic_cdk::eprintln!("system job reschedule failed: {}", error.message);
        }
    });

    NEAREST_TIMER.with_borrow_mut(|slot| {
        *slot = Some(ScheduledWakeup {
            timer_id,
            job_key,
            due_at_ms,
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn replace_timer(_job_key: String, _due_at: Timestamp) {}

#[cfg(target_arch = "wasm32")]
fn schedule_wakeup_for_upserted_job(job: &SystemJob) -> Result<(), ApiError> {
    let had_timer = NEAREST_TIMER.with_borrow(|slot| slot.is_some());
    if !had_timer {
        schedule_nearest_due_job()?;
    }

    let due_at_ms = job.due_at.as_millis();
    if due_at_ms <= Timestamp::now().as_millis() {
        request_due_scan();
    }
    let should_replace = NEAREST_TIMER.with_borrow(|slot| {
        slot.as_ref()
            .is_none_or(|wakeup| wakeup.job_key == job.job_key || due_at_ms <= wakeup.due_at_ms)
    });
    if should_replace {
        replace_timer(job.job_key.clone(), job.due_at);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn request_due_scan() {
    DUE_SCAN_REQUESTED.with_borrow_mut(|requested| {
        *requested = true;
    });
}

#[cfg(target_arch = "wasm32")]
fn take_due_scan_requested() -> bool {
    DUE_SCAN_REQUESTED.with_borrow_mut(|requested| {
        let was_requested = *requested;
        *requested = false;
        was_requested
    })
}

fn clear_nearest_timer() {
    NEAREST_TIMER.with_borrow_mut(|slot| {
        if let Some(wakeup) = slot.take() {
            canic_cdk::timers::clear_timer(wakeup.timer_id);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn clear_fired_timer(job_key: &str, due_at_ms: i64) {
    NEAREST_TIMER.with_borrow_mut(|slot| {
        let fired = slot
            .as_ref()
            .is_some_and(|wakeup| wakeup.job_key == job_key && wakeup.due_at_ms == due_at_ms);
        if fired {
            *slot = None;
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn nearest_timer_is_due() -> bool {
    let now_ms = Timestamp::now().as_millis();
    NEAREST_TIMER.with_borrow(|slot| {
        slot.as_ref()
            .is_some_and(|wakeup| now_ms >= wakeup.due_at_ms)
    })
}

#[cfg(target_arch = "wasm32")]
fn timer_delay(due_at: Timestamp) -> Duration {
    let now_ms = Timestamp::now().as_millis();
    let delay_ms = due_at.as_millis().saturating_sub(now_ms).max(0);
    Duration::from_millis(u64::try_from(delay_ms).unwrap_or(u64::MAX))
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
    let Some(job) = system_jobs::claim_system_job(job_key, now, "system_timer", lease_expires_at)?
    else {
        return Ok(0);
    };

    dispatch_claimed_job(job)?;
    Ok(1)
}

fn next_claimable_or_scheduled_job() -> Result<Option<SystemJob>, ApiError> {
    let now = Timestamp::now();
    if let Some(job) = system_jobs::next_expired_running_system_job(now)? {
        return Ok(Some(job));
    }
    system_jobs::next_scheduled_system_job()
}

fn dispatch_claimed_job(job: SystemJob) -> Result<(), ApiError> {
    match job.job_kind.as_str() {
        "setup_session" => {
            account_lobby_session::process_setup_session_job(job)?;
        }
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
