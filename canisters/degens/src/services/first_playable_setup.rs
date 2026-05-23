use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use domm_degens_schema::schema::{
    ArtifactDefinition, BuildingDefinition, Champion, ChampionClassDefinition, FactionDefinition,
    GameParticipant, GameSession, MapChunk, MapObjectDefinition, MapOccupancy, NeutralArmy,
    ParticipantKnownObject, ResourceLedgerTurnSummary, SpellDefinition, TerrainDefinition,
    UnitDefinition, VisibilityChunk,
};
use domm_game::{
    FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION, FixtureIds, ResourceCost,
    first_playable_content_manifest, first_playable_scenario,
};
use icydb::{
    traits::EntityValue,
    types::{Blob, Id, Timestamp, Ulid},
};

use crate::repos::{
    champions_artifacts, content, economy, economy_expansion, foundation, map_visibility_occupancy,
    neutrals, sessions, towns,
};

use super::{battle_start, render_projection, town_runtime};

thread_local! {
    static FIRST_PLAYABLE_CONTENT_ROWS_CACHE: RefCell<Option<FirstPlayableContentRows>> =
        const { RefCell::new(None) };
    static FIRST_PLAYABLE_MAP_STATE_CACHE: RefCell<Option<(String, domm_game::FirstPlayableMapState)>> =
        const { RefCell::new(None) };
}

#[derive(Clone)]
pub(crate) struct FirstPlayableContentRows {
    pub factions: BTreeMap<String, FactionDefinition>,
    pub champion_classes: BTreeMap<String, ChampionClassDefinition>,
    pub units: BTreeMap<String, UnitDefinition>,
    pub buildings: BTreeMap<String, BuildingDefinition>,
    pub artifacts: BTreeMap<String, ArtifactDefinition>,
    pub map_objects: BTreeMap<String, MapObjectDefinition>,
}

pub(crate) fn ensure_first_playable_content_rows()
-> foundation::RepoResult<FirstPlayableContentRows> {
    if let Some(rows) = cached_first_playable_content_rows() {
        return Ok(rows);
    }

    let manifest = first_playable_content_manifest();
    let ruleset = match content::find_ruleset_by_slug_version(
        FIRST_PLAYABLE_RULESET_SLUG,
        FIRST_PLAYABLE_RULESET_VERSION,
    )? {
        Some(ruleset) => ruleset,
        None => content::create_ruleset_definition(
            manifest.ruleset.slug.clone(),
            manifest.ruleset.version,
            manifest.ruleset.name.clone(),
            manifest.ruleset.description.clone(),
            Some(manifest.ruleset.content_manifest_hash.clone()),
        )?,
    };

    let mut factions = BTreeMap::new();
    for faction in manifest.factions {
        let row = match content::find_faction_by_ruleset_slug(ruleset.id(), &faction.slug)? {
            Some(row) => row,
            None => content::create_faction_definition(
                ruleset.id(),
                faction.slug.clone(),
                faction.name,
                faction.trait_key,
            )?,
        };
        factions.insert(faction.slug, row);
    }

    let manifest = first_playable_content_manifest();
    seed_content_definition_batches(ruleset.id(), &manifest, &factions)?;
    let champion_classes = content::page_champion_classes_by_ruleset(ruleset.id())?
        .into_iter()
        .map(|row| (row.slug.clone(), row))
        .collect();
    let units = content::page_units_by_ruleset(ruleset.id())?
        .into_iter()
        .map(|row| (row.slug.clone(), row))
        .collect();
    let buildings = content::page_buildings_by_ruleset(ruleset.id())?
        .into_iter()
        .map(|row| (row.slug.clone(), row))
        .collect();
    let artifacts = content::page_artifacts_by_ruleset(ruleset.id())?
        .into_iter()
        .map(|row| (row.slug.clone(), row))
        .collect();
    let map_objects = content::page_map_objects_by_ruleset(ruleset.id())?
        .into_iter()
        .map(|row| (row.slug.clone(), row))
        .collect();

    let rows = FirstPlayableContentRows {
        factions,
        champion_classes,
        units,
        buildings,
        artifacts,
        map_objects,
    };
    remember_first_playable_content_rows(&rows);
    Ok(rows)
}

fn cached_first_playable_content_rows() -> Option<FirstPlayableContentRows> {
    FIRST_PLAYABLE_CONTENT_ROWS_CACHE.with_borrow(Clone::clone)
}

fn remember_first_playable_content_rows(rows: &FirstPlayableContentRows) {
    FIRST_PLAYABLE_CONTENT_ROWS_CACHE.with_borrow_mut(|cache| *cache = Some(rows.clone()));
}

