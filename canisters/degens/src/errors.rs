use domm_game::ApiError;

pub(crate) fn repository_not_implemented(endpoint: &str) -> ApiError {
    ApiError::new(
        "icydb_repository_not_implemented",
        format!(
            "{endpoint} is declared in the canister contract; its IcyDB-backed service body is scheduled for a later checkpoint"
        ),
        true,
    )
}
