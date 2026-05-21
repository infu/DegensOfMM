use std::{cell::RefCell, collections::BTreeMap};

use domm_degens_schema::schema::{
    ArtifactInstance, Champion, ChampionArmyStack, GameParticipant, GameSession, MapChunk,
    NeutralArmy, ParticipantKnownObject, SpellDefinition, Town, VisibilityChunk, WorldObject,
};
use domm_game::{
    ActionAffordance, ApiError, ApiTownView, ArtifactView, ChampionArmyStackRecord, ChampionView,
    MapChunkPage, MapChunkView, ObjectView, ObjectViewPage, TownBuildingRecord, TownRecord,
    TownRecruitPoolRecord, Viewport,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Ulid},
};

use crate::repos::{
    champions_artifacts, content, foundation, map_visibility_occupancy, neutrals, towns,
};

use super::{
    session_context::{SessionCallerContext, public_error},
    session_turn_runtime, town_runtime,
};

const MAX_OWNED_CHAMPIONS_VIEW: u32 = 16;

thread_local! {
    static CHAMPION_STACK_ROWS: RefCell<Vec<(String, Vec<ChampionArmyStack>)>> =
        const { RefCell::new(Vec::new()) };
    static CHAMPION_BANNER_ARTIFACT_IDS: RefCell<Vec<(String, String)>> =
        const { RefCell::new(Vec::new()) };
    static KNOWN_OBJECT_ROWS: RefCell<Vec<ParticipantKnownObject>> =
        const { RefCell::new(Vec::new()) };
    static NEUTRAL_ARMY_ROWS: RefCell<Vec<NeutralArmy>> =
        const { RefCell::new(Vec::new()) };
    static MAP_CHUNK_ROWS: RefCell<Vec<MapChunk>> =
        const { RefCell::new(Vec::new()) };
    static VISIBILITY_CHUNK_ROWS: RefCell<Vec<VisibilityChunk>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) fn remember_champion_stack_rows(
    champion_id: Id<Champion>,
    rows: Vec<ChampionArmyStack>,
) {
    if rows.is_empty() {
        return;
    }
    let key = champion_id.to_string();
    CHAMPION_STACK_ROWS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
        cache.push((key, rows));
    });
}

pub(crate) fn remember_champion_banner_artifact(champion_id: Id<Champion>, artifact_id: String) {
    let key = champion_id.to_string();
    CHAMPION_BANNER_ARTIFACT_IDS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
        cache.push((key, artifact_id));
    });
}

pub(crate) fn invalidate_champion_detail(champion_id: Id<Champion>) {
    let key = champion_id.to_string();
    CHAMPION_STACK_ROWS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
    });
    CHAMPION_BANNER_ARTIFACT_IDS.with_borrow_mut(|cache| {
        cache.retain(|(existing, _)| existing != &key);
    });
}

pub(crate) fn remember_known_objects(rows: &[ParticipantKnownObject]) {
    KNOWN_OBJECT_ROWS.with_borrow_mut(|cache| {
        cache.extend(rows.iter().cloned());
    });
}

pub(crate) fn remember_known_object(row: &ParticipantKnownObject) {
    KNOWN_OBJECT_ROWS.with_borrow_mut(|cache| {
        cache.push(row.clone());
    });
}

pub(crate) fn known_object_cache_loaded(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
) -> bool {
    let session_key = session_id.key();
    let participant_key = participant_id.key();
    KNOWN_OBJECT_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .any(|row| row.session_id == session_key && row.participant_id == participant_key)
    })
}

pub(crate) fn cached_known_object(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    subject_kind: &str,
    subject_id_text: &str,
) -> Option<ParticipantKnownObject> {
    let session_key = session_id.key();
    let participant_key = participant_id.key();
    KNOWN_OBJECT_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .rev()
            .find(|row| {
                row.session_id == session_key
                    && row.participant_id == participant_key
                    && row.subject_kind == subject_kind
                    && row.subject_id_text == subject_id_text
            })
            .cloned()
    })
}

pub(crate) fn remember_known_object_if_cached(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    row: &ParticipantKnownObject,
) {
    let session_key = session_id.key();
    let participant_key = participant_id.key();
    KNOWN_OBJECT_ROWS.with_borrow_mut(|cache| {
        if cache
            .iter()
            .any(|row| row.session_id == session_key && row.participant_id == participant_key)
        {
            cache.push(row.clone());
        }
    });
}

pub(crate) fn remember_neutral_armies(rows: &[NeutralArmy]) {
    NEUTRAL_ARMY_ROWS.with_borrow_mut(|cache| {
        for row in rows {
            cache.retain(|existing| existing.id() != row.id());
            cache.push(row.clone());
        }
    });
}

pub(crate) fn remember_map_chunks(rows: &[MapChunk]) {
    MAP_CHUNK_ROWS.with_borrow_mut(|cache| {
        cache.extend(rows.iter().cloned());
    });
}

pub(crate) fn remember_visibility_chunks(rows: &[VisibilityChunk]) {
    VISIBILITY_CHUNK_ROWS.with_borrow_mut(|cache| {
        cache.extend(rows.iter().cloned());
    });
}