fn seed_content_definition_batches(
    ruleset_id: Id<domm_degens_schema::schema::RulesetDefinition>,
    manifest: &domm_game::ContentManifest,
    factions: &BTreeMap<String, FactionDefinition>,
) -> foundation::RepoResult<()> {
    let now = Timestamp::now();
    let existing_champion_class_slugs = content::page_champion_classes_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.slug)
        .collect::<BTreeSet<_>>();
    let mut champion_class_rows = Vec::new();
    for class in &manifest.champion_classes {
        if !existing_champion_class_slugs.contains(&class.slug) {
            champion_class_rows.push(ChampionClassDefinition {
                id: Ulid::generate(),
                ruleset_id: ruleset_id.key(),
                faction_id: class
                    .faction_slug
                    .as_ref()
                    .and_then(|slug| factions.get(slug))
                    .map(|row| row.id().key()),
                slug: class.slug.clone(),
                name: class.name.clone(),
                description: class.description.clone(),
                portrait_key: class.portrait_key.clone(),
                base_movement: class.base_movement,
                base_vision: class.base_vision,
                created_at: now,
                updated_at: now,
            });
        }
    }
    if !champion_class_rows.is_empty() {
        foundation::insert_many_atomic("content.seed_champion_classes", champion_class_rows)?;
    }

    let existing_terrain_keys = content::page_terrain_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.terrain_key)
        .collect::<BTreeSet<_>>();
    let mut terrain_rows_by_key = BTreeMap::new();
    for terrain in &manifest.terrain {
        if !existing_terrain_keys.contains(&terrain.terrain_key) {
            terrain_rows_by_key
                .entry(terrain.terrain_key.clone())
                .or_insert_with(|| TerrainDefinition {
                    id: Ulid::generate(),
                    ruleset_id: ruleset_id.key(),
                    terrain_key: terrain.terrain_key.clone(),
                    terrain_code: terrain.terrain_code,
                    name: terrain.name.clone(),
                    movement_cost: terrain.movement_cost,
                    passable: terrain.passable,
                    sprite_key: terrain.sprite_key.clone(),
                    created_at: now,
                    updated_at: now,
                });
        }
    }
    if !terrain_rows_by_key.is_empty() {
        foundation::insert_many_atomic(
            "content.seed_terrain",
            terrain_rows_by_key.into_values().collect::<Vec<_>>(),
        )?;
    }

    let existing_unit_slugs = content::page_units_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.slug)
        .collect::<BTreeSet<_>>();
    let mut unit_rows = Vec::new();
    for unit in &manifest.units {
        if !existing_unit_slugs.contains(&unit.slug) {
            unit_rows.push(UnitDefinition {
                id: Ulid::generate(),
                ruleset_id: ruleset_id.key(),
                faction_id: unit
                    .faction_slug
                    .as_ref()
                    .and_then(|slug| factions.get(slug))
                    .map(|row| row.id().key()),
                slug: unit.slug.clone(),
                name: unit.name.clone(),
                description: unit.description.clone(),
                sprite_key: unit.sprite_key.clone(),
                icon_key: unit.icon_key.clone(),
                animation_key: unit.animation_key.clone(),
                tier: unit.tier,
                attack: unit.attack,
                defense: unit.defense,
                damage_min: unit.damage_min,
                damage_max: unit.damage_max,
                max_hp: unit.max_hp,
                speed: unit.speed,
                initiative: unit.initiative,
                ranged: unit.ranged,
                flying: unit.flying,
                shots: unit.shots,
                gold_cost: unit.cost.gold,
                wood_cost: unit.cost.wood,
                stone_cost: unit.cost.stone,
                iron_cost: unit.cost.iron,
                crystal_cost: unit.cost.crystal,
                ember_cost: unit.cost.ember,
                aether_cost: unit.cost.aether,
                weekly_growth: unit.weekly_growth,
                ability_keys: unit.ability_keys.clone(),
                created_at: now,
                updated_at: now,
            });
        }
    }
    if !unit_rows.is_empty() {
        let rows = foundation::insert_many_atomic("content.seed_units", unit_rows)?;
        content::cache_units(&rows);
    }

    let existing_building_slugs = content::page_buildings_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.slug)
        .collect::<BTreeSet<_>>();
    let mut building_rows = Vec::new();
    for building in &manifest.buildings {
        if !existing_building_slugs.contains(&building.slug) {
            building_rows.push(BuildingDefinition {
                id: Ulid::generate(),
                ruleset_id: ruleset_id.key(),
                faction_id: building
                    .faction_slug
                    .as_ref()
                    .and_then(|slug| factions.get(slug))
                    .map(|row| row.id().key()),
                slug: building.slug.clone(),
                name: building.name.clone(),
                description: building.description.clone(),
                icon_key: building.icon_key.clone(),
                building_type: building.building_type.clone(),
                gold_cost: building.cost.gold,
                wood_cost: building.cost.wood,
                stone_cost: building.cost.stone,
                iron_cost: building.cost.iron,
                crystal_cost: building.cost.crystal,
                ember_cost: building.cost.ember,
                aether_cost: building.cost.aether,
                requires_building_slugs: building.requires_building_slugs.clone(),
                unlocks_unit_slug: building.unlocks_unit_slug.clone(),
                effect_key: building.effect_key.clone(),
                created_at: now,
                updated_at: now,
            });
        }
    }
    if !building_rows.is_empty() {
        let rows = foundation::insert_many_atomic("content.seed_buildings", building_rows)?;
        content::cache_buildings(&rows);
    }

    if !manifest.spells.is_empty() {
        let preferred_cache_slug = manifest.spells.last().map(|spell| spell.slug.as_str());
        let existing_spell_slugs = content::page_spells_by_ruleset(ruleset_id)?
            .into_iter()
            .map(|row| row.slug)
            .collect::<BTreeSet<_>>();
        let mut spell_rows = Vec::new();
        for spell in &manifest.spells {
            if !existing_spell_slugs.contains(&spell.slug) {
                spell_rows.push(SpellDefinition {
                    id: Ulid::generate(),
                    ruleset_id: ruleset_id.key(),
                    slug: spell.slug.clone(),
                    name: spell.name.clone(),
                    description: spell.description.clone(),
                    icon_key: spell.icon_key.clone(),
                    school: spell.school.clone(),
                    level: spell.level,
                    mana_cost: spell.mana_cost,
                    target_type: spell.target_type.clone(),
                    effect_key: spell.effect_key.clone(),
                    duration_rounds: spell.duration_rounds,
                    created_at: now,
                    updated_at: now,
                });
            }
        }
        if !spell_rows.is_empty() {
            foundation::insert_many_atomic("content.seed_spells", spell_rows)?;
        }
        if let Some(preferred_slug) = preferred_cache_slug
            && let Some(preferred) =
                content::find_spell_by_ruleset_slug(ruleset_id, preferred_slug)?
        {
            content::cache_seeded_spells(&[preferred], Some(preferred_slug));
        }
    }

    let existing_artifact_slugs = content::page_artifacts_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.slug)
        .collect::<BTreeSet<_>>();
    let mut artifact_rows = Vec::new();
    for artifact in &manifest.artifacts {
        if !existing_artifact_slugs.contains(&artifact.slug) {
            artifact_rows.push(ArtifactDefinition {
                id: Ulid::generate(),
                ruleset_id: ruleset_id.key(),
                slug: artifact.slug.clone(),
                name: artifact.name.clone(),
                description: artifact.description.clone(),
                icon_key: artifact.icon_key.clone(),
                slot: artifact.slot.clone(),
                rarity: artifact.rarity.clone(),
                effect_key: artifact.effect_key.clone(),
                created_at: now,
                updated_at: now,
            });
        }
    }
    if !artifact_rows.is_empty() {
        foundation::insert_many_atomic("content.seed_artifacts", artifact_rows)?;
    }

    let existing_map_object_slugs = content::page_map_objects_by_ruleset(ruleset_id)?
        .into_iter()
        .map(|row| row.slug)
        .collect::<BTreeSet<_>>();
    let mut map_object_rows = Vec::new();
    for object in &manifest.map_objects {
        if !existing_map_object_slugs.contains(&object.slug) {
            map_object_rows.push(MapObjectDefinition {
                id: Ulid::generate(),
                ruleset_id: ruleset_id.key(),
                slug: object.slug.clone(),
                name: object.name.clone(),
                description: object.description.clone(),
                sprite_key: object.sprite_key.clone(),
                icon_key: object.icon_key.clone(),
                object_type: object.object_type.clone(),
                footprint_w: object.footprint_w,
                footprint_h: object.footprint_h,
                blocking: object.blocking,
                interaction_key: object.interaction_key.clone(),
                refresh_rule: object.refresh_rule.clone(),
                created_at: now,
                updated_at: now,
            });
        }
    }
    if !map_object_rows.is_empty() {
        foundation::insert_many_atomic("content.seed_map_objects", map_object_rows)?;
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn seed_content_definition_batches_for_tests(
    ruleset_id: Id<domm_degens_schema::schema::RulesetDefinition>,
    manifest: &domm_game::ContentManifest,
    factions: &BTreeMap<String, FactionDefinition>,
) -> foundation::RepoResult<()> {
    seed_content_definition_batches(ruleset_id, manifest, factions)
}

pub(crate) fn seed_first_playable_towns(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    let participants_by_slot = participants_by_slot(participants);
    seed_towns(
        session,
        &participants_by_slot,
        &content_rows,
        &mut BTreeMap::new(),
    )
}

pub(crate) fn seed_first_playable_champions(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    let participants_by_slot = participants_by_slot(participants);
    let mut champion_keys = BTreeMap::new();
    seed_champions(
        session,
        &participants_by_slot,
        &content_rows,
        &mut champion_keys,
    )?;
    seed_west_artifact(session, &content_rows, &champion_keys)
}

pub(crate) fn seed_first_playable_neutrals(session: &GameSession) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    seed_neutrals(session, &content_rows, &mut BTreeMap::new())
}

