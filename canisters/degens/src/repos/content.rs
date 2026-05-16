//! Repository boundary for rulesets and content definition rows.

use domm_degens_schema::schema::{
    ArtifactDefinition, BuildingDefinition, ChampionClassDefinition, FactionDefinition,
    MapObjectDefinition, RulesetDefinition, SpellDefinition, TerrainDefinition, UnitDefinition,
};
use domm_game::{
    ArtifactContent, BuildingContent, ChampionClassContent, MapObjectContent, ResourceCost,
    SpellContent, TerrainContent, UnitContent,
};
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

pub(crate) const CHAMPION_CLASS_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.champion_class_by_ruleset_slug",
    entity: "ChampionClassDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) const TERRAIN_KEY_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.terrain_by_ruleset_key",
    entity: "TerrainDefinition",
    indexed_fields: &["ruleset_id", "terrain_key"],
    bounded_limit: Some(1),
};

pub(crate) const UNIT_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.unit_by_ruleset_slug",
    entity: "UnitDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) const BUILDING_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.building_by_ruleset_slug",
    entity: "BuildingDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) const SPELL_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.spell_by_ruleset_slug",
    entity: "SpellDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) const ARTIFACT_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.artifact_by_ruleset_slug",
    entity: "ArtifactDefinition",
    indexed_fields: &["ruleset_id", "slug"],
    bounded_limit: Some(1),
};