pub(crate) fn remember_visibility_chunk(row: &VisibilityChunk) {
    VISIBILITY_CHUNK_ROWS.with_borrow_mut(|cache| {
        if let Some(existing) = cache.iter_mut().find(|existing| existing.id() == row.id()) {
            *existing = row.clone();
        } else {
            cache.push(row.clone());
        }
    });
}

pub(crate) fn cached_visibility_chunk(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
    chunk_x: u16,
    chunk_y: u16,
) -> Option<VisibilityChunk> {
    let session_key = session_id.key();
    let participant_key = participant_id.key();
    VISIBILITY_CHUNK_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .rev()
            .find(|row| {
                row.session_id == session_key
                    && row.participant_id == participant_key
                    && row.chunk_x == chunk_x
                    && row.chunk_y == chunk_y
            })
            .cloned()
    })
}

pub(crate) fn invalidate_visibility_chunks(
    session_id: Id<GameSession>,
    participant_id: Id<GameParticipant>,
) {
    let session_key = session_id.key();
    let participant_key = participant_id.key();
    VISIBILITY_CHUNK_ROWS.with_borrow_mut(|cache| {
        cache.retain(|row| row.session_id != session_key || row.participant_id != participant_key);
    });
}

fn cached_champion_stack_rows(champion_id: Id<Champion>) -> Option<Vec<ChampionArmyStack>> {
    let key = champion_id.to_string();
    CHAMPION_STACK_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .find(|(existing, _)| existing == &key)
            .map(|(_, rows)| rows.clone())
    })
}

fn cached_champion_banner_artifact(champion_id: Id<Champion>) -> Option<String> {
    let key = champion_id.to_string();
    CHAMPION_BANNER_ARTIFACT_IDS.with_borrow(|cache| {
        cache
            .iter()
            .find(|(existing, _)| existing == &key)
            .map(|(_, artifact_id)| artifact_id.clone())
    })
}

fn cached_known_objects(context: &SessionCallerContext) -> Vec<ParticipantKnownObject> {
    KNOWN_OBJECT_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .filter(|row| {
                row.session_id == context.session.id().key()
                    && row.participant_id == context.participant.id().key()
            })
            .cloned()
            .collect()
    })
}

fn cached_map_chunks(context: &SessionCallerContext) -> Vec<MapChunk> {
    MAP_CHUNK_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .filter(|row| row.session_id == context.session.id().key())
            .cloned()
            .collect()
    })
}

fn cached_neutral_for_known(
    session_id: Id<GameSession>,
    subject: &ObjectSubject,
) -> Option<NeutralArmy> {
    let session_key = session_id.key();
    let id_key = Ulid::from_str(&subject.subject_id_text).ok();
    NEUTRAL_ARMY_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .find(|row| {
                row.session_id == session_key
                    && (id_key.is_some_and(|id| row.id == id)
                        || row.x == subject.x && row.y == subject.y)
            })
            .cloned()
    })
}

