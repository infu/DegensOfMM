//! Shared typed IcyDB repository helpers.

use domm_degens_schema::schema::DegensCanister;
use domm_game::{ApiError, MAX_LIST_LIMIT};
use icydb::{
    db::{FluentLoadQuery, PersistedRow},
    traits::{EntityCreateInput, EntityValue},
    types::Id,
};

pub(crate) type RepoResult<T> = Result<T, ApiError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepositoryPage<E> {
    pub items: Vec<E>,
    pub next_cursor: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexedQueryPlan {
    pub name: &'static str,
    pub entity: &'static str,
    pub indexed_fields: &'static [&'static str],
    pub bounded_limit: Option<u32>,
}

pub(crate) fn validate_limit(
    name: &str,
    limit: u32,
    max: u32,
    error_code: &'static str,
) -> RepoResult<u32> {
    if limit == 0 {
        return Err(ApiError::new(
            "limit_must_be_positive",
            format!("{name} must be at least 1"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    if limit > max {
        return Err(ApiError::new(
            error_code,
            format!("{name} exceeds the v1 public query limit"),
            false,
        )
        .with_details(format!(r#"{{"limit":{limit},"max":{max}}}"#)));
    }

    Ok(limit)
}

pub(crate) fn validate_list_limit(limit: u32) -> RepoResult<u32> {
    validate_limit("limit", limit, MAX_LIST_LIMIT, "list_limit_exceeded")
}

pub(crate) fn map_storage_error(operation: &'static str, _error: icydb::Error) -> ApiError {
    ApiError::new(
        "icydb_repository_error",
        format!("IcyDB repository operation failed: {operation}"),
        true,
    )
}

pub(crate) fn storage_result<T>(
    operation: &'static str,
    result: Result<T, icydb::Error>,
) -> RepoResult<T> {
    result.map_err(|error| map_storage_error(operation, error))
}

pub(crate) fn storage_operation<T>(
    operation: &'static str,
    body: impl FnOnce() -> Result<T, icydb::Error>,
) -> RepoResult<T> {
    let result = crate::metrics::benchmark_repo_operation(operation, body);
    storage_result(operation, result)
}

pub(crate) fn create<I>(operation: &'static str, input: I) -> RepoResult<I::Entity>
where
    I: EntityCreateInput,
    I::Entity: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().create(input))
}

pub(crate) fn insert<E>(operation: &'static str, entity: E) -> RepoResult<E>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().insert(entity))
}

pub(crate) fn insert_many_atomic<E>(
    operation: &'static str,
    entities: impl IntoIterator<Item = E>,
) -> RepoResult<Vec<E>>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().insert_many_atomic(entities))
}

pub(crate) fn update<E>(operation: &'static str, entity: E) -> RepoResult<E>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().update(entity))
}

pub(crate) fn load_by_id<E>(operation: &'static str, id: Id<E>) -> RepoResult<Option<E>>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().load::<E>().by_id(id).try_entity())
}

pub(crate) fn delete_by_id<E>(operation: &'static str, id: Id<E>) -> RepoResult<u32>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || crate::db().delete::<E>().by_id(id).count())
}

pub(crate) fn execute_page<E>(
    operation: &'static str,
    query: FluentLoadQuery<'_, E>,
    limit: u32,
    cursor: Option<String>,
) -> RepoResult<RepositoryPage<E>>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    let page = storage_operation(operation, || query.limit(limit).page())?;
    let page = match cursor {
        Some(cursor) => page.cursor(cursor),
        None => page,
    };
    let response = storage_operation(operation, || page.execute())?;

    let next_cursor = response.next_cursor().map(str::to_string);
    let items = response.into_items();

    Ok(RepositoryPage {
        items,
        next_cursor,
        limit,
    })
}

#[cfg(test)]
pub(crate) fn explain_text<E>(
    operation: &'static str,
    query: FluentLoadQuery<'_, E>,
) -> RepoResult<String>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    storage_operation(operation, || query.explain()).map(|plan| plan.render_text_canonical())
}
