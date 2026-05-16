use domm_game::{
    ApiError, ContentManifestResponse, FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG,
    FIRST_PLAYABLE_RULESET_VERSION, first_playable_content_manifest,
};
use icydb::traits::EntityValue;

use crate::repos::content;

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

    let ruleset = content::find_ruleset_by_slug_version(FIRST_PLAYABLE_RULESET_SLUG, version)?
        .ok_or_else(|| {
            ApiError::new(
                "content_manifest_not_seeded",
                "first playable content rows have not been seeded",
                false,
            )
        })?;
    verify_seeded_definition_rows(ruleset.id())?;

    Ok(ContentManifestResponse {
        manifest: first_playable_content_manifest(),
    })
}

fn verify_seeded_definition_rows(
    ruleset_id: icydb::types::Id<domm_degens_schema::schema::RulesetDefinition>,
) -> Result<(), ApiError> {
    let manifest = first_playable_content_manifest();
    let factions = content::page_factions_by_ruleset(ruleset_id)?;
    let classes = content::page_champion_classes_by_ruleset(ruleset_id)?;
    let terrain = content::page_terrain_by_ruleset(ruleset_id)?;
    let units = content::page_units_by_ruleset(ruleset_id)?;
    let buildings = content::page_buildings_by_ruleset(ruleset_id)?;
    let spells = content::page_spells_by_ruleset(ruleset_id)?;
    let artifacts = content::page_artifacts_by_ruleset(ruleset_id)?;
    let map_objects = content::page_map_objects_by_ruleset(ruleset_id)?;

    require_at_least("factions", factions.len(), manifest.factions.len())?;
    require_at_least(
        "champion_classes",
        classes.len(),
        manifest.champion_classes.len(),
    )?;
    require_at_least("terrain", terrain.len(), unique_terrain_count())?;
    require_at_least("units", units.len(), manifest.units.len())?;
    require_at_least("buildings", buildings.len(), manifest.buildings.len())?;
    require_at_least("spells", spells.len(), manifest.spells.len())?;
    require_at_least("artifacts", artifacts.len(), manifest.artifacts.len())?;
    require_at_least("map_objects", map_objects.len(), manifest.map_objects.len())?;
    Ok(())
}

fn unique_terrain_count() -> usize {
    let mut keys = first_playable_content_manifest()
        .terrain
        .into_iter()
        .map(|terrain| terrain.terrain_key)
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys.len()
}

fn require_at_least(kind: &str, actual: usize, expected: usize) -> Result<(), ApiError> {
    if actual < expected {
        return Err(ApiError::new(
            "content_manifest_incomplete",
            format!("{kind} content rows are incomplete"),
            true,
        )
        .with_details(format!(
            "{{\"kind\":\"{}\",\"actual\":{},\"expected\":{}}}",
            kind, actual, expected
        )));
    }
    Ok(())
}