fn cached_visibility_chunks(context: &SessionCallerContext) -> Vec<VisibilityChunk> {
    VISIBILITY_CHUNK_ROWS.with_borrow(|cache| {
        cache
            .iter()
            .filter(|row| {
                row.session_id == context.session.id().key()
                    && row.participant_id == context.participant.id().key()
            })
            .cloned()
            .collect()
    })
}

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
    let mut visibility_rows = cached_visibility_chunks(context);
    if visibility_rows.is_empty() {
        visibility_rows = map_visibility_occupancy::page_visibility_chunks_by_participant(
            context.session.id(),
            context.participant.id(),
            domm_game::MAX_LIST_LIMIT,
            None,
        )?
        .items;
    }
    let visibility_by_coord = visibility_rows
        .into_iter()
        .map(|visibility| ((visibility.chunk_x, visibility.chunk_y), visibility))
        .collect::<BTreeMap<_, _>>();
    let mut map_chunks = cached_map_chunks(context);
    if map_chunks.is_empty() {
        map_chunks = map_visibility_occupancy::page_map_chunks_by_session(
            context.session.id(),
            domm_game::MAX_LIST_LIMIT,
            None,
        )?
        .items;
    }
    let mut chunks = map_chunks
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
    let mut objects = Vec::new();
    let live_world_objects_by_coord = live_world_objects_by_coord(context.session.id())?;

    for subject in known_subjects_for_viewport(context, viewport)? {
        if !viewport.contains(subject.x, subject.y) {
            continue;
        }
        if let Some(view) = object_view_from_known_fast(
            context,
            &subject,
            Some(viewport),
            Some(&live_world_objects_by_coord),
            None,
        )? {
            objects.push(view);
        }
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

pub(crate) fn object_view_by_subject(
    context: &SessionCallerContext,
    subject_kind: &str,
    subject_id_text: &str,
) -> Result<ObjectView, ApiError> {
    if !matches!(
        subject_kind,
        "world_object" | "champion" | "town" | "neutral_army"
    ) {
        return Err(public_error(
            "unknown_subject_kind",
            "subject_kind is not supported by get_object_view",
            false,
        ));
    }
    let known = if let Some(known) = cached_known_object(
        context.session.id(),
        context.participant.id(),
        subject_kind,
        subject_id_text,
    ) {
        known
    } else {
        if known_object_cache_loaded(context.session.id(), context.participant.id()) {
            return Err(public_error(
                "not_visible",
                "object is not visible or known to this participant",
                false,
            ));
        }
        let Some(known) = map_visibility_occupancy::find_known_object(
            context.participant.id(),
            subject_kind,
            subject_id_text,
        )?
        else {
            return Err(public_error(
                "not_visible",
                "object is not visible or known to this participant",
                false,
            ));
        };
        known
    };
    if known.visibility == "hidden" {
        return Err(public_error(
            "not_visible",
            "object is not visible or known to this participant",
            false,
        ));
    }
    let subject = ObjectSubject::from_known(&known);
    object_view_from_known_fast(context, &subject, None, None, None)?
        .ok_or_else(|| public_error("not_visible", "object is no longer visible", false))
}

pub(crate) fn my_champions(context: &SessionCallerContext) -> Result<Vec<ChampionView>, ApiError> {
    if !context.participant.champion_ids.is_empty() {
        let mut views = Vec::new();
        for champion_id in context
            .participant
            .champion_ids
            .iter()
            .take(MAX_OWNED_CHAMPIONS_VIEW as usize)
        {
            let champion_id = Id::<Champion>::from_key(*champion_id);
            let champion = match session_turn_runtime::champion_snapshot(
                &context.session.id().to_string(),
                &champion_id.to_string(),
            ) {
                Some(champion) => champion,
                None => {
                    let Some(champion) = champions_artifacts::load_champion(champion_id)? else {
                        continue;
                    };
                    champion
                }
            };
            if champion.session_id != context.session.id().key()
                || champion.participant_id != context.participant.id().key()
                || champion.status != "active"
            {
                continue;
            }
            views.push(champion_summary_view(champion, true));
        }
        return Ok(views);
    }

    let champions = champions_artifacts::list_champions_by_session_owner_status(
        context.session.id(),
        context.participant.id(),
        "active",
        MAX_OWNED_CHAMPIONS_VIEW,
    )?;
    Ok(champions
        .into_iter()
        .map(|champion| champion_summary_view(champion, true))
        .collect())
}

pub(crate) fn champion_view_by_id(
    context: &SessionCallerContext,
    champion_id: &str,
) -> Result<ChampionView, ApiError> {
    if scenario_champion_belongs_to_participant(context, champion_id) {
        for owned_champion_id in &context.participant.champion_ids {
            if let Some(champion) = session_turn_runtime::champion_snapshot(
                &context.session.id().to_string(),
                &Id::<Champion>::from_key(*owned_champion_id).to_string(),
            ) {
                return champion_view(context, champion, true);
            }
        }
    }
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
) -> Vec<ActionAffordance> {
    let mut actions = Vec::new();
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
    let spell_slugs = if own {
        learned_spell_slugs(&champion)?
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
        spell_slugs,
        vision_radius: champion.vision_radius,
        strength_label: strength_label(&stacks),
        army_stacks: stacks,
        artifacts,
        redacted: !own,
    })
}

fn champion_summary_view(champion: Champion, own: bool) -> ChampionView {
    ChampionView {
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
        skill_keys: if own { champion.skill_keys } else { Vec::new() },
        spell_slugs: Vec::new(),
        vision_radius: champion.vision_radius,
        strength_label: "details_required".to_string(),
        army_stacks: Vec::new(),
        artifacts: Vec::new(),
        redacted: !own,
    }
}

fn champion_stacks(champion: &Champion) -> Result<Vec<ChampionArmyStackRecord>, ApiError> {
    let champion_id = champion.id();
    let rows = match cached_champion_stack_rows(champion_id) {
        Some(rows) => rows,
        None => {
            let rows = champions_artifacts::list_champion_army_stacks(
                champion_id,
                u32::from(domm_game::MAX_ARMY_SLOTS),
            )?;
            remember_champion_stack_rows(champion_id, rows.clone());
            rows
        }
    };
    rows.into_iter()
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
    if let Some(artifact_id) = cached_champion_banner_artifact(champion_id) {
        return Ok(vec![ArtifactView {
            artifact_id,
            artifact_def_id: "artifact:bent-banner".to_string(),
            slot: "banner".to_string(),
            state: "equipped".to_string(),
        }]);
    }
    let mut artifacts = Vec::new();
    for slot in ["banner"] {
        let Some(equipment) =
            champions_artifacts::find_equipment_by_champion_slot(champion_id, slot)?
        else {
            continue;
        };
        let artifact_id = Id::<ArtifactInstance>::from_key(equipment.artifact_id).to_string();
        remember_champion_banner_artifact(champion_id, artifact_id.clone());
        artifacts.push(ArtifactView {
            artifact_id,
            artifact_def_id: "artifact:bent-banner".to_string(),
            slot: equipment.slot,
            state: "equipped".to_string(),
        });
    }
    Ok(artifacts)
}