pub(crate) fn seed_first_playable_world_objects(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    let participants_by_slot = participants_by_slot(participants);
    let neutral_keys = neutral_keys_for_session(session)?;
    seed_world_objects(session, &participants_by_slot, &content_rows, &neutral_keys)
}

pub(crate) fn seed_first_playable_resource_piles(
    session: &GameSession,
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    seed_resource_pile_objects(session, &content_rows)
}

pub(crate) fn seed_first_playable_external_dwellings(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    let participants_by_slot = participants_by_slot(participants);
    seed_external_dwelling_objects(session, &participants_by_slot, &content_rows)
}

pub(crate) fn seed_first_playable_dwelling_pools(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    seed_dwelling_pools(session, &participants_by_slot(participants), &content_rows)
}

pub(crate) fn seed_first_playable_map_chunks(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    seed_map_chunks(session, participants)
}

pub(crate) fn seed_first_playable_visibility(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    seed_visibility_chunks(session, participants)?;
    seed_known_objects(session, participants)
}

pub(crate) fn seed_first_playable_occupancy(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    seed_occupancy(session, participants)
}

pub(crate) fn seed_first_playable_economy(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    seed_economy_summaries(session, participants)
}

pub(crate) fn seed_first_playable_tavern_offers(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let content_rows = ensure_first_playable_content_rows()?;
    seed_tavern_offers(session, participants, &content_rows)
}

