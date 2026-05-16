//! Canister clock helpers for public endpoint boundaries.

use icydb::types::Timestamp;

pub(crate) fn now_ms() -> u64 {
    u64::try_from(Timestamp::now().as_millis()).unwrap_or(0)
}