fn learned_spell_slugs(champion: &Champion) -> Result<Vec<String>, ApiError> {
    if !champion.skill_keys.iter().any(|key| key == "sour_sorcery") {
        return Ok(Vec::new());
    }
    let champion_id = champion.id();
    let page = champions_artifacts::page_champion_spells(
        champion_id,
        domm_game::CHAMPION_SPELLBOOK_CAP as u32,
        None,
    )?;
    let mut slugs = Vec::new();
    for known in page.items {
        if let Some(slug) = known.spell_slug.as_deref().filter(|slug| !slug.is_empty()) {
            slugs.push(slug.to_string());
            continue;
        }
        let spell = content::load_spell(Id::<SpellDefinition>::from_key(known.spell_id))?
            .ok_or_else(|| {
                public_error(
                    "spell_not_found",
                    "known spell definition was not found",
                    false,
                )
            })?;
        slugs.push(spell.slug);
    }
    slugs.sort();
    Ok(slugs)
}

pub(crate) fn town_view(town: &Town) -> Result<ApiTownView, ApiError> {
    let projection = town_runtime::projection_for_town(town)?;
    town_view_from_parts(
        &projection.town,
        projection.buildings,
        projection.recruit_pools,
        projection.garrison_stacks,
    )
}

fn town_view_from_parts(
    town: &Town,
    building_rows: Vec<domm_degens_schema::schema::TownBuilding>,
    recruit_pool_rows: Vec<domm_degens_schema::schema::TownRecruitPool>,
    garrison_rows: Vec<domm_degens_schema::schema::TownGarrisonStack>,
) -> Result<ApiTownView, ApiError> {
    let faction_slug = town_faction_slug(town);
    let buildings = building_rows
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
    let recruit_pools = recruit_pool_rows
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
    let garrison_stacks = garrison_rows
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

struct ObjectSubject {
    subject_kind: String,
    subject_id_text: String,
    x: u16,
    y: u16,
    last_seen_turn: u32,
    redacted_json: Option<String>,
}

impl ObjectSubject {
    fn from_known(known: &ParticipantKnownObject) -> Self {
        Self {
            subject_kind: known.subject_kind.clone(),
            subject_id_text: known.subject_id_text.clone(),
            x: known.x,
            y: known.y,
            last_seen_turn: known.last_seen_turn,
            redacted_json: known.redacted_json.clone(),
        }
    }
}

fn known_subjects_for_viewport(
    context: &SessionCallerContext,
    viewport: &Viewport,
) -> Result<Vec<ObjectSubject>, ApiError> {
    let mut subjects = Vec::new();
    let mut rows = cached_known_objects(context);
    if rows.is_empty() {
        rows = map_visibility_occupancy::page_known_objects_for_participant(
            context.session.id(),
            context.participant.id(),
            domm_game::MAX_LIST_LIMIT,
            None,
        )?
        .items;
    }
    for known in rows {
        if known.visibility == "hidden" || !viewport.contains(known.x, known.y) {
            continue;
        }
        subjects.push(ObjectSubject::from_known(&known));
    }
    subjects.sort_by_key(|subject| {
        (
            subject.y,
            subject.x,
            subject.subject_kind.clone(),
            subject.subject_id_text.clone(),
        )
    });
    subjects.dedup_by(|left, right| {
        left.subject_kind == right.subject_kind && left.subject_id_text == right.subject_id_text
    });
    Ok(subjects)
}

fn object_view_from_known_fast(
    context: &SessionCallerContext,
    subject: &ObjectSubject,
    viewport: Option<&Viewport>,
    live_world_objects_by_coord: Option<&BTreeMap<(u16, u16), WorldObject>>,
    visibility_by_coord: Option<&BTreeMap<(u16, u16), VisibilityChunk>>,
) -> Result<Option<ObjectView>, ApiError> {
    match subject.subject_kind.as_str() {
        "world_object" => {
            if viewport.is_some() {
                return world_object_list_view(
                    context,
                    subject,
                    live_world_objects_by_coord,
                    visibility_by_coord,
                );
            }
            let Some(object) = live_world_object_for_known(context.session.id(), subject)? else {
                return Ok(None);
            };
            if object.state == "collected" {
                return Ok(None);
            }
            if viewport.is_some_and(|viewport| !viewport.contains(object.x, object.y)) {
                return Ok(None);
            }
            let object_slug = json_string_field(object.instance_json.as_deref(), "object_slug")
                .unwrap_or_else(|| subject.subject_id_text.clone());
            return Ok(Some(ObjectView {
                subject_kind: subject.subject_kind.clone(),
                subject_id_text: subject.subject_id_text.clone(),
                visibility: "visible".to_string(),
                redaction_level: "none".to_string(),
                x: object.x,
                y: object.y,
                last_seen_turn: Some(context.session.current_turn),
                display_name: Some(object_slug),
                asset_key: None,
                owner_participant_id: object
                    .owner_participant_id
                    .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
                details_json: world_object_live_details_json(&subject.subject_id_text, &object),
            }));
        }
        "neutral_army" => {
            if viewport.is_some() {
                let Some(neutral) = live_neutral_for_known(context.session.id(), subject)? else {
                    return Ok(None);
                };
                if neutral.state == "defeated" {
                    return Ok(None);
                }
                if visibility_by_coord.is_some()
                    && !is_visible_for_projection(
                        context,
                        visibility_by_coord,
                        neutral.x,
                        neutral.y,
                    )?
                {
                    return Ok(None);
                }
                if viewport.is_some_and(|viewport| !viewport.contains(neutral.x, neutral.y)) {
                    return Ok(None);
                }
                let details = scenario_subject_details(
                    &subject.subject_kind,
                    &subject.subject_id_text,
                    &BTreeMap::new(),
                );
                return Ok(Some(ObjectView {
                    subject_kind: subject.subject_kind.clone(),
                    subject_id_text: subject.subject_id_text.clone(),
                    visibility: "visible".to_string(),
                    redaction_level: "none".to_string(),
                    x: neutral.x,
                    y: neutral.y,
                    last_seen_turn: Some(context.session.current_turn),
                    display_name: Some(details.display_name),
                    asset_key: details.asset_key,
                    owner_participant_id: None,
                    details_json: format!(
                        "{{\"type\":\"neutral_army\",\"scenario_key\":\"{}\",\"neutral_army_id\":\"{}\",\"state\":\"{}\"}}",
                        escape_json(&subject.subject_id_text),
                        neutral.id(),
                        escape_json(&neutral.state)
                    ),
                }));
            }
            let Some(neutral) = live_neutral_for_known(context.session.id(), subject)? else {
                return Ok(None);
            };
            if neutral.state == "defeated" {
                return Ok(None);
            }
            if visibility_by_coord.is_some()
                && !is_visible_for_projection(context, visibility_by_coord, neutral.x, neutral.y)?
            {
                return Ok(last_known_object_view_fast(subject, viewport));
            }
            if viewport.is_some_and(|viewport| !viewport.contains(neutral.x, neutral.y)) {
                return Ok(None);
            }
            return Ok(Some(ObjectView {
                subject_kind: subject.subject_kind.clone(),
                subject_id_text: subject.subject_id_text.clone(),
                visibility: "visible".to_string(),
                redaction_level: "none".to_string(),
                x: neutral.x,
                y: neutral.y,
                last_seen_turn: Some(context.session.current_turn),
                display_name: Some("Neutral Army".to_string()),
                asset_key: None,
                owner_participant_id: None,
                details_json: format!(
                    "{{\"type\":\"neutral_army\",\"scenario_key\":\"{}\",\"neutral_army_id\":\"{}\",\"state\":\"{}\"}}",
                    escape_json(&subject.subject_id_text),
                    neutral.id(),
                    escape_json(&neutral.state)
                ),
            }));
        }
        "champion" => {
            if viewport.is_some() {
                if let Some(champion) = fast_champion_for_known(context, subject)? {
                    if !matches!(champion.status.as_str(), "active" | "in_battle") {
                        return Ok(None);
                    }
                    if viewport.is_some_and(|viewport| !viewport.contains(champion.x, champion.y)) {
                        return Ok(None);
                    }
                    return Ok(Some(ObjectView {
                        subject_kind: subject.subject_kind.clone(),
                        subject_id_text: subject.subject_id_text.clone(),
                        visibility: "visible".to_string(),
                        redaction_level: "none".to_string(),
                        x: champion.x,
                        y: champion.y,
                        last_seen_turn: Some(context.session.current_turn),
                        display_name: Some(champion.name.clone()),
                        asset_key: None,
                        owner_participant_id: Some(
                            Id::<GameParticipant>::from_key(champion.participant_id).to_string(),
                        ),
                        details_json: format!(
                            "{{\"type\":\"champion\",\"scenario_key\":\"{}\",\"champion_id\":\"{}\",\"class_key\":\"{}\",\"status\":\"{}\"}}",
                            escape_json(&subject.subject_id_text),
                            champion.id(),
                            escape_json(&champion.class_key),
                            escape_json(&champion.status)
                        ),
                    }));
                }
                let details = scenario_subject_details(
                    &subject.subject_kind,
                    &subject.subject_id_text,
                    &BTreeMap::new(),
                );
                return Ok(Some(ObjectView {
                    subject_kind: subject.subject_kind.clone(),
                    subject_id_text: subject.subject_id_text.clone(),
                    visibility: "visible".to_string(),
                    redaction_level: "none".to_string(),
                    x: subject.x,
                    y: subject.y,
                    last_seen_turn: Some(context.session.current_turn),
                    display_name: Some(details.display_name),
                    asset_key: details.asset_key,
                    owner_participant_id: scenario_champion_belongs_to_participant(
                        context,
                        &subject.subject_id_text,
                    )
                    .then(|| context.participant.id().to_string()),
                    details_json: format!(
                        "{{\"type\":\"champion\",\"scenario_key\":\"{}\",\"status\":\"active\"}}",
                        escape_json(&subject.subject_id_text)
                    ),
                }));
            }
            let Some(champion) = fast_champion_for_known(context, subject)? else {
                return Ok(last_known_object_view_fast(subject, viewport));
            };
            if !matches!(champion.status.as_str(), "active" | "in_battle") {
                return Ok(None);
            }
            if viewport.is_some_and(|viewport| !viewport.contains(champion.x, champion.y)) {
                return Ok(None);
            }
            return Ok(Some(ObjectView {
                subject_kind: subject.subject_kind.clone(),
                subject_id_text: subject.subject_id_text.clone(),
                visibility: "visible".to_string(),
                redaction_level: "none".to_string(),
                x: champion.x,
                y: champion.y,
                last_seen_turn: Some(context.session.current_turn),
                display_name: Some(champion.name.clone()),
                asset_key: None,
                owner_participant_id: Some(
                    Id::<GameParticipant>::from_key(champion.participant_id).to_string(),
                ),
                details_json: format!(
                    "{{\"type\":\"champion\",\"scenario_key\":\"{}\",\"champion_id\":\"{}\",\"class_key\":\"{}\",\"status\":\"{}\"}}",
                    escape_json(&subject.subject_id_text),
                    champion.id(),
                    escape_json(&champion.class_key),
                    escape_json(&champion.status)
                ),
            }));
        }
        "town" => {
            if viewport.is_some() {
                if scenario_town_belongs_to_participant(context, &subject.subject_id_text) {
                    let details = scenario_subject_details(
                        &subject.subject_kind,
                        &subject.subject_id_text,
                        &BTreeMap::new(),
                    );
                    return Ok(Some(ObjectView {
                        subject_kind: subject.subject_kind.clone(),
                        subject_id_text: subject.subject_id_text.clone(),
                        visibility: "visible".to_string(),
                        redaction_level: "none".to_string(),
                        x: subject.x,
                        y: subject.y,
                        last_seen_turn: Some(context.session.current_turn),
                        display_name: Some(details.display_name),
                        asset_key: details.asset_key,
                        owner_participant_id: Some(context.participant.id().to_string()),
                        details_json: format!(
                            "{{\"type\":\"town\",\"scenario_key\":\"{}\",\"status\":\"active\"}}",
                            escape_json(&subject.subject_id_text)
                        ),
                    }));
                }
                if let Some(town) = live_town_for_known(context.session.id(), subject)? {
                    if town.status != "active" {
                        return Ok(None);
                    }
                    if viewport.is_some_and(|viewport| !viewport.contains(town.x, town.y)) {
                        return Ok(None);
                    }
                    let details = scenario_subject_details(
                        &subject.subject_kind,
                        &subject.subject_id_text,
                        &BTreeMap::new(),
                    );
                    return Ok(Some(ObjectView {
                        subject_kind: subject.subject_kind.clone(),
                        subject_id_text: subject.subject_id_text.clone(),
                        visibility: "visible".to_string(),
                        redaction_level: "none".to_string(),
                        x: town.x,
                        y: town.y,
                        last_seen_turn: Some(context.session.current_turn),
                        display_name: Some(town.name.clone()),
                        asset_key: details.asset_key,
                        owner_participant_id: town
                            .owner_participant_id
                            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
                        details_json: format!(
                            "{{\"type\":\"town\",\"scenario_key\":\"{}\",\"town_id\":\"{}\",\"status\":\"{}\"}}",
                            escape_json(&subject.subject_id_text),
                            town.id(),
                            escape_json(&town.status)
                        ),
                    }));
                }
                return Ok(last_known_object_view_fast(subject, viewport));
            }
            let Some(town) = live_town_for_known(context.session.id(), subject)? else {
                return Ok(last_known_object_view_fast(subject, viewport));
            };
            if town.status != "active" {
                return Ok(None);
            }
            if viewport.is_some_and(|viewport| !viewport.contains(town.x, town.y)) {
                return Ok(None);
            }
            return Ok(Some(ObjectView {
                subject_kind: subject.subject_kind.clone(),
                subject_id_text: subject.subject_id_text.clone(),
                visibility: "visible".to_string(),
                redaction_level: "none".to_string(),
                x: town.x,
                y: town.y,
                last_seen_turn: Some(context.session.current_turn),
                display_name: Some(town.name.clone()),
                asset_key: None,
                owner_participant_id: town
                    .owner_participant_id
                    .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
                details_json: format!(
                    "{{\"type\":\"town\",\"scenario_key\":\"{}\",\"town_id\":\"{}\",\"status\":\"{}\"}}",
                    escape_json(&subject.subject_id_text),
                    town.id(),
                    escape_json(&town.status)
                ),
            }));
        }
        _ => {}
    }
    Ok(last_known_object_view_fast(subject, viewport))
}

fn world_object_list_view(
    context: &SessionCallerContext,
    subject: &ObjectSubject,
    live_world_objects_by_coord: Option<&BTreeMap<(u16, u16), WorldObject>>,
    visibility_by_coord: Option<&BTreeMap<(u16, u16), VisibilityChunk>>,
) -> Result<Option<ObjectView>, ApiError> {
    let mut state = "available".to_string();
    let mut owner_participant_id = None;
    let mut scoring_kind = if subject.subject_id_text.starts_with("mine:") {
        "mine"
    } else {
        "world_object"
    }
    .to_string();

    let mut x = subject.x;
    let mut y = subject.y;
    let mut details_json = None;
    let live_object = if let Some(objects) = live_world_objects_by_coord {
        objects.get(&(subject.x, subject.y)).cloned()
    } else {
        map_visibility_occupancy::find_world_object_by_session_xy(
            context.session.id(),
            subject.x,
            subject.y,
        )?
    };
    if live_world_objects_by_coord.is_some() && live_object.is_none() {
        return Ok(None);
    }
    if let Some(object) = live_object {
        if object.state == "collected" {
            return Ok(None);
        }
        x = object.x;
        y = object.y;
        state = object.state.clone();
        owner_participant_id = object
            .owner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string());
        scoring_kind = object.scoring_kind.clone();
        details_json = Some(world_object_live_details_json(
            &subject.subject_id_text,
            &object,
        ));
    }
    if visibility_by_coord.is_some()
        && !is_visible_for_projection(context, visibility_by_coord, x, y)?
    {
        return Ok(None);
    }

    Ok(Some(ObjectView {
        subject_kind: subject.subject_kind.clone(),
        subject_id_text: subject.subject_id_text.clone(),
        visibility: "visible".to_string(),
        redaction_level: "none".to_string(),
        x,
        y,
        last_seen_turn: Some(context.session.current_turn),
        display_name: Some(subject.subject_id_text.clone()),
        asset_key: None,
        owner_participant_id,
        details_json: details_json.unwrap_or_else(|| format!(
            "{{\"type\":\"world_object\",\"scenario_key\":\"{}\",\"object_slug\":\"{}\",\"state\":\"{}\",\"scoring_kind\":\"{}\"}}",
            escape_json(&subject.subject_id_text),
            escape_json(&subject.subject_id_text),
            escape_json(&state),
            escape_json(&scoring_kind)
        )),
    }))
}

