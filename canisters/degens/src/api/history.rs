use canic_cdk::query;

use crate::dto::public::{ApiError, MatchHistoryPage};

#[query]
fn get_match_history(_cursor: u32, _limit: u32) -> Result<MatchHistoryPage, ApiError> {
    crate::services::history::unavailable("get_match_history")
}
