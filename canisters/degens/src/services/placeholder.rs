use domm_game::ApiError;

pub(crate) fn repository_not_implemented<T>(endpoint: &str) -> Result<T, ApiError> {
    Err(crate::errors::repository_not_implemented(endpoint))
}