fn live_world_objects_by_coord(
    session_id: Id<GameSession>,
) -> Result<BTreeMap<(u16, u16), WorldObject>, ApiError> {
    let session_id_text = session_id.to_string();
    let runtime_objects = session_turn_runtime::world_object_snapshots(&session_id_text);
    if !runtime_objects.is_empty() {
        return Ok(runtime_objects
            .into_iter()
            .map(|object| ((object.x, object.y), object))
            .collect());
    }
    let mut objects = map_visibility_occupancy::page_world_objects_by_session(
        session_id,
        domm_game::MAX_LIST_LIMIT,
        None,
    )?
    .items
    .into_iter()
    .map(|object| ((object.x, object.y), object))
    .collect::<BTreeMap<_, _>>();
    for object in session_turn_runtime::world_object_snapshots(&session_id_text) {
        objects.insert((object.x, object.y), object);
    }
    Ok(objects)
}

fn is_visible_for_projection(
    context: &SessionCallerContext,
    visibility_by_coord: Option<&BTreeMap<(u16, u16), VisibilityChunk>>,
    x: u16,
    y: u16,
) -> Result<bool, ApiError> {
    if let Some(visibility_by_coord) = visibility_by_coord {
        return Ok(is_visible_with_cache(
            &context.session,
            visibility_by_coord,
            x,
            y,
        ));
    }
    is_visible_at(&context.session, context.participant.id(), x, y)
}

