use canic_cdk::query;

use crate::dto::public::{ApiError, ContentManifestResponse};

#[query]
fn get_content_manifest(
    _ruleset_id: String,
    _version: u32,
) -> Result<ContentManifestResponse, ApiError> {
    crate::services::content::unavailable("get_content_manifest")
}
