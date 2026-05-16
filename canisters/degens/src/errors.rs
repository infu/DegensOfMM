use domm_game::ApiError;

pub(crate) fn repository_not_implemented(endpoint: &str) -> ApiError {
    ApiError::new(
        "icydb_repository_not_implemented",
        format!(
            "{endpoint} is declared in the canister contract; IcyDB repository wiring starts in checkpoint 19C"
        ),
        true,
    )
}