fn scenario_champion_belongs_to_participant(
    context: &SessionCallerContext,
    subject_id_text: &str,
) -> bool {
    domm_game::first_playable_scenario()
        .starts
        .iter()
        .any(|start| {
            start.champion_key == subject_id_text
                && start.slot_index == context.participant.slot_index
        })
}

fn scenario_town_belongs_to_participant(
    context: &SessionCallerContext,
    subject_id_text: &str,
) -> bool {
    domm_game::first_playable_scenario()
        .starts
        .iter()
        .any(|start| {
            start.town_key == subject_id_text && start.slot_index == context.participant.slot_index
        })
}

fn fast_champion_for_known(
    context: &SessionCallerContext,
    subject: &ObjectSubject,
) -> Result<Option<Champion>, ApiError> {
    if let Ok(id) = Ulid::from_str(&subject.subject_id_text).map(Id::<Champion>::from_key) {
        let champion = session_turn_runtime::champion_snapshot(
            &context.session.id().to_string(),
            &id.to_string(),
        );
        return match champion {
            Some(champion) => Ok(Some(champion)),
            None => champions_artifacts::load_champion(id),
        };
    }
    if scenario_champion_belongs_to_participant(context, &subject.subject_id_text) {
        for champion_id in &context.participant.champion_ids {
            let champion_id = Id::<Champion>::from_key(*champion_id);
            let champion = match session_turn_runtime::champion_snapshot(
                &context.session.id().to_string(),
                &champion_id.to_string(),
            ) {
                Some(champion) => champion,
                None => {
                    let Some(champion) = champions_artifacts::load_champion(champion_id)? else {
                        continue;
                    };
                    champion
                }
            };
            if champion.session_id == context.session.id().key()
                && champion.participant_id == context.participant.id().key()
            {
                return Ok(Some(champion));
            }
        }
    }
    champions_artifacts::find_champion_by_session_xy(context.session.id(), subject.x, subject.y)
}

