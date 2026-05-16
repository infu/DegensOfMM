use std::collections::BTreeMap;

use domm_degens_schema::schema::{ArtifactInstance, Champion, GameParticipant, GameSession, Town};
use domm_game::{
    ActionAffordance, ApiError, ApiTownView, ArtifactView, ChampionArmyStackRecord, ChampionView,
    MapChunkPage, MapChunkView, ObjectView, ObjectViewPage, PageInfo, TownBuildingRecord,
    TownRecord, TownRecruitPoolRecord, Viewport,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Ulid},
};

use crate::repos::{champions_artifacts, content, foundation, map_visibility_occupancy, towns};

use super::session_context::{SessionCallerContext, public_error};

const MAX_OWNED_CHAMPIONS_VIEW: u32 = 16;

pub(crate) fn visible_map_chunks(
    context: &SessionCallerContext,
    viewport: &Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<MapChunkPage, ApiError> {
    let limit = foundation::validate_limit(
        "chunk_limit",
        limit,
        domm_game::MAX_CHUNK_LIMIT,
        "viewport_chunk_limit_exceeded",
    )?;
    let visibility_by_coord = map_visibility_occupancy::page_visibility_chunks_by_participant(
        context.session.id(),
        context.participant.id(),
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    .into_iter()
    .map(|visibility| ((visibility.chunk_x, visibility.chunk_y), visibility))
    .collect::<BTreeMap<_, _>>();
    let mut chunks = map_visibility_occupancy::page_map_chunks_by_session(
        context.session.id(),
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    .into_iter()
    .filter(|chunk| chunk_intersects_viewport(&context.session, viewport, chunk))
    .filter_map(|chunk| {
        let visibility = visibility_by_coord.get(&(chunk.chunk_x, chunk.chunk_y))?;
        Some(MapChunkView {
            chunk_id: chunk.id().to_string(),
            chunk_x: chunk.chunk_x,
            chunk_y: chunk.chunk_y,
            width: u16::from(chunk.width),
            height: u16::from(chunk.height),
            terrain_blob: chunk.terrain_blob.to_vec(),
            movement_blob: chunk.movement_blob.to_vec(),
            flags_blob: chunk.flags_blob.to_vec(),
            discovered_blob: visibility.discovered_blob.to_vec(),
            visible_blob: visibility.visible_blob.to_vec(),
        })
    })
    .collect::<Vec<_>>();
    chunks.sort_by_key(|chunk| (chunk.chunk_y, chunk.chunk_x));

    let start = cursor.unwrap_or(0) as usize;
    let limit_usize = limit as usize;
    let end = start.saturating_add(limit_usize).min(chunks.len());
    let page_items = chunks.get(start..end).unwrap_or_default().to_vec();
    let has_more = end < chunks.len();

    Ok(MapChunkPage {
        chunks: page_items,
        next_cursor: has_more.then_some(end as u32),
        has_more,
    })
}

pub(crate) fn visible_objects(
    context: &SessionCallerContext,
    viewport: &Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> Result<ObjectViewPage, ApiError> {
    let limit = foundation::validate_limit(
        "object_limit",
        limit,
        domm_game::MAX_OBJECT_LIMIT,
        "list_limit_exceeded",
    )?;
    let participants_by_slot = BTreeMap::from([(
        context.participant.slot_index,
        context.participant.id().to_string(),
    )]);
    let mut objects = Vec::new();
    let page = map_visibility_occupancy::page_known_objects_for_participant(
        context.session.id(),
        context.participant.id(),
        domm_game::MAX_LIST_LIMIT,
        None,
    )?;
    for known in page.items {
        if !viewport.contains(known.x, known.y) {
            continue;
        }
        objects.push(object_view_from_known(&known, &participants_by_slot));
    }
    objects.sort_by_key(|object| {
        (
            object.y,
            object.x,
            object.subject_kind.clone(),
            object.subject_id_text.clone(),
        )
    });
    objects.dedup_by(|left, right| {
        left.subject_kind == right.subject_kind && left.subject_id_text == right.subject_id_text
    });

    let start = cursor.unwrap_or(0) as usize;
    let limit_usize = limit as usize;
    let end = start.saturating_add(limit_usize).min(objects.len());
    let page_items = objects.get(start..end).unwrap_or_default().to_vec();
    let has_more = end < objects.len();

    Ok(ObjectViewPage {
        objects: page_items,
        next_cursor: has_more.then_some(end as u32),
        has_more,
    })
}

pub(crate) fn my_champions(context: &SessionCallerContext) -> Result<Vec<ChampionView>, ApiError> {
    let page = champions_artifacts::page_champions_by_owner_status(
        context.participant.id(),
        "active",
        MAX_OWNED_CHAMPIONS_VIEW,
        None,
    )?;
    page.items
        .into_iter()
        .map(|champion| champion_view(context, champion, true))
        .collect()
}

pub(crate) fn champion_view_by_id(
    context: &SessionCallerContext,
    champion_id: &str,
) -> Result<ChampionView, ApiError> {
    let champion = resolve_champion(&context.session, champion_id)?;
    let own = champion.participant_id == context.participant.id().key();
    if !own
        && !is_visible_at(
            &context.session,
            context.participant.id(),
            champion.x,
            champion.y,
        )?
    {
        return Err(public_error("not_visible", "champion is hidden", false));
    }
    champion_view(context, champion, own)
}

pub(crate) fn action_affordances(
    champions: &[ChampionView],
    towns: &[ApiTownView],
    sync_required: bool,
) -> Vec<ActionAffordance> {
    let mut actions = vec![ActionAffordance {
        action: "sync_session_turn".to_string(),
        enabled: sync_required,
        target_id: None,
        disabled_reason: (!sync_required).then(|| "turn_not_due".to_string()),
    }];
    actions.extend(champions.iter().map(|champion| ActionAffordance {
        action: "submit_move_intent".to_string(),
        enabled: champion.status == "active",
        target_id: Some(champion.champion_id.clone()),
        disabled_reason: (champion.status != "active").then(|| champion.status.clone()),
    }));
    actions.extend(towns.iter().flat_map(|town| {
        [
            ActionAffordance {
                action: "submit_build_town_structure".to_string(),
                enabled: town.town.status == "active",
                target_id: Some(town.town.town_id.clone()),
                disabled_reason: (town.town.status != "active").then(|| town.town.status.clone()),
            },
            ActionAffordance {
                action: "submit_recruit_units".to_string(),
                enabled: town.town.status == "active",
                target_id: Some(town.town.town_id.clone()),
                disabled_reason: (town.town.status != "active").then(|| town.town.status.clone()),
            },
        ]
    }));
    actions
}

pub(crate) fn map_page_info(page: &MapChunkPage, limit: u32) -> PageInfo {
    PageInfo {
        next_cursor: page.next_cursor,
        has_more: page.has_more,
        limit,
    }
}

pub(crate) fn object_page_info(page: &ObjectViewPage, limit: u32) -> PageInfo {
    PageInfo {
        next_cursor: page.next_cursor,
        has_more: page.has_more,
        limit,
    }
}

fn champion_view(
    _context: &SessionCallerContext,
    champion: Champion,
    own: bool,
) -> Result<ChampionView, ApiError> {
    let stacks = champion_stacks(&champion)?;
    let artifacts = if own {
        equipped_artifacts(champion.id())?
    } else {
        Vec::new()
    };
    Ok(ChampionView {
        champion_id: champion.id().to_string(),
        owner_participant_id: Id::<GameParticipant>::from_key(champion.participant_id).to_string(),
        name: Some(champion.name),
        class_def_id: format!("class:{}", champion.class_key),
        class_key: champion.class_key,
        status: champion.status,
        x: champion.x,
        y: champion.y,
        effective_movement: champion.movement_remaining,
        movement_max: champion.movement_max,
        mana: champion.mana,
        mana_max: champion.mana_max,
        skill_points: champion.skill_points,
        skill_keys: if own {
            champion.skill_keys.clone()
        } else {
            Vec::new()
        },
        spell_slugs: Vec::new(),
        vision_radius: champion.vision_radius,
        strength_label: strength_label(&stacks),
        army_stacks: stacks,
        artifacts,
        redacted: !own,
    })
}

fn champion_stacks(champion: &Champion) -> Result<Vec<ChampionArmyStackRecord>, ApiError> {
    let champion_id = champion.id();
    let page = champions_artifacts::page_champion_army_stacks(
        champion_id,
        u32::from(domm_game::MAX_ARMY_SLOTS),
        None,
    )?;
    page.items
        .into_iter()
        .map(|stack| {
            let unit_slug = known_champion_unit_slug(champion, stack.slot_index).map_or_else(
                || {
                    content::load_unit(Id::from_key(stack.unit_id))?
                        .map(|unit| unit.slug)
                        .ok_or_else(|| {
                            public_error(
                                "unit_not_found",
                                "champion stack unit was not found",
                                false,
                            )
                        })
                },
                Ok,
            )?;
            Ok(ChampionArmyStackRecord {
                stack_id: stack.id().to_string(),
                session_id: Id::<GameSession>::from_key(stack.session_id).to_string(),
                champion_id: champion_id.to_string(),
                unit_slug,
                slot_index: stack.slot_index,
                quantity: stack.quantity,
                front_hp: stack.front_hp,
                status: stack.status,
                last_command_id: stack.last_command_id.map(|id| {
                    Id::<domm_degens_schema::schema::GameCommand>::from_key(id).to_string()
                }),
            })
        })
        .collect()
}

fn known_champion_unit_slug(champion: &Champion, slot_index: u8) -> Option<String> {
    domm_game::first_playable_scenario()
        .starts
        .into_iter()
        .find(|start| {
            start.champion_name == champion.name && start.champion_class_slug == champion.class_key
        })?
        .starting_army_stacks
        .into_iter()
        .find(|stack| stack.slot_index == slot_index)
        .map(|stack| stack.unit_slug)
}

fn equipped_artifacts(champion_id: Id<Champion>) -> Result<Vec<ArtifactView>, ApiError> {
    let mut artifacts = Vec::new();
    for slot in ["banner"] {
        let Some(equipment) =
            champions_artifacts::find_equipment_by_champion_slot(champion_id, slot)?
        else {
            continue;
        };
        let artifact = champions_artifacts::load_artifact_instance(
            Id::<ArtifactInstance>::from_key(equipment.artifact_id),
        )?
        .ok_or_else(|| {
            public_error(
                "artifact_not_found",
                "equipped artifact was not found",
                false,
            )
        })?;
        let artifact_def = content::load_artifact(Id::from_key(artifact.artifact_def_id))?
            .ok_or_else(|| {
                public_error(
                    "artifact_not_found",
                    "artifact definition was not found",
                    false,
                )
            })?;
        artifacts.push(ArtifactView {
            artifact_id: artifact.id().to_string(),
            artifact_def_id: format!("artifact:{}", artifact_def.slug),
            slot: equipment.slot,
            state: artifact.state,
        });
    }
    Ok(artifacts)
}

pub(crate) fn town_view(town: &Town) -> Result<ApiTownView, ApiError> {
    let faction_slug = town_faction_slug(town);
    let buildings = towns::page_town_buildings(town.id(), 16, None)?
        .items
        .into_iter()
        .map(|row| {
            Ok(TownBuildingRecord {
                building_id: row.id().to_string(),
                session_id: Id::<GameSession>::from_key(row.session_id).to_string(),
                town_id: town.id().to_string(),
                building_slug: row.building_slug,
                built_turn: row.built_turn,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let recruit_pools = towns::page_town_recruit_pools(town.id(), 16, None)?
        .items
        .into_iter()
        .map(|row| {
            Ok(TownRecruitPoolRecord {
                pool_id: row.id().to_string(),
                session_id: Id::<GameSession>::from_key(row.session_id).to_string(),
                town_id: town.id().to_string(),
                unit_slug: row.unit_slug,
                available: row.available,
                last_growth_week: row.last_growth_week,
                last_command_id: row.last_command_id.map(|id| {
                    Id::<domm_degens_schema::schema::GameCommand>::from_key(id).to_string()
                }),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let garrison_stacks =
        towns::page_town_garrison(town.id(), u32::from(domm_game::MAX_ARMY_SLOTS), None)?
            .items
            .into_iter()
            .map(|row| {
                Ok(domm_game::ArmyStackRecord {
                    stack_id: row.id().to_string(),
                    session_id: Id::<GameSession>::from_key(row.session_id).to_string(),
                    owner_kind: "town".to_string(),
                    owner_id: town.id().to_string(),
                    unit_slug: row.unit_slug,
                    slot_index: row.slot_index,
                    quantity: row.quantity,
                    front_hp: row.front_hp,
                    status: "active".to_string(),
                    last_command_id: row.last_command_id.map(|id| {
                        Id::<domm_degens_schema::schema::GameCommand>::from_key(id).to_string()
                    }),
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(ApiTownView {
        town: TownRecord {
            town_id: town.id().to_string(),
            session_id: Id::<GameSession>::from_key(town.session_id).to_string(),
            owner_participant_id: town
                .owner_participant_id
                .map(|id| Id::<GameParticipant>::from_key(id).to_string())
                .unwrap_or_default(),
            faction_slug,
            name: town.name.clone(),
            x: town.x,
            y: town.y,
            status: town.status.clone(),
            hall_level: town.hall_level,
            fort_level: town.fort_level,
            last_built_turn: town.last_built_turn,
            captured_turn: town.captured_turn,
            income_started_turn: town.income_started_turn,
            unrest_until_turn: town.unrest_until_turn,
            last_command_id: town
                .last_command_id
                .map(|id| Id::<domm_degens_schema::schema::GameCommand>::from_key(id).to_string()),
        },
        buildings,
        recruit_pools,
        garrison_stacks,
    })
}

fn town_faction_slug(town: &Town) -> String {
    domm_game::first_playable_scenario()
        .starts
        .iter()
        .find(|start| start.town_x == town.x && start.town_y == town.y)
        .map(|start| start.faction_slug.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn object_view_from_known(
    known: &domm_degens_schema::schema::ParticipantKnownObject,
    participants_by_slot: &BTreeMap<u8, String>,
) -> ObjectView {
    let visible = known.visibility == "visible";
    let details = scenario_subject_details(
        &known.subject_kind,
        &known.subject_id_text,
        participants_by_slot,
    );
    ObjectView {
        subject_kind: known.subject_kind.clone(),
        subject_id_text: known.subject_id_text.clone(),
        visibility: known.visibility.clone(),
        redaction_level: if visible { "none" } else { "last_known" }.to_string(),
        x: known.x,
        y: known.y,
        last_seen_turn: Some(known.last_seen_turn),
        display_name: visible.then(|| details.display_name.clone()),
        asset_key: details.asset_key,
        owner_participant_id: visible.then_some(details.owner_participant_id).flatten(),
        details_json: if visible {
            details.public_json
        } else {
            known.redacted_json.clone().unwrap_or(details.redacted_json)
        },
    }
}

struct SubjectDetails {
    display_name: String,
    asset_key: Option<String>,
    owner_participant_id: Option<String>,
    public_json: String,
    redacted_json: String,
}

fn scenario_subject_details(
    subject_kind: &str,
    subject_id_text: &str,
    participants_by_slot: &BTreeMap<u8, String>,
) -> SubjectDetails {
    let scenario = domm_game::first_playable_scenario();
    let manifest = domm_game::first_playable_content_manifest();
    if subject_kind == "town" {
        if let Some(start) = scenario
            .starts
            .iter()
            .find(|start| start.town_key == subject_id_text)
        {
            return SubjectDetails {
                display_name: start.town_name.clone(),
                asset_key: Some(format!("sprite:town:{}", start.faction_slug)),
                owner_participant_id: participants_by_slot.get(&start.slot_index).cloned(),
                public_json: format!(
                    "{{\"type\":\"town\",\"scenario_key\":\"{}\",\"status\":\"active\"}}",
                    escape_json(subject_id_text)
                ),
                redacted_json: format!(
                    "{{\"type\":\"town\",\"scenario_key\":\"{}\",\"status\":\"last_known\"}}",
                    escape_json(subject_id_text)
                ),
            };
        }
    }
    if subject_kind == "champion" {
        if let Some(start) = scenario
            .starts
            .iter()
            .find(|start| start.champion_key == subject_id_text)
        {
            return SubjectDetails {
                display_name: start.champion_name.clone(),
                asset_key: Some(format!("sprite:champion:{}", start.champion_class_slug)),
                owner_participant_id: participants_by_slot.get(&start.slot_index).cloned(),
                public_json: format!(
                    "{{\"type\":\"champion\",\"scenario_key\":\"{}\",\"class_key\":\"{}\",\"status\":\"active\"}}",
                    escape_json(subject_id_text),
                    escape_json(&start.champion_class_slug)
                ),
                redacted_json: format!(
                    "{{\"type\":\"champion\",\"scenario_key\":\"{}\",\"status\":\"last_known\"}}",
                    escape_json(subject_id_text)
                ),
            };
        }
    }
    if subject_kind == "neutral_army" {
        if let Some(neutral) = scenario
            .neutral_armies
            .iter()
            .find(|neutral| neutral.key == subject_id_text)
        {
            return SubjectDetails {
                display_name: format!("Neutral Guard ({})", neutral.strength_band),
                asset_key: Some("sprite:unit:broken-pike".to_string()),
                owner_participant_id: None,
                public_json: format!(
                    "{{\"type\":\"neutral_army\",\"scenario_key\":\"{}\",\"strength_label\":\"{}\",\"stack_count\":{}}}",
                    escape_json(subject_id_text),
                    escape_json(&neutral.strength_band),
                    neutral.stacks.len()
                ),
                redacted_json: format!(
                    "{{\"type\":\"neutral_army\",\"scenario_key\":\"{}\",\"strength_label\":\"{}\"}}",
                    escape_json(subject_id_text),
                    escape_json(&neutral.strength_band)
                ),
            };
        }
    }
    let object = scenario
        .mines
        .iter()
        .chain(scenario.external_dwellings.iter())
        .chain(scenario.central_objectives.iter())
        .find(|object| object.key == subject_id_text)
        .map(|object| (object.object_slug.clone(), None))
        .or_else(|| {
            scenario
                .resource_piles
                .iter()
                .find(|pile| pile.key == subject_id_text)
                .map(|pile| (pile.object_slug.clone(), Some(pile.reward.clone())))
        });
    if let Some((slug, reward)) = object {
        let definition = manifest.map_object(&slug);
        let object_type = definition
            .map(|definition| definition.object_type.as_str())
            .unwrap_or("world_object");
        return SubjectDetails {
            display_name: definition
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| slug.clone()),
            asset_key: definition.and_then(|definition| definition.sprite_key.clone()),
            owner_participant_id: None,
            public_json: world_object_details_json(
                subject_id_text,
                &slug,
                object_type,
                reward.as_ref(),
            ),
            redacted_json: format!(
                "{{\"type\":\"world_object\",\"scenario_key\":\"{}\",\"object_type\":\"{}\",\"state\":\"last_known\"}}",
                escape_json(subject_id_text),
                escape_json(object_type)
            ),
        };
    }

    SubjectDetails {
        display_name: subject_id_text.to_string(),
        asset_key: None,
        owner_participant_id: None,
        public_json: "{}".to_string(),
        redacted_json: "{}".to_string(),
    }
}

fn world_object_details_json(
    scenario_key: &str,
    slug: &str,
    object_type: &str,
    reward: Option<&domm_game::ResourceCost>,
) -> String {
    match reward {
        Some(reward) => format!(
            "{{\"type\":\"world_object\",\"scenario_key\":\"{}\",\"object_slug\":\"{}\",\"object_type\":\"{}\",\"state\":\"available\",\"reward\":{{\"gold\":{},\"wood\":{},\"stone\":{},\"iron\":{},\"crystal\":{},\"ember\":{},\"aether\":{}}}}}",
            escape_json(scenario_key),
            escape_json(slug),
            escape_json(object_type),
            reward.gold,
            reward.wood,
            reward.stone,
            reward.iron,
            reward.crystal,
            reward.ember,
            reward.aether
        ),
        None => format!(
            "{{\"type\":\"world_object\",\"scenario_key\":\"{}\",\"object_slug\":\"{}\",\"object_type\":\"{}\",\"state\":\"available\"}}",
            escape_json(scenario_key),
            escape_json(slug),
            escape_json(object_type)
        ),
    }
}

fn resolve_champion(session: &GameSession, champion_id: &str) -> Result<Champion, ApiError> {
    if let Ok(id) = Ulid::from_str(champion_id).map(Id::<Champion>::from_key) {
        return champions_artifacts::load_champion(id)?
            .ok_or_else(|| public_error("not_found", "champion not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.champion_key == champion_id)
        .ok_or_else(|| public_error("not_found", "champion not found", false))?;
    champions_artifacts::find_champion_by_session_xy(
        session.id(),
        start.champion_x,
        start.champion_y,
    )?
    .ok_or_else(|| public_error("not_found", "champion not found", false))
}

pub(crate) fn is_visible_at(
    session: &GameSession,
    participant_id: Id<GameParticipant>,
    x: u16,
    y: u16,
) -> Result<bool, ApiError> {
    let chunk_size = u16::from(session.chunk_size);
    let chunk_x = x / chunk_size;
    let chunk_y = y / chunk_size;
    let Some(visibility) =
        map_visibility_occupancy::find_visibility_chunk(participant_id, chunk_x, chunk_y)?
    else {
        return Ok(false);
    };
    let width = chunk_width(session, chunk_x);
    let local_x = x % chunk_size;
    let local_y = y % chunk_size;
    let index = usize::from(local_y) * usize::from(width) + usize::from(local_x);
    Ok(domm_game::read_visibility_bit(
        visibility.visible_blob.as_slice(),
        index,
    ))
}

fn chunk_intersects_viewport(
    session: &GameSession,
    viewport: &Viewport,
    chunk: &domm_degens_schema::schema::MapChunk,
) -> bool {
    let chunk_size = u16::from(session.chunk_size);
    let chunk_min_x = chunk.chunk_x * chunk_size;
    let chunk_min_y = chunk.chunk_y * chunk_size;
    let chunk_max_x = chunk_min_x.saturating_add(u16::from(chunk.width));
    let chunk_max_y = chunk_min_y.saturating_add(u16::from(chunk.height));
    let view_max_x = viewport.x.saturating_add(viewport.width);
    let view_max_y = viewport.y.saturating_add(viewport.height);
    viewport.x < chunk_max_x
        && view_max_x > chunk_min_x
        && viewport.y < chunk_max_y
        && view_max_y > chunk_min_y
}

fn chunk_width(session: &GameSession, chunk_x: u16) -> u16 {
    let chunk_size = u16::from(session.chunk_size);
    let origin_x = chunk_x * chunk_size;
    session.map_width.saturating_sub(origin_x).min(chunk_size)
}

fn strength_label(stacks: &[ChampionArmyStackRecord]) -> String {
    let total = stacks.iter().map(|stack| stack.quantity).sum::<u32>();
    match total {
        0 => "none",
        1..=20 => "small",
        21..=60 => "modest",
        _ => "large",
    }
    .to_string()
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
