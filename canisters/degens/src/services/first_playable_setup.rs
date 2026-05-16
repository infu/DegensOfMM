use std::collections::BTreeMap;

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
    champions_artifacts, content, economy, foundation, map_visibility_occupancy, neutrals, towns,
};

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

    Ok(FirstPlayableContentRows {
        factions,
        champion_classes,
        units,
        buildings,
        artifacts,
        map_objects,
    })
}

fn seed_content_definition_batches(
    ruleset_id: Id<domm_degens_schema::schema::RulesetDefinition>,
    manifest: &domm_game::ContentManifest,
    factions: &BTreeMap<String, FactionDefinition>,
) -> foundation::RepoResult<()> {
    let now = Timestamp::now();
    if content::find_champion_class_by_ruleset_slug(ruleset_id, "toll-broken-captain")?.is_none() {
        let rows = manifest
            .champion_classes
            .iter()
            .map(|class| ChampionClassDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_champion_classes", rows)?;
    }

    if content::find_terrain_by_ruleset_key(ruleset_id, "grass")?.is_none() {
        let mut rows_by_key = BTreeMap::new();
        for terrain in &manifest.terrain {
            rows_by_key
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
        foundation::insert_many_atomic(
            "content.seed_terrain",
            rows_by_key.into_values().collect::<Vec<_>>(),
        )?;
    }

    if content::find_unit_by_ruleset_slug(ruleset_id, "mudhook-levy")?.is_none() {
        let rows = manifest
            .units
            .iter()
            .map(|unit| UnitDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_units", rows)?;
    }

    if content::find_building_by_ruleset_slug(ruleset_id, "crumbling-hall")?.is_none() {
        let rows = manifest
            .buildings
            .iter()
            .map(|building| BuildingDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_buildings", rows)?;
    }

    if !manifest.spells.is_empty()
        && content::find_spell_by_ruleset_slug(ruleset_id, &manifest.spells[0].slug)?.is_none()
    {
        let rows = manifest
            .spells
            .iter()
            .map(|spell| SpellDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_spells", rows)?;
    }

    if content::find_artifact_by_ruleset_slug(ruleset_id, "bent-banner")?.is_none() {
        let rows = manifest
            .artifacts
            .iter()
            .map(|artifact| ArtifactDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_artifacts", rows)?;
    }

    if content::find_map_object_by_ruleset_slug(ruleset_id, "gold-mine")?.is_none() {
        let rows = manifest
            .map_objects
            .iter()
            .map(|object| MapObjectDefinition {
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
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("content.seed_map_objects", rows)?;
    }

    Ok(())
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
        if towns::find_town_building(town.id(), hall.id())?.is_none() {
            towns::create_town_building(session.id(), town.id(), hall.id(), hall.slug.clone(), 1)?;
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
        for stack in &start.starting_army_stacks {
            let unit = require_unit(content_rows, &stack.unit_slug)?;
            if champions_artifacts::find_champion_army_stack(champion.id(), stack.slot_index)?
                .is_none()
            {
                champions_artifacts::create_champion_army_stack(
                    session.id(),
                    champion.id(),
                    unit.id(),
                    stack.slot_index,
                    u32::from(stack.quantity),
                    unit.max_hp,
                    "active".to_string(),
                )?;
            }
        }
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
    if champions_artifacts::find_equipment_by_champion_slot(champion_id, "banner")?.is_some() {
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
        for stack in &neutral.stacks {
            let unit = require_unit(content_rows, &stack.unit_slug)?;
            if neutrals::find_neutral_army_stack(army.id(), stack.slot_index)?.is_none() {
                neutrals::create_neutral_army_stack(
                    session.id(),
                    army.id(),
                    unit.id(),
                    stack.slot_index,
                    u32::from(stack.quantity),
                    unit.max_hp,
                )?;
            }
        }
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
    for pile in &scenario.resource_piles {
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

fn seed_map_chunks(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let ids = fixture_ids_for_rows(session, participants);
    let map_state = domm_game::build_first_playable_map_state_for_ids(&ids).map_err(|error| {
        domm_game::ApiError::new("first_playable_map_seed_invalid", error.to_string(), false)
    })?;
    if map_visibility_occupancy::find_map_chunk(session.id(), 0, 0)?.is_none() {
        let now = Timestamp::now();
        let rows = map_state
            .chunks
            .into_iter()
            .map(|chunk| MapChunk {
                id: Ulid::generate(),
                session_id: session.id().key(),
                chunk_x: chunk.chunk_x,
                chunk_y: chunk.chunk_y,
                width: u8::try_from(chunk.width).unwrap_or(u8::MAX),
                height: u8::try_from(chunk.height).unwrap_or(u8::MAX),
                terrain_blob: Blob::from(chunk.terrain_blob),
                movement_blob: Blob::from(chunk.movement_blob),
                flags_blob: Blob::from(chunk.flags_blob),
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("map.seed_map_chunks", rows)?;
    }
    Ok(())
}

fn seed_visibility_chunks(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let ids = fixture_ids_for_rows(session, participants);
    let map_state = domm_game::build_first_playable_map_state_for_ids(&ids).map_err(|error| {
        domm_game::ApiError::new("first_playable_map_seed_invalid", error.to_string(), false)
    })?;
    let first_participant_id = participants.first().map(EntityValue::id);
    if first_participant_id
        .map(|id| map_visibility_occupancy::find_visibility_chunk(id, 0, 0))
        .transpose()?
        .flatten()
        .is_none()
    {
        let now = Timestamp::now();
        let rows = map_state
            .visibility_chunks
            .into_iter()
            .map(|visibility| {
                Ok(VisibilityChunk {
                    id: Ulid::generate(),
                    session_id: session.id().key(),
                    participant_id: parse_participant_id(&visibility.participant_id)?.key(),
                    chunk_x: visibility.chunk_x,
                    chunk_y: visibility.chunk_y,
                    discovered_blob: Blob::from(visibility.discovered_blob),
                    visible_blob: Blob::from(visibility.visible_blob),
                    visible_turn: visibility.visible_turn,
                    created_at: now,
                    updated_at: now,
                })
            })
            .collect::<foundation::RepoResult<Vec<_>>>()?;
        foundation::insert_many_atomic("map.seed_visibility_chunks", rows)?;
    }
    Ok(())
}

fn seed_known_objects(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let ids = fixture_ids_for_rows(session, participants);
    let map_state = domm_game::build_first_playable_map_state_for_ids(&ids).map_err(|error| {
        domm_game::ApiError::new("first_playable_map_seed_invalid", error.to_string(), false)
    })?;
    let first_participant_id = participants.first().map(EntityValue::id);
    if first_participant_id
        .map(|id| map_visibility_occupancy::find_known_object(id, "town", "town:west"))
        .transpose()?
        .flatten()
        .is_none()
    {
        let now = Timestamp::now();
        let rows = map_state
            .known_objects
            .into_iter()
            .map(|known| {
                Ok(ParticipantKnownObject {
                    id: Ulid::generate(),
                    session_id: session.id().key(),
                    participant_id: parse_participant_id(&known.participant_id)?.key(),
                    subject_kind: known.subject_kind,
                    subject_id_text: known.subject_id_text,
                    x: known.x,
                    y: known.y,
                    chunk_x: known.chunk_x,
                    chunk_y: known.chunk_y,
                    visibility: known.visibility,
                    last_seen_turn: known.last_seen_turn,
                    redacted_json: Some(known.redacted_json),
                    created_at: now,
                    updated_at: now,
                })
            })
            .collect::<foundation::RepoResult<Vec<_>>>()?;
        foundation::insert_many_atomic("map.seed_known_objects", rows)?;
    }
    Ok(())
}

fn seed_occupancy(
    session: &GameSession,
    participants: &[GameParticipant],
) -> foundation::RepoResult<()> {
    let ids = fixture_ids_for_rows(session, participants);
    let map_state = domm_game::build_first_playable_map_state_for_ids(&ids).map_err(|error| {
        domm_game::ApiError::new("first_playable_map_seed_invalid", error.to_string(), false)
    })?;
    if map_visibility_occupancy::find_occupancy_cell(session.id(), 6, 24, "town")?.is_none() {
        let now = Timestamp::now();
        let rows = map_state
            .occupancy_rows
            .into_iter()
            .filter(|row| matches!(row.occupant_kind.as_str(), "town" | "champion"))
            .map(|row| MapOccupancy {
                id: Ulid::generate(),
                session_id: session.id().key(),
                x: row.x,
                y: row.y,
                chunk_x: row.chunk_x,
                chunk_y: row.chunk_y,
                layer: row.layer,
                occupant_kind: row.occupant_kind,
                occupant_id_text: row.occupant_id_text,
                occupant_cell_index: u8::try_from(row.occupant_cell_index).unwrap_or(u8::MAX),
                blocking: row.blocking,
                last_command_id: None,
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        foundation::insert_many_atomic("map.seed_occupancy", rows)?;
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