fn live_town_for_known(
    session_id: Id<GameSession>,
    subject: &ObjectSubject,
) -> Result<Option<Town>, ApiError> {
    if let Ok(id) = Ulid::from_str(&subject.subject_id_text).map(Id::<Town>::from_key) {
        return towns::load_town(id);
    }
    towns::find_town_by_session_xy(session_id, subject.x, subject.y)
}

fn live_neutral_for_known(
    session_id: Id<GameSession>,
    subject: &ObjectSubject,
) -> Result<Option<NeutralArmy>, ApiError> {
    if let Some(neutral) = cached_neutral_for_known(session_id, subject) {
        return Ok(Some(neutral));
    }
    if let Ok(id) = Ulid::from_str(&subject.subject_id_text).map(Id::<NeutralArmy>::from_key) {
        return neutrals::load_neutral_army(id);
    }
    neutrals::find_neutral_army_by_session_xy(session_id, subject.x, subject.y)
}

fn live_world_object_for_known(
    session_id: Id<GameSession>,
    subject: &ObjectSubject,
) -> Result<Option<WorldObject>, ApiError> {
    if let Ok(id) = Ulid::from_str(&subject.subject_id_text).map(Id::<WorldObject>::from_key) {
        if let Some(object) =
            session_turn_runtime::world_object_by_id(&session_id.to_string(), &id.to_string())
        {
            return Ok(Some(object));
        }
        return map_visibility_occupancy::load_world_object(id);
    }
    if let Some(runtime_result) =
        session_turn_runtime::world_object_at(&session_id.to_string(), subject.x, subject.y)
    {
        return Ok(runtime_result);
    }
    map_visibility_occupancy::find_world_object_by_session_xy(session_id, subject.x, subject.y)
}