pub(crate) fn seed_first_playable_scenario_progress(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    super::scenario_progress::ensure_seeded_scenario_progress(session, participants)
}

pub(crate) fn seed_first_playable_worldgen(session: &GameSession) -> foundation::RepoResult<()> {
    super::worldgen::ensure_seeded_worldgen_state(session, None).map(|_| ())
}

fn participants_by_slot(participants: &[GameParticipant]) -> BTreeMap<u8, GameParticipant> {
    participants
        .iter()
        .map(|participant| (participant.slot_index, participant.clone()))
        .collect()
}

fn seed_towns(
    session: &GameSession,
    participants_by_slot: &BTreeMap<u8, GameParticipant>,
    content_rows: &FirstPlayableContentRows,
    town_keys: &mut BTreeMap<String, Id<domm_degens_schema::schema::Town>>,
) -> foundation::RepoResult<()> {
    let scenario = first_playable_scenario();
    let hall = require_building(content_rows, "crumbling-hall")?;
    for start in &scenario.starts {
        let participant = require_slot(participants_by_slot, start.slot_index)?;
        let faction = require_faction(content_rows, &start.faction_slug)?;
        let town = match towns::find_town_by_session_xy(session.id(), start.town_x, start.town_y)? {
            Some(row) => row,
            None => towns::create_town(
                session.id(),
                Some(participant.id()),
                faction.id(),
                start.town_name.clone(),
                start.town_x,
                start.town_y,
                chunk_coord(start.town_x),
                chunk_coord(start.town_y),
                "active".to_string(),
                scenario.starting_state.town_hall_level,
                0,
                0,
                1,
                1,
                0,
            )?,
        };
        town_runtime::seed_town(&town);
        if towns::find_town_building(town.id(), hall.id())?.is_none() {
            let building = towns::create_town_building(
                session.id(),
                town.id(),
                hall.id(),
                hall.slug.clone(),
                1,
            )?;
            town_runtime::mirror_building(&building);
        }
        town_keys.insert(start.town_key.clone(), town.id());
    }
    Ok(())
}

fn seed_champions(
    session: &GameSession,
    participants_by_slot: &BTreeMap<u8, GameParticipant>,
    content_rows: &FirstPlayableContentRows,
    champion_keys: &mut BTreeMap<String, Id<Champion>>,
) -> foundation::RepoResult<()> {
    let scenario = first_playable_scenario();
    for start in &scenario.starts {
        let participant = require_slot(participants_by_slot, start.slot_index)?;
        let class = require_champion_class(content_rows, &start.champion_class_slug)?;
        let champion = match champions_artifacts::find_champion_by_session_xy(
            session.id(),
            start.champion_x,
            start.champion_y,
        )? {
            Some(row) => row,
            None => champions_artifacts::create_champion(
                session.id(),
                participant.id(),
                class.id(),
                start.champion_name.clone(),
                start.champion_class_slug.clone(),
                "active".to_string(),
                start.champion_x,
                start.champion_y,
                chunk_coord(start.champion_x),
                chunk_coord(start.champion_y),
                u16::from(scenario.starting_state.champion_level),
                0,
                1,
                1,
                1,
                1,
                10,
                10,
                1,
                1,
                Vec::new(),
                scenario.starting_state.champion_movement,
                scenario.starting_state.champion_movement,
                1,
                scenario.starting_state.champion_vision,
                0,
            )?,
        };
        sessions::ensure_participant_champion_id(participant.clone(), champion.id())?;
        let mut seeded_stacks = Vec::new();
        for stack in &start.starting_army_stacks {
            let unit = require_unit(content_rows, &stack.unit_slug)?;
            let row = match champions_artifacts::find_champion_army_stack(
                champion.id(),
                stack.slot_index,
            )? {
                Some(row) => row,
                None => champions_artifacts::create_champion_army_stack(
                    session.id(),
                    champion.id(),
                    unit.id(),
                    stack.slot_index,
                    u32::from(stack.quantity),
                    unit.max_hp,
                    "active".to_string(),
                )?,
            };
            seeded_stacks.push(row);
        }
        render_projection::remember_champion_stack_rows(champion.id(), seeded_stacks.clone());
        battle_start::remember_seeded_champion_army_stacks(champion.id(), seeded_stacks);
        champion_keys.insert(start.champion_key.clone(), champion.id());
    }
    Ok(())
}

fn seed_west_artifact(
    session: &GameSession,
    content_rows: &FirstPlayableContentRows,
    champion_keys: &BTreeMap<String, Id<Champion>>,
) -> foundation::RepoResult<()> {
    let Some(champion_id) = champion_keys.get("champion:west").copied() else {
        return Ok(());
    };
    if let Some(equipment) =
        champions_artifacts::find_equipment_by_champion_slot(champion_id, "banner")?
    {
        render_projection::remember_champion_banner_artifact(
            champion_id,
            Id::<domm_degens_schema::schema::ArtifactInstance>::from_key(equipment.artifact_id)
                .to_string(),
        );
        return Ok(());
    }
    let artifact = require_artifact(content_rows, "bent-banner")?;
    let instance = champions_artifacts::create_artifact_instance(
        session.id(),
        artifact.id(),
        Some(champion_id),
        Some("banner".to_string()),
        0,
        0,
        0,
        0,
        "equipped".to_string(),
    )?;
    champions_artifacts::create_artifact_equipment(
        session.id(),
        champion_id,
        instance.id(),
        "banner".to_string(),
        1,
    )?;
    render_projection::remember_champion_banner_artifact(champion_id, instance.id().to_string());
    Ok(())
}

