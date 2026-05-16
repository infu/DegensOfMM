use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CLEANUP_MAX_ROWS_PER_UPDATE: u32 = 100;
pub const CLEANUP_MAX_FINISHED_SESSIONS_PER_UPDATE: u32 = 1;
pub const RAW_FINISHED_LOG_RETENTION_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
pub const RAW_FINISHED_SESSION_LIMIT: u32 = 100;
pub const ACTIVE_SESSION_LIMIT: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CleanupBudget {
    pub max_rows: u32,
    pub max_finished_sessions: u32,
}

impl Default for CleanupBudget {
    fn default() -> Self {
        Self {
            max_rows: CLEANUP_MAX_ROWS_PER_UPDATE,
            max_finished_sessions: CLEANUP_MAX_FINISHED_SESSIONS_PER_UPDATE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CleanupPolicy {
    pub now_ms: u64,
    pub raw_log_retention_ms: u64,
    pub max_finished_raw_sessions: u32,
    pub active_session_limit: u32,
}

impl CleanupPolicy {
    #[must_use]
    pub const fn at(now_ms: u64) -> Self {
        Self {
            now_ms,
            raw_log_retention_ms: RAW_FINISHED_LOG_RETENTION_MS,
            max_finished_raw_sessions: RAW_FINISHED_SESSION_LIMIT,
            active_session_limit: ACTIVE_SESSION_LIMIT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CleanupTarget {
    pub finished_at_ms: u64,
    pub finished_raw_session_rank: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CleanupReport {
    pub session_id: String,
    pub cleaned_sessions: u32,
    pub completed: bool,
    pub budget_exhausted: bool,
    pub raw_logs_retained: bool,
    pub rows_compacted: u32,
    pub event_summaries_written: u32,
    pub ledger_summaries_written: u32,
    pub battle_rows_removed: u32,
    pub map_occupancy_rows_removed: u32,
    pub visibility_rows_removed: u32,
    pub raw_event_rows_removed: u32,
    pub raw_ledger_rows_removed: u32,
    pub retained_player_match_summaries: u32,
    pub retained_match_history_entries: u32,
    pub retained_event_summaries: u32,
    pub retained_ledger_summaries: u32,
    pub operations: Vec<String>,
}

impl CleanupReport {
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            cleaned_sessions: 0,
            completed: false,
            budget_exhausted: false,
            raw_logs_retained: false,
            rows_compacted: 0,
            event_summaries_written: 0,
            ledger_summaries_written: 0,
            battle_rows_removed: 0,
            map_occupancy_rows_removed: 0,
            visibility_rows_removed: 0,
            raw_event_rows_removed: 0,
            raw_ledger_rows_removed: 0,
            retained_player_match_summaries: 0,
            retained_match_history_entries: 0,
            retained_event_summaries: 0,
            retained_ledger_summaries: 0,
            operations: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CleanupCanisterSnapshot {
    pub active_session_count: u32,
    pub finished_raw_session_count: u32,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CleanupError {
    #[error("session is not finished: {session_id}")]
    SessionNotFinished { session_id: String },
    #[error("active recovery rows exist: {reason}")]
    ActiveRecoveryRows { reason: String },
    #[error("cleanup budget cannot clean any finished session")]
    NoFinishedSessionBudget,
    #[error("active session limit reached: {active_session_count}/{active_session_limit}")]
    ActiveSessionLimitReached {
        active_session_count: u32,
        active_session_limit: u32,
    },
    #[error(transparent)]
    Economy(#[from] crate::economy::EconomyError),
}
