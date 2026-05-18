use canic_cdk::query;

use crate::dto::public::{ApiError, ContentManifestResponse};

#[query]
fn get_content_manifest(
    ruleset_id: String,
    version: u32,
) -> Result<ContentManifestResponse, ApiError> {
    crate::metrics::benchmark_query("get_content_manifest", || {
        crate::services::content::get_content_manifest(ruleset_id, version)
    })
}