fn seed_neutrals(
    session: &GameSession,
    content_rows: &FirstPlayableContentRows,
    neutral_keys: &mut BTreeMap<String, Id<NeutralArmy>>,
) -> foundation::RepoResult<()> {
    let scenario = first_playable_scenario();
    for neutral in &scenario.neutral_armies {
        if !matches!(
            neutral.key.as_str(),
            "neutral:west-mine" | "neutral:east-mine"
        ) {
            continue;
        }
        let army =
            match neutrals::find_neutral_army_by_session_xy(session.id(), neutral.x, neutral.y)? {
                Some(row) => row,
                None => neutrals::create_neutral_army(
                    session.id(),
                    neutral.x,
                    neutral.y,
                    chunk_coord(neutral.x),
                    chunk_coord(neutral.y),
                    "active".to_string(),
                    "guard".to_string(),
                    "none".to_string(),
                    1,
                )?,
            };
        let mut seeded_stacks = Vec::new();
        for stack in &neutral.stacks {
            let unit = require_unit(content_rows, &stack.unit_slug)?;
            let row = match neutrals::find_neutral_army_stack(army.id(), stack.slot_index)? {
                Some(row) => row,
                None => neutrals::create_neutral_army_stack(
                    session.id(),
                    army.id(),
                    unit.id(),
                    stack.slot_index,
                    u32::from(stack.quantity),
                    unit.max_hp,
                )?,
            };
            seeded_stacks.push(row);
        }
        battle_start::remember_seeded_neutral_army_stacks(army.id(), seeded_stacks);
        render_projection::remember_neutral_armies(std::slice::from_ref(&army));
        neutral_keys.insert(neutral.key.clone(), army.id());
    }
    Ok(())
}

fn neutral_keys_for_session(
    session: &GameSession,
) -> foundation::RepoResult<BTreeMap<String, Id<NeutralArmy>>> {
    let scenario = first_playable_scenario();
    let mut keys = BTreeMap::new();
    for neutral in &scenario.neutral_armies {
        if let Some(row) =
            neutrals::find_neutral_army_by_session_xy(session.id(), neutral.x, neutral.y)?
        {
            keys.insert(neutral.key.clone(), row.id());
        }
    }
    Ok(keys)
}

fn seed_world_objects(
    session: &GameSession,
    participants_by_slot: &BTreeMap<u8, GameParticipant>,
    content_rows: &FirstPlayableContentRows,
    neutral_keys: &BTreeMap<String, Id<NeutralArmy>>,
) -> foundation::RepoResult<()> {
    let scenario = first_playable_scenario();
    for object in scenario
        .mines
        .iter()
        .chain(scenario.central_objectives.iter())
    {
        let def = require_map_object(content_rows, &object.object_slug)?;
        let owner = object
            .owner_slot_index
            .and_then(|slot| participants_by_slot.get(&slot))
            .map(EntityValue::id);
        let guard = object
            .guard_neutral_army_key
            .as_ref()
            .and_then(|key| neutral_keys.get(key))
            .copied();
        if map_visibility_occupancy::find_world_object_by_session_xy(
            session.id(),
            object.x,
            object.y,
        )?
        .is_none()
        {
            map_visibility_occupancy::create_world_object(
                session.id(),
                def.id(),
                owner,
                guard,
                object.x,
                object.y,
                chunk_coord(object.x),
                chunk_coord(object.y),
                "available".to_string(),
                object_scoring_kind(&object.object_slug).to_string(),
                0,
                0,
                1,
                Some(world_object_json(&object.key, &object.object_slug, None)),
            )?;
        }
    }
    Ok(())
}

fn seed_resource_pile_objects(
    session: &GameSession,
    content_rows: &FirstPlayableContentRows,
) -> foundation::RepoResult<()> {
    for pile in &first_playable_scenario().resource_piles {
        let def = require_map_object(content_rows, &pile.object_slug)?;
        if map_visibility_occupancy::find_world_object_by_session_xy(session.id(), pile.x, pile.y)?
            .is_none()
        {
            map_visibility_occupancy::create_world_object(
                session.id(),
                def.id(),
                None,
                None,
                pile.x,
                pile.y,
                chunk_coord(pile.x),
                chunk_coord(pile.y),
                "available".to_string(),
                "resource_pile".to_string(),
                0,
                0,
                1,
                Some(world_object_json(
                    &pile.key,
                    &pile.object_slug,
                    Some(&pile.reward),
                )),
            )?;
        }
    }
    Ok(())
}

