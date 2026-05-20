//! Repository boundary for player accounts and caller-principal lookups.

use domm_degens_schema::schema::PlayerAccount;
use icydb::{
    db::query::FieldRef,
    types::{Id, Principal},
};

use super::foundation::{self, IndexedQueryPlan, RepoResult};

pub(crate) const PRINCIPAL_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "players.by_principal",
    entity: "PlayerAccount",
    indexed_fields: &["account_principal"],
    bounded_limit: Some(1),
};

pub(crate) fn create_player_account(
    account_principal: Principal,
    username: Option<String>,
    display_name: Option<String>,
) -> RepoResult<PlayerAccount> {
    foundation::storage_result(
        "players.create_player_account",
        try_create_player_account(account_principal, username, display_name),
    )
}

pub(crate) fn try_create_player_account(
    account_principal: Principal,
    username: Option<String>,
    display_name: Option<String>,
) -> Result<PlayerAccount, icydb::Error> {
    let player = PlayerAccount {
        account_principal,
        username,
        display_name,
        ..Default::default()
    };

    crate::metrics::benchmark_repo_operation("players.create_player_account", || {
        crate::db().insert(player)
    })
}

pub(crate) fn load_player_account(id: Id<PlayerAccount>) -> RepoResult<Option<PlayerAccount>> {
    foundation::load_by_id("players.load_player_account", id)
}

pub(crate) fn find_by_principal(account_principal: Principal) -> RepoResult<Option<PlayerAccount>> {
    foundation::storage_result(
        PRINCIPAL_LOOKUP.name,
        crate::db()
            .load::<PlayerAccount>()
            .filter(FieldRef::new("account_principal").eq(account_principal))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn find_by_username(username: &str) -> RepoResult<Option<PlayerAccount>> {
    foundation::storage_result(
        "players.by_username",
        crate::db()
            .load::<PlayerAccount>()
            .filter(FieldRef::new("username").eq(username))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn update_player_account(player: PlayerAccount) -> RepoResult<PlayerAccount> {
    foundation::update("players.update_player_account", player)
}

#[cfg(test)]
#[cfg(test)]
pub(crate) fn principal_lookup_plan_text(account_principal: Principal) -> RepoResult<String> {
    foundation::explain_text(
        PRINCIPAL_LOOKUP.name,
        crate::db()
            .load::<PlayerAccount>()
            .filter(FieldRef::new("account_principal").eq(account_principal))
            .order_asc("id")
            .limit(1),
    )
}
