//! Repository boundary for bounded retained-state cleanup and dependency-order deletes.

use domm_degens_schema::schema::DegensCanister;
use icydb::{db::PersistedRow, traits::EntityValue, types::Id};

use super::foundation::{self, RepoResult};

pub(crate) fn delete_row_by_id<E>(operation: &'static str, id: Id<E>) -> RepoResult<u32>
where
    E: PersistedRow<Canister = DegensCanister> + EntityValue,
{
    foundation::delete_by_id(operation, id)
}