fn seed_external_dwelling_objects(
    session: &GameSession,
    participants_by_slot: &BTreeMap<u8, GameParticipant>,
    content_rows: &FirstPlayableContentRows,
) -> foundation::RepoResult<()> {
    for object in &first_playable_scenario().external_dwellings {
        let def = require_map_object(content_rows, &object.object_slug)?;
        if map_visibility_occupancy::find_world_object_by_session_xy(
            session.id(),
            object.x,
            object.y,
        )?
        .is_some()
        {
            continue;
        }
        let owner = object
            .owner_slot_index
            .and_then(|slot| participants_by_slot.get(&slot))
            .map(EntityValue::id);
        map_visibility_occupancy::create_world_object(
            session.id(),
            def.id(),
            owner,
            None,
            object.x,
            object.y,
            chunk_coord(object.x),
            chunk_coord(object.y),
            "available".to_string(),
            object_scoring_kind(&object.object_slug).to_string(),
            0,
            0,
            1,
            Some(world_object_json(&object.key, &object.object_slug, None)),
        )?;
    }
    Ok(())
}

fn with_first_playable_map_state<T>(
    session: &GameSession,
    participants: &[GameParticipant],
    body: impl FnOnce(&domm_game::FirstPlayableMapState) -> foundation::RepoResult<T>,
) -> foundation::RepoResult<T> {
    let ids = fixture_ids_for_rows(session, participants);
    let cache_key = first_playable_map_state_cache_key(&ids);
    FIRST_PLAYABLE_MAP_STATE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let should_refresh = cache
            .as_ref()
            .is_none_or(|(cached_key, _)| cached_key != &cache_key);
        if should_refresh {
            let map_state =
                domm_game::build_first_playable_map_state_for_ids(&ids).map_err(map_seed_error)?;
            *cache = Some((cache_key, map_state));
        }
        let Some((_, map_state)) = cache.as_ref() else {
            return Err(map_seed_error(domm_game::MapError::OutOfBounds {
                x: 0,
                y: 0,
            }));
        };
        body(map_state)
    })
}

fn first_playable_map_state_cache_key(ids: &FixtureIds) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        ids.session_id,
        ids.player_one_id,
        ids.player_two_id,
        ids.participant_one_id,
        ids.participant_two_id,
        ids.map_id
    )
}

fn map_seed_error(error: domm_game::MapError) -> domm_game::ApiError {
    domm_game::ApiError::new("first_playable_map_seed_invalid", error.to_string(), false)
}

fn seed_map_chunks(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    if map_visibility_occupancy::find_map_chunk(session.id(), 0, 0)?.is_none() {
        with_first_playable_map_state(session, participants, |map_state| {
            let now = Timestamp::now();
            let rows = map_state
                .chunks
                .iter()
                .map(|chunk| MapChunk {
                    id: Ulid::generate(),
                    session_id: session.id().key(),
                    chunk_x: chunk.chunk_x,
                    chunk_y: chunk.chunk_y,
                    width: u8::try_from(chunk.width).unwrap_or(u8::MAX),
                    height: u8::try_from(chunk.height).unwrap_or(u8::MAX),
                    terrain_blob: Blob::from(chunk.terrain_blob.clone()),
                    movement_blob: Blob::from(chunk.movement_blob.clone()),
                    flags_blob: Blob::from(chunk.flags_blob.clone()),
                    created_at: now,
                    updated_at: now,
                })
                .collect::<Vec<_>>();
            let rows = foundation::insert_many_atomic("map.seed_map_chunks", rows)?;
            render_projection::remember_map_chunks(&rows);
            Ok(())
        })?;
    }
    Ok(())
}

fn seed_visibility_chunks(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let first_participant_id = participants.first().map(EntityValue::id);
    if first_participant_id
        .map(|id| map_visibility_occupancy::find_visibility_chunk(id, 0, 0))
        .transpose()?
        .flatten()
        .is_none()
    {
        with_first_playable_map_state(session, participants, |map_state| {
            let now = Timestamp::now();
            let rows = map_state
                .visibility_chunks
                .iter()
                .map(|visibility| {
                    Ok(VisibilityChunk {
                        id: Ulid::generate(),
                        session_id: session.id().key(),
                        participant_id: parse_participant_id(&visibility.participant_id)?.key(),
                        chunk_x: visibility.chunk_x,
                        chunk_y: visibility.chunk_y,
                        discovered_blob: Blob::from(visibility.discovered_blob.clone()),
                        visible_blob: Blob::from(visibility.visible_blob.clone()),
                        visible_turn: visibility.visible_turn,
                        created_at: now,
                        updated_at: now,
                    })
                })
                .collect::<foundation::RepoResult<Vec<_>>>()?;
            let rows = foundation::insert_many_atomic("map.seed_visibility_chunks", rows)?;
            render_projection::remember_visibility_chunks(&rows);
            Ok(())
        })?;
    }
    Ok(())
}

fn seed_known_objects(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let first_participant_id = participants.first().map(EntityValue::id);
    if first_participant_id
        .map(|id| map_visibility_occupancy::find_known_object(id, "town", "town:west"))
        .transpose()?
        .flatten()
        .is_none()
    {
        with_first_playable_map_state(session, participants, |map_state| {
            let now = Timestamp::now();
            let rows = map_state
                .known_objects
                .iter()
                .map(|known| {
                    Ok(ParticipantKnownObject {
                        id: Ulid::generate(),
                        session_id: session.id().key(),
                        participant_id: parse_participant_id(&known.participant_id)?.key(),
                        subject_kind: known.subject_kind.clone(),
                        subject_id_text: known.subject_id_text.clone(),
                        x: known.x,
                        y: known.y,
                        chunk_x: known.chunk_x,
                        chunk_y: known.chunk_y,
                        visibility: known.visibility.clone(),
                        last_seen_turn: known.last_seen_turn,
                        redacted_json: Some(known.redacted_json.clone()),
                        created_at: now,
                        updated_at: now,
                    })
                })
                .collect::<foundation::RepoResult<Vec<_>>>()?;
            let rows = foundation::insert_many_atomic("map.seed_known_objects", rows)?;
            render_projection::remember_known_objects(&rows);
            Ok(())
        })?;
    }
    Ok(())
}

