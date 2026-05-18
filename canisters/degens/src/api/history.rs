use canic_cdk::query;

use crate::dto::public::{ApiError, MatchHistoryPage};

#[query]
fn get_match_history(cursor: u32, limit: u32) -> Result<MatchHistoryPage, ApiError> {
    crate::metrics::benchmark_query("get_match_history", || {
        crate::services::history::get_match_history(canic_cdk::api::msg_caller(), cursor, limit)
    })
}