pub(crate) const MAP_OBJECT_SLUG_LOOKUP: IndexedQueryPlan = IndexedQueryPlan {
    name: "content.map_object_by_ruleset_slug",
    entity: "MapObjectDefinition",
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

pub(crate) fn load_ruleset(id: Id<RulesetDefinition>) -> RepoResult<Option<RulesetDefinition>> {
    foundation::load_by_id("content.load_ruleset", id)
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

pub(crate) fn load_faction(id: Id<FactionDefinition>) -> RepoResult<Option<FactionDefinition>> {
    foundation::load_by_id("content.load_faction", id)
}

pub(crate) fn page_factions_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<FactionDefinition>> {
    page_content_rows(
        "content.factions_by_ruleset",
        crate::db()
            .load::<FactionDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_champion_class_definition(
    ruleset_id: Id<RulesetDefinition>,
    faction_id: Option<Id<FactionDefinition>>,
    class: ChampionClassContent,
) -> RepoResult<ChampionClassDefinition> {
    let input: Create<ChampionClassDefinition> = Create::<ChampionClassDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        faction_id: Some(faction_id.map(|id| id.key())),
        slug: Some(class.slug),
        name: Some(class.name),
        description: Some(class.description),
        portrait_key: Some(class.portrait_key),
        base_movement: Some(class.base_movement),
        base_vision: Some(class.base_vision),
    };

    foundation::create("content.create_champion_class_definition", input)
}

pub(crate) fn find_champion_class_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<ChampionClassDefinition>> {
    foundation::storage_result(
        CHAMPION_CLASS_SLUG_LOOKUP.name,
        crate::db()
            .load::<ChampionClassDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_champion_class(
    id: Id<ChampionClassDefinition>,
) -> RepoResult<Option<ChampionClassDefinition>> {
    foundation::load_by_id("content.load_champion_class", id)
}

pub(crate) fn page_champion_classes_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<ChampionClassDefinition>> {
    page_content_rows(
        "content.champion_classes_by_ruleset",
        crate::db()
            .load::<ChampionClassDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_terrain_definition(
    ruleset_id: Id<RulesetDefinition>,
    terrain: TerrainContent,
) -> RepoResult<TerrainDefinition> {
    let input: Create<TerrainDefinition> = Create::<TerrainDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        terrain_key: Some(terrain.terrain_key),
        terrain_code: Some(terrain.terrain_code),
        name: Some(terrain.name),
        movement_cost: Some(terrain.movement_cost),
        passable: Some(terrain.passable),
        sprite_key: Some(terrain.sprite_key),
    };

    foundation::create("content.create_terrain_definition", input)
}

pub(crate) fn find_terrain_by_ruleset_key(
    ruleset_id: Id<RulesetDefinition>,
    terrain_key: &str,
) -> RepoResult<Option<TerrainDefinition>> {
    foundation::storage_result(
        TERRAIN_KEY_LOOKUP.name,
        crate::db()
            .load::<TerrainDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("terrain_key").eq(terrain_key))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_terrain_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<TerrainDefinition>> {
    page_content_rows(
        "content.terrain_by_ruleset",
        crate::db()
            .load::<TerrainDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("terrain_code")
            .order_asc("terrain_key")
            .order_asc("id"),
    )
}

pub(crate) fn create_unit_definition(
    ruleset_id: Id<RulesetDefinition>,
    faction_id: Option<Id<FactionDefinition>>,
    unit: UnitContent,
) -> RepoResult<UnitDefinition> {
    let input: Create<UnitDefinition> = Create::<UnitDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        faction_id: Some(faction_id.map(|id| id.key())),
        slug: Some(unit.slug),
        name: Some(unit.name),
        description: Some(unit.description),
        sprite_key: Some(unit.sprite_key),
        icon_key: Some(unit.icon_key),
        animation_key: Some(unit.animation_key),
        tier: Some(unit.tier),
        attack: Some(unit.attack),
        defense: Some(unit.defense),
        damage_min: Some(unit.damage_min),
        damage_max: Some(unit.damage_max),
        max_hp: Some(unit.max_hp),
        speed: Some(unit.speed),
        initiative: Some(unit.initiative),
        ranged: Some(unit.ranged),
        flying: Some(unit.flying),
        shots: Some(unit.shots),
        gold_cost: Some(unit.cost.gold),
        wood_cost: Some(unit.cost.wood),
        stone_cost: Some(unit.cost.stone),
        iron_cost: Some(unit.cost.iron),
        crystal_cost: Some(unit.cost.crystal),
        ember_cost: Some(unit.cost.ember),
        aether_cost: Some(unit.cost.aether),
        weekly_growth: Some(unit.weekly_growth),
        ability_keys: Some(unit.ability_keys),
    };

    foundation::create("content.create_unit_definition", input)
}

pub(crate) fn find_unit_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<UnitDefinition>> {
    foundation::storage_result(
        UNIT_SLUG_LOOKUP.name,
        crate::db()
            .load::<UnitDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_unit(id: Id<UnitDefinition>) -> RepoResult<Option<UnitDefinition>> {
    foundation::load_by_id("content.load_unit", id)
}

pub(crate) fn page_units_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<UnitDefinition>> {
    page_content_rows(
        "content.units_by_ruleset",
        crate::db()
            .load::<UnitDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_building_definition(
    ruleset_id: Id<RulesetDefinition>,
    faction_id: Option<Id<FactionDefinition>>,
    building: BuildingContent,
) -> RepoResult<BuildingDefinition> {
    let cost = building.cost;
    let input: Create<BuildingDefinition> = Create::<BuildingDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        faction_id: Some(faction_id.map(|id| id.key())),
        slug: Some(building.slug),
        name: Some(building.name),
        description: Some(building.description),
        icon_key: Some(building.icon_key),
        building_type: Some(building.building_type),
        gold_cost: Some(cost.gold),
        wood_cost: Some(cost.wood),
        stone_cost: Some(cost.stone),
        iron_cost: Some(cost.iron),
        crystal_cost: Some(cost.crystal),
        ember_cost: Some(cost.ember),
        aether_cost: Some(cost.aether),
        requires_building_slugs: Some(building.requires_building_slugs),
        unlocks_unit_slug: Some(building.unlocks_unit_slug),
        effect_key: Some(building.effect_key),
    };

    foundation::create("content.create_building_definition", input)
}

pub(crate) fn find_building_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<BuildingDefinition>> {
    foundation::storage_result(
        BUILDING_SLUG_LOOKUP.name,
        crate::db()
            .load::<BuildingDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_building(id: Id<BuildingDefinition>) -> RepoResult<Option<BuildingDefinition>> {
    foundation::load_by_id("content.load_building", id)
}

pub(crate) fn page_buildings_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<BuildingDefinition>> {
    page_content_rows(
        "content.buildings_by_ruleset",
        crate::db()
            .load::<BuildingDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_spell_definition(
    ruleset_id: Id<RulesetDefinition>,
    spell: SpellContent,
) -> RepoResult<SpellDefinition> {
    let input: Create<SpellDefinition> = Create::<SpellDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        slug: Some(spell.slug),
        name: Some(spell.name),
        description: Some(spell.description),
        icon_key: Some(spell.icon_key),
        school: Some(spell.school),
        level: Some(spell.level),
        mana_cost: Some(spell.mana_cost),
        target_type: Some(spell.target_type),
        effect_key: Some(spell.effect_key),
        duration_rounds: Some(spell.duration_rounds),
    };

    foundation::create("content.create_spell_definition", input)
}

pub(crate) fn find_spell_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<SpellDefinition>> {
    foundation::storage_result(
        SPELL_SLUG_LOOKUP.name,
        crate::db()
            .load::<SpellDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn page_spells_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<SpellDefinition>> {
    page_content_rows(
        "content.spells_by_ruleset",
        crate::db()
            .load::<SpellDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_artifact_definition(
    ruleset_id: Id<RulesetDefinition>,
    artifact: ArtifactContent,
) -> RepoResult<ArtifactDefinition> {
    let input: Create<ArtifactDefinition> = Create::<ArtifactDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        slug: Some(artifact.slug),
        name: Some(artifact.name),
        description: Some(artifact.description),
        icon_key: Some(artifact.icon_key),
        slot: Some(artifact.slot),
        rarity: Some(artifact.rarity),
        effect_key: Some(artifact.effect_key),
    };

    foundation::create("content.create_artifact_definition", input)
}

pub(crate) fn find_artifact_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<ArtifactDefinition>> {
    foundation::storage_result(
        ARTIFACT_SLUG_LOOKUP.name,
        crate::db()
            .load::<ArtifactDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_artifact(id: Id<ArtifactDefinition>) -> RepoResult<Option<ArtifactDefinition>> {
    foundation::load_by_id("content.load_artifact", id)
}

pub(crate) fn page_artifacts_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<ArtifactDefinition>> {
    page_content_rows(
        "content.artifacts_by_ruleset",
        crate::db()
            .load::<ArtifactDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

pub(crate) fn create_map_object_definition(
    ruleset_id: Id<RulesetDefinition>,
    object: MapObjectContent,
) -> RepoResult<MapObjectDefinition> {
    let input: Create<MapObjectDefinition> = Create::<MapObjectDefinition> {
        ruleset_id: Some(ruleset_id.key()),
        slug: Some(object.slug),
        name: Some(object.name),
        description: Some(object.description),
        sprite_key: Some(object.sprite_key),
        icon_key: Some(object.icon_key),
        object_type: Some(object.object_type),
        footprint_w: Some(object.footprint_w),
        footprint_h: Some(object.footprint_h),
        blocking: Some(object.blocking),
        interaction_key: Some(object.interaction_key),
        refresh_rule: Some(object.refresh_rule),
    };

    foundation::create("content.create_map_object_definition", input)
}

pub(crate) fn find_map_object_by_ruleset_slug(
    ruleset_id: Id<RulesetDefinition>,
    slug: &str,
) -> RepoResult<Option<MapObjectDefinition>> {
    foundation::storage_result(
        MAP_OBJECT_SLUG_LOOKUP.name,
        crate::db()
            .load::<MapObjectDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .filter(FieldRef::new("slug").eq(slug))
            .order_asc("id")
            .limit(1)
            .try_entity(),
    )
}

pub(crate) fn load_map_object(
    id: Id<MapObjectDefinition>,
) -> RepoResult<Option<MapObjectDefinition>> {
    foundation::load_by_id("content.load_map_object", id)
}

pub(crate) fn page_map_objects_by_ruleset(
    ruleset_id: Id<RulesetDefinition>,
) -> RepoResult<Vec<MapObjectDefinition>> {
    page_content_rows(
        "content.map_objects_by_ruleset",
        crate::db()
            .load::<MapObjectDefinition>()
            .filter(FieldRef::new("ruleset_id").eq(ruleset_id.key()))
            .order_asc("slug")
            .order_asc("id"),
    )
}

fn page_content_rows<E>(
    operation: &'static str,
    query: icydb::db::FluentLoadQuery<'_, E>,
) -> RepoResult<Vec<E>>
where
    E: icydb::db::PersistedRow<Canister = domm_degens_schema::schema::DegensCanister>
        + icydb::traits::EntityValue,
{
    foundation::execute_page(operation, query, domm_game::MAX_LIST_LIMIT, None)
        .map(|page| page.items)
}

pub(crate) fn row_resource_cost(
    gold: u32,
    wood: u32,
    stone: u32,
    iron: u32,
    crystal: u32,
    ember: u32,
    aether: u32,
) -> ResourceCost {
    ResourceCost {
        gold,
        wood,
        stone,
        iron,
        crystal,
        ember,
        aether,
    }
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