fn seed_occupancy(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    if map_visibility_occupancy::find_occupancy_cell(session.id(), 6, 24, "town")?.is_none() {
        with_first_playable_map_state(session, participants, |map_state| {
            let now = Timestamp::now();
            let rows = map_state
                .occupancy_rows
                .iter()
                .filter(|row| matches!(row.occupant_kind.as_str(), "town" | "champion"))
                .map(|row| {
                    let occupant_id_text = match row.occupant_kind.as_str() {
                        "champion" => champions_artifacts::find_champion_by_session_xy(
                            session.id(),
                            row.x,
                            row.y,
                        )?
                        .map(|champion| champion.id().to_string())
                        .unwrap_or_else(|| row.occupant_id_text.clone()),
                        "town" => towns::find_town_by_session_xy(session.id(), row.x, row.y)?
                            .map(|town| town.id().to_string())
                            .unwrap_or_else(|| row.occupant_id_text.clone()),
                        _ => row.occupant_id_text.clone(),
                    };
                    Ok(MapOccupancy {
                        id: Ulid::generate(),
                        session_id: session.id().key(),
                        x: row.x,
                        y: row.y,
                        chunk_x: row.chunk_x,
                        chunk_y: row.chunk_y,
                        layer: row.layer.clone(),
                        occupant_kind: row.occupant_kind.clone(),
                        occupant_id_text,
                        occupant_cell_index: u8::try_from(row.occupant_cell_index)
                            .unwrap_or(u8::MAX),
                        blocking: row.blocking,
                        last_command_id: None,
                        created_at: now,
                        updated_at: now,
                    })
                })
                .collect::<foundation::RepoResult<Vec<_>>>()?;
            foundation::insert_many_atomic("map.seed_occupancy", rows)?;
            Ok(())
        })?;
    }
    Ok(())
}

fn seed_economy_summaries(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    for participant in participants {
        if economy::find_resource_turn_summary(session.id(), participant.id(), 1)?.is_some() {
            return Ok(());
        }
    }
    let now = Timestamp::now();
    let rows = participants
        .iter()
        .map(|participant| ResourceLedgerTurnSummary {
            id: Ulid::generate(),
            session_id: session.id().key(),
            participant_id: participant.id().key(),
            turn_number: 1,
            summary_json: format!(
                "{{\"kind\":\"opening_balance\",\"gold\":{},\"wood\":{},\"stone\":{},\"iron\":{},\"crystal\":{},\"ember\":{},\"aether\":{}}}",
                participant.gold,
                participant.wood,
                participant.stone,
                participant.iron,
                participant.crystal,
                participant.ember,
                participant.aether
            ),
            created_at: now,
            updated_at: now,
        })
        .collect::<Vec<_>>();
    foundation::insert_many_atomic("economy.seed_opening_summaries", rows)?;
    Ok(())
}

fn seed_dwelling_pools(
    session: &GameSession,
    participants_by_slot: &BTreeMap<u8, GameParticipant>,
    content_rows: &FirstPlayableContentRows,
) -> foundation::RepoResult<()> {
    let unit = require_unit(content_rows, "mudhook-levy")?;
    for object in &first_playable_scenario().external_dwellings {
        let Some(world_object) = map_visibility_occupancy::find_world_object_by_session_xy(
            session.id(),
            object.x,
            object.y,
        )?
        else {
            continue;
        };
        if let Some(pool) =
            economy_expansion::find_dwelling_pool_by_object(session.id(), world_object.id())?
        {
            crate::services::economy_expansion::mirror_runtime_dwelling_pool(&pool);
            continue;
        }
        let owner = object
            .owner_slot_index
            .and_then(|slot| participants_by_slot.get(&slot))
            .map(EntityValue::id);
        let pool = economy_expansion::create_dwelling_pool(
            session.id(),
            world_object.id(),
            owner,
            unit.id(),
            unit.slug.clone(),
            u32::from(domm_game::DWELLING_GROWTH_PER_WEEK),
            1,
            domm_game::DWELLING_GROWTH_PER_WEEK,
            true,
        )?;
        crate::services::economy_expansion::mirror_runtime_dwelling_pool(&pool);
    }
    Ok(())
}

