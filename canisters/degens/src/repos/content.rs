//! Repository boundary for rulesets and content definition rows.

use domm_degens_schema::schema::{FactionDefinition, RulesetDefinition};
use icydb::{Create, db::query::FieldRef, types::Id};

use super::foundation::{self, IndexedQueryPlan, RepoResult};

pub(crate) const RULESET_SLUG_VERSION_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.ruleset_by_slug_version",
    entity: "RulesetDefinition",
    indexed_fields: &["slug", "version"],
    bounded_limit: Some(1),
};

pub(crate) const FACTION_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.faction_by_ruleset_slug",
    entity: "FactionDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) fn create_ruleset_definition(
    slug: String,
    version: u32,
    name: String,
    description: Option<String>,
    content_manifest_hash: Option<String>,
) -> RepoResult<RulesetDefinition> {
    let input: Create<RulesetDefinition> = Create::<RulesetDefinition> {
        slug: Some(slug),
        version: Some(version),
        name: Some(name),
        description: Some(description),
        content_manifest_hash: Some(content_manifest_hash),
    };

    foundation::create("content.create_ruleset_definition", input)
}

pub(crate) fn find_ruleset_by_slug_version(
    slug: &str,
    version: u32,
) -> RepoResult<Option<RulesetDefinition>> {
    foundation::storage_result(
        RULESET_SLUG_VERSION_LOOKUP.name,
        crate::db()
            .load::<RulesetDefinition>()
            .filter(FieldRef::new("slug").eq(slug))
            .filter(FieldRef::new("version").eq(version))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn create_faction_definition(
    ruleset_id: Id<RulesetDefinition>,
    slug: String,
    name: String,
    trait_key: String,
) -> RepoResult<FactionDefinition> {
    let input: Create<FactionDefinition> = Create::<FactionDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        slug: Some(slug),
        name: Some(name),
        theme: Some(None),
        description: Some(None),
        icon_key: Some(None),
        banner_key: Some(None),
        native_terrain: Some(None),
        trait_key: Some(trait_key),
    };

    foundation::create("content.create_faction_definition", input)
}

pub(crate) fn find_faction_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<FactionDefinition>> {
    foundation::storage_result(
        FACTION_SLUG_LOOKUP.name,
        crate::db()
            .load::<FactionDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

#[cfg(test)]
pub(crate) fn ruleset_lookup_plan_text(slug: &str, version: u32) -> RepoResult<String> {
    foundation::explain_text(
        RULESET_SLUG_VERSION_LOOKUP.name,
        crate::db()
            .load::<RulesetDefinition>()
            .filter(FieldRef::new("slug").eq(slug))
            .filter(FieldRef::new("version").eq(version))
            .order_asc("id")
            .limit(1),
    )
}