fn last_known_object_view_fast(
    subject: &ObjectSubject,
    viewport: Option<&Viewport>,
) -> Option<ObjectView> {
    if viewport.is_some_and(|viewport| !viewport.contains(subject.x, subject.y)) {
        return None;
    }
    Some(ObjectView {
        subject_kind: subject.subject_kind.clone(),
        subject_id_text: subject.subject_id_text.clone(),
        visibility: "last_known".to_string(),
        redaction_level: "last_known".to_string(),
        x: subject.x,
        y: subject.y,
        last_seen_turn: Some(subject.last_seen_turn),
        display_name: None,
        asset_key: None,
        owner_participant_id: None,
        details_json: subject
            .redacted_json
            .clone()
            .unwrap_or_else(|| "{}".to_string()),
    })
}

#[derive(Clone)]
struct SubjectDetails {
    display_name: String,
    asset_key: Option<String>,
}

fn scenario_subject_details(
    subject_kind: &str,
    subject_id_text: &str,
    _participants_by_slot: &BTreeMap<u8, String>,
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
            };
        }
    }
    let object = scenario
        .mines
        .iter()
        .chain(scenario.external_dwellings.iter())
        .chain(scenario.central_objectives.iter())
        .find(|object| object.key == subject_id_text)
        .map(|object| object.object_slug.clone())
        .or_else(|| {
            scenario
                .resource_piles
                .iter()
                .find(|pile| pile.key == subject_id_text)
                .map(|pile| pile.object_slug.clone())
        });
    if let Some(slug) = object {
        let definition = manifest.map_object(&slug);
        return SubjectDetails {
            display_name: definition
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| slug.clone()),
            asset_key: definition.and_then(|definition| definition.sprite_key.clone()),
        };
    }

    SubjectDetails {
        display_name: subject_id_text.to_string(),
        asset_key: None,
    }
}

fn world_object_live_details_json(scenario_key: &str, object: &WorldObject) -> String {
    let object_slug = json_string_field(object.instance_json.as_deref(), "object_slug")
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{{\"type\":\"world_object\",\"scenario_key\":\"{}\",\"object_id\":\"{}\",\"object_slug\":\"{}\",\"state\":\"{}\",\"scoring_kind\":\"{}\"}}",
        escape_json(scenario_key),
        object.id(),
        escape_json(&object_slug),
        escape_json(&object.state),
        escape_json(&object.scoring_kind)
    )
}

fn json_string_field(json: Option<&str>, field: &str) -> Option<String> {
    let json = json?;
    let needle = format!(r#""{field}":""#);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn resolve_champion(session: &GameSession, champion_id: &str) -> Result<Champion, ApiError> {
    if let Ok(id) = Ulid::from_str(champion_id).map(Id::<Champion>::from_key) {
        if let Some(champion) =
            session_turn_runtime::champion_snapshot(&session.id().to_string(), &id.to_string())
        {
            return Ok(champion);
        }
        return champions_artifacts::load_champion(id)?
            .ok_or_else(|| public_error("not_found", "champion not found", false));
    }
    let scenario = domm_game::first_playable_scenario();
    let start = scenario
        .starts
        .iter()
        .find(|start| start.champion_key == champion_id)
        .ok_or_else(|| public_error("not_found", "champion not found", false))?;
    if let Some(champion) = session_turn_runtime::champion_snapshot_by_start(
        &session.id().to_string(),
        start.champion_x,
        start.champion_y,
    ) {
        return Ok(champion);
    }
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

fn is_visible_with_cache(
    session: &GameSession,
    visibility_by_coord: &BTreeMap<(u16, u16), VisibilityChunk>,
    x: u16,
    y: u16,
) -> bool {
    let chunk_size = u16::from(session.chunk_size);
    let chunk_x = x / chunk_size;
    let chunk_y = y / chunk_size;
    let Some(visibility) = visibility_by_coord.get(&(chunk_x, chunk_y)) else {
        return false;
    };
    let width = chunk_width(session, chunk_x);
    let local_x = x % chunk_size;
    let local_y = y % chunk_size;
    let index = usize::from(local_y) * usize::from(width) + usize::from(local_x);
    domm_game::read_visibility_bit(visibility.visible_blob.as_slice(), index)
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
