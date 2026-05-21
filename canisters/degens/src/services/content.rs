use domm_game::{
    ApiError, ContentManifestResponse, FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG,
    FIRST_PLAYABLE_RULESET_VERSION, first_playable_content_manifest,
};

pub(crate) fn get_content_manifest(
    ruleset_id: String,
    version: u32,
) -> Result<ContentManifestResponse, ApiError> {
    if version != FIRST_PLAYABLE_RULESET_VERSION {
        return Err(ApiError::new(
            "content_manifest_not_found",
            "content manifest was not found",
            false,
        ));
    }
    if !matches!(
        ruleset_id.as_str(),
        FIRST_PLAYABLE_RULESET_ID | FIRST_PLAYABLE_RULESET_SLUG | "ruleset:first-playable"
    ) {
        return Err(ApiError::new(
            "content_manifest_not_found",
            "content manifest was not found",
            false,
        ));
    }

    let manifest = first_playable_content_manifest();
    Ok(ContentManifestResponse { manifest })
}