fn seed_tavern_offers(
    session: &GameSession,
    participants: &[GameParticipant],
    content_rows: &FirstPlayableContentRows,
) -> foundation::RepoResult<()> {
    let scenario = first_playable_scenario();
    let participants_by_slot = participants_by_slot(participants);
    let class_slugs = content_rows
        .champion_classes
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let week_number = 1;
    for start in &scenario.starts {
        let Some(participant) = participants_by_slot.get(&start.slot_index) else {
            continue;
        };
        let town = if let Some(town) =
            town_runtime::cached_town_by_xy(session.id(), start.town_x, start.town_y)
        {
            town
        } else {
            let Some(town) =
                towns::find_town_by_session_xy(session.id(), start.town_x, start.town_y)?
            else {
                continue;
            };
            town
        };
        for slot in 0..domm_game::TAVERN_OFFERS_PER_WEEK {
            let offer = domm_game::deterministic_tavern_offer(
                &session.seed.to_string(),
                &town.id().to_string(),
                week_number,
                u8::try_from(slot).unwrap_or(u8::MAX),
                &class_slugs,
            );
            if town_runtime::cached_tavern_offer_by_key(&town, &offer.offer_key).is_some() {
                continue;
            }
            let class = require_champion_class(content_rows, &offer.champion_class_slug)?;
            let offer = economy_expansion::create_tavern_offer(
                session.id(),
                town.id(),
                participant.id(),
                week_number,
                offer.offer_slot,
                offer.offer_key,
                class.id(),
                offer.champion_class_slug,
                offer.candidate_name,
                offer.cost_gold,
            )?;
            town_runtime::mirror_tavern_offer(&offer);
        }
    }
    Ok(())
}

fn fixture_ids_for_rows(session: &GameSession, participants: &[GameParticipant]) -> FixtureIds {
    let mut participants = participants.to_vec();
    participants.sort_by_key(|participant| participant.slot_index);
    FixtureIds {
        session_id: session.id().to_string(),
        player_one_id: participants
            .first()
            .map(|participant| {
                Id::<domm_degens_schema::schema::PlayerAccount>::from_key(participant.player_id)
                    .to_string()
            })
            .unwrap_or_default(),
        player_two_id: participants
            .get(1)
            .map(|participant| {
                Id::<domm_degens_schema::schema::PlayerAccount>::from_key(participant.player_id)
                    .to_string()
            })
            .unwrap_or_default(),
        participant_one_id: participants
            .first()
            .map(EntityValue::id)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        participant_two_id: participants
            .get(1)
            .map(EntityValue::id)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        map_id: format!("map:{}", session.id()),
    }
}

fn parse_participant_id(value: &str) -> foundation::RepoResult<Id<GameParticipant>> {
    icydb::types::Ulid::from_str(value)
        .map(Id::from_key)
        .map_err(|_| {
            domm_game::ApiError::new(
                "invalid_participant_id",
                "seed participant id was invalid",
                false,
            )
        })
}

fn require_slot<'a>(
    participants_by_slot: &'a BTreeMap<u8, GameParticipant>,
    slot_index: u8,
) -> foundation::RepoResult<&'a GameParticipant> {
    participants_by_slot.get(&slot_index).ok_or_else(|| {
        domm_game::ApiError::new(
            "setup_participant_missing",
            format!("setup participant slot {slot_index} is missing"),
            false,
        )
    })
}

fn require_faction<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a FactionDefinition> {
    rows.factions
        .get(slug)
        .ok_or_else(|| missing_content("faction", slug))
}

fn require_champion_class<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a ChampionClassDefinition> {
    rows.champion_classes
        .get(slug)
        .ok_or_else(|| missing_content("champion_class", slug))
}

fn require_unit<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a UnitDefinition> {
    rows.units
        .get(slug)
        .ok_or_else(|| missing_content("unit", slug))
}

fn require_building<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a BuildingDefinition> {
    rows.buildings
        .get(slug)
        .ok_or_else(|| missing_content("building", slug))
}

fn require_artifact<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a ArtifactDefinition> {
    rows.artifacts
        .get(slug)
        .ok_or_else(|| missing_content("artifact", slug))
}

fn require_map_object<'a>(
    rows: &'a FirstPlayableContentRows,
    slug: &str,
) -> foundation::RepoResult<&'a MapObjectDefinition> {
    rows.map_objects
        .get(slug)
        .ok_or_else(|| missing_content("map_object", slug))
}

fn missing_content(kind: &str, slug: &str) -> domm_game::ApiError {
    domm_game::ApiError::new(
        "content_seed_missing",
        format!("{kind} was not seeded: {slug}"),
        false,
    )
}

fn object_scoring_kind(object_slug: &str) -> &'static str {
    match object_slug {
        "gold-mine" | "crystal-mine" => "mine",
        "misery-beacon" => "central_objective",
        _ => "none",
    }
}

fn world_object_json(
    scenario_key: &str,
    object_slug: &str,
    reward: Option<&ResourceCost>,
) -> String {
    match reward {
        Some(reward) => format!(
            "{{\"scenario_key\":\"{}\",\"object_slug\":\"{}\",\"reward\":{{\"gold\":{},\"wood\":{},\"stone\":{},\"iron\":{},\"crystal\":{},\"ember\":{},\"aether\":{}}}}}",
            escape_json(scenario_key),
            escape_json(object_slug),
            reward.gold,
            reward.wood,
            reward.stone,
            reward.iron,
            reward.crystal,
            reward.ember,
            reward.aether
        ),
        None => format!(
            "{{\"scenario_key\":\"{}\",\"object_slug\":\"{}\"}}",
            escape_json(scenario_key),
            escape_json(object_slug)
        ),
    }
}

fn chunk_coord(value: u16) -> u16 {
    value / u16::from(domm_game::FIRST_PLAYABLE_CHUNK_SIZE)
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
