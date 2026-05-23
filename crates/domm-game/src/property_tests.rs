use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BATTLE_SIDE_DEFENDER, ChampionError, CommandResponse, CommandStatus, FIRST_PLAYABLE_MAP_HEIGHT,
    FIRST_PLAYABLE_MAP_WIDTH, FixtureApiBackend, MoveCoord, RecruitTarget, ResourceBalances,
    TURN_DURATION_MS, apply_damage_to_stack, build_first_playable_aftermath_state,
    build_first_playable_battle_state, build_first_playable_champion_state,
    build_first_playable_economy_state, build_first_playable_map_state,
    build_first_playable_movement_state, build_first_playable_town_state,
    check_and_finalize_victory, damage_preview, first_playable_content_manifest,
    first_playable_fixture, first_playable_scenario, initiative_order, max_turn_rule_state,
    objective_status, preview_move_path,
};

#[test]
fn property_movement_previews_are_read_only_adjacent_and_monotonic() {
    let fixture = first_playable_fixture();
    let movement = build_first_playable_movement_state();
    let map = build_first_playable_map_state();
    let champions = build_first_playable_champion_state();
    let champion = champions
        .champion("champion:west")
        .expect("west champion should exist");
    let paths = [
        vec![MoveCoord::new(9, 24)],
        vec![MoveCoord::new(9, 24), MoveCoord::new(10, 24)],
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
        ],
    ];
    let mut previous_cost = 0;

    for path in paths {
        assert!(MoveCoord::new(champion.x, champion.y).is_adjacent_to(path[0]));
        assert!(
            path.windows(2)
                .all(|window| window[0].is_adjacent_to(window[1]))
        );
        let preview = preview_move_path(
            &movement,
            &map,
            &champions,
            &fixture.ids.participant_one_id,
            "champion:west",
            path.clone(),
            1_000,
        )
        .expect("preview path should be legal");

        assert_eq!(preview.path, path);
        assert!(preview.total_cost >= previous_cost);
        assert!(preview.total_cost <= preview.available_movement);
        assert!(preview.chunks_touched > 0);
        assert!(preview.chunks_touched <= preview.path.len() as u32);
        assert!(movement.intents.is_empty());
        previous_cost = preview.total_cost;
    }
}

#[test]
fn property_battle_initiative_damage_and_survivors_stay_bounded() {
    let state = build_first_playable_battle_state().expect("battle fixture should build");
    let battle_id = state.battles[0].battle_id.clone();
    let first_order = initiative_order(&state, &battle_id).expect("initiative should build");
    let second_order = initiative_order(&state, &battle_id).expect("initiative should be stable");
    let living_stack_count = state
        .stacks
        .iter()
        .filter(|stack| stack.battle_id == battle_id && stack.is_living())
        .count();
    assert_eq!(first_order, second_order);
    assert_eq!(first_order.len(), living_stack_count);
    for window in first_order.windows(2) {
        if !window[0].waited && !window[1].waited {
            assert!(
                (
                    window[0].initiative,
                    window[0].speed,
                    std::cmp::Reverse(window[0].tie_breaker)
                ) >= (
                    window[1].initiative,
                    window[1].speed,
                    std::cmp::Reverse(window[1].tie_breaker)
                )
            );
        }
    }

    let attacker = state
        .battle(&battle_id)
        .expect("battle should exist")
        .active_stack_id
        .as_deref()
        .and_then(|stack_id| state.stack(stack_id).ok())
        .expect("active stack should exist")
        .clone();
    let target = state
        .stacks
        .iter()
        .find(|stack| stack.side == BATTLE_SIDE_DEFENDER && stack.is_living())
        .expect("defender target should exist")
        .clone();
    let preview = damage_preview(&attacker, &target);
    assert!(preview.min_damage <= preview.max_damage);
    assert!(preview.estimated_kills_min <= preview.estimated_kills_max);
    assert!(preview.estimated_kills_max <= target.quantity);

    let damage_cases = [
        0,
        1,
        u32::from(target.max_hp.saturating_sub(1)),
        u32::from(target.max_hp),
        target
            .quantity
            .saturating_mul(u32::from(target.max_hp))
            .saturating_add(10_000),
    ];
    for damage in damage_cases {
        let mut clone = state.clone();
        let (killed, quantity_after, front_hp_after) = apply_damage_to_stack(
            &mut clone,
            &target.battle_stack_id,
            damage,
            &format!("command:property:damage:{damage}"),
        )
        .expect("damage should apply");

        assert!(killed <= target.quantity);
        assert_eq!(quantity_after, target.quantity - killed);
        assert!(front_hp_after <= target.max_hp);
        if quantity_after == 0 {
            assert_eq!(
                clone.stack(&target.battle_stack_id).unwrap().status,
                "defeated"
            );
            assert!(
                clone
                    .occupancy
                    .iter()
                    .all(|row| row.battle_stack_id != target.battle_stack_id)
            );
        }
    }
}

#[test]
fn property_town_resource_cost_previews_match_manifest_costs() {
    let fixture = first_playable_fixture();
    let manifest = first_playable_content_manifest();
    let mut town = build_first_playable_town_state();
    let mut economy = build_first_playable_economy_state();
    let participant_id = fixture.ids.participant_one_id.as_str();

    for building in &manifest.buildings {
        let preview = town
            .preview_build_town_structure(&economy, participant_id, "town:west", &building.slug, 2)
            .expect("build preview should return a decision");
        assert_eq!(preview.cost, ResourceBalances::from_cost(&building.cost));
        assert_eq!(preview.building_slug, building.slug);
    }

    town.submit_build_town_structure(
        &mut economy,
        participant_id,
        "town:west",
        "freehold-training-yard",
        2,
        "command:property:build-training-yard",
    )
    .expect("training yard should build");
    town.materialize_recruit_pool_growth("town:west", "mudhook-levy", 8, "command:property:growth")
        .expect("recruit pool should grow");
    let unit = manifest
        .unit("mudhook-levy")
        .expect("mudhook levy should exist");
    for quantity in 1..=3 {
        let preview = town
            .preview_recruit_units(
                &economy,
                participant_id,
                "town:west",
                "mudhook-levy",
                quantity,
                &RecruitTarget::TownGarrison { slot_index: None },
                8,
            )
            .expect("recruit preview should return a decision");
        assert!(preview.allowed);
        assert_eq!(
            preview.total_cost.gold,
            u64::from(unit.cost.gold) * u64::from(quantity)
        );
        assert_eq!(preview.total_cost.wood, unit.cost.wood * quantity);
        assert_eq!(preview.total_cost.stone, unit.cost.stone * quantity);
        assert_eq!(preview.total_cost.iron, unit.cost.iron * quantity);
        assert_eq!(preview.total_cost.crystal, unit.cost.crystal * quantity);
        assert_eq!(preview.total_cost.ember, unit.cost.ember * quantity);
        assert_eq!(preview.total_cost.aether, unit.cost.aether * quantity);
    }
}

#[test]
fn property_spell_prerequisites_targets_and_mana_are_enforced() {
    let manifest = first_playable_content_manifest();

    for spell in &manifest.spells {
        let mut unskilled = build_first_playable_champion_state();
        let missing = unskilled
            .learn_spell(
                "champion:west",
                &manifest,
                &spell.slug,
                2,
                &format!("command:property:learn-missing:{}", spell.slug),
            )
            .expect_err("spell learning should require sour sorcery");
        assert!(matches!(
            missing,
            ChampionError::SpellPrerequisiteMissing { .. }
        ));

        let mut skilled = build_first_playable_champion_state();
        skilled
            .grant_experience("champion:west", 1_000, "command:property:xp")
            .expect("experience should grant a skill point");
        skilled
            .select_level_up_choice(
                "champion:west",
                "sour_sorcery",
                "command:property:sour-sorcery",
            )
            .expect("sour sorcery should be selectable");
        skilled
            .learn_spell(
                "champion:west",
                &manifest,
                &spell.slug,
                2,
                &format!("command:property:learn:{}", spell.slug),
            )
            .expect("skilled champion should learn spell");
        assert_eq!(
            skilled.learned_spell_slugs("champion:west"),
            vec![spell.slug.clone()]
        );

        let mana_before = skilled
            .effective_mana("champion:west", 3)
            .expect("mana should resolve");
        let cast = skilled.cast_adventure_spell(
            "champion:west",
            &manifest,
            &spell.slug,
            3,
            &format!("command:property:cast:{}", spell.slug),
        );
        if spell.target_type == "self_champion" {
            let receipt = cast.expect("self-targeted adventure spell should cast");
            assert_eq!(
                receipt.mana_after,
                mana_before.saturating_sub(spell.mana_cost)
            );
        } else {
            assert!(matches!(
                cast,
                Err(ChampionError::InvalidSpellTarget { .. })
            ));
        }
    }
}

#[test]
fn property_scenario_victory_gameover_and_manifest_references_are_monotonic() {
    for required in 1..=4 {
        let mut complete_seen = false;
        for progress in 0..=6 {
            let status = objective_status(progress, required);
            if complete_seen {
                assert_eq!(status, "complete");
            }
            if status == "complete" {
                complete_seen = true;
            }
        }
    }
    for max_turn in 1..=8 {
        for current_turn in 0..=max_turn + 2 {
            let state = max_turn_rule_state(current_turn, max_turn);
            assert_eq!(
                state,
                if current_turn >= max_turn {
                    "max_turn_reached"
                } else {
                    "active"
                }
            );
        }
    }

    let fixture = first_playable_fixture();
    let mut aftermath = build_first_playable_aftermath_state().expect("aftermath state builds");
    aftermath
        .town
        .town_mut("town:east")
        .expect("east town exists")
        .owner_participant_id = fixture.ids.participant_one_id.clone();
    aftermath.battle.battles.clear();
    aftermath
        .champions
        .set_champion_status(
            "champion:east",
            "defeated",
            12,
            "command:property:defeat-east",
        )
        .expect("east champion should be defeated");
    let first = check_and_finalize_victory(
        &mut aftermath,
        "command:property:victory",
        1_800_000_600_000,
    )
    .expect("victory should finalize");
    assert!(first.finalized);
    assert_eq!(aftermath.session.state, "finished");
    let event_count = aftermath.aftermath_events.len();
    let summary_count = aftermath.player_match_summaries.len();
    let replay = check_and_finalize_victory(
        &mut aftermath,
        "command:property:victory:replay",
        1_800_000_600_100,
    )
    .expect("finished gameover check should be idempotent");
    assert!(replay.finalized);
    assert_eq!(replay.winner_participant_id, first.winner_participant_id);
    assert_eq!(aftermath.aftermath_events.len(), event_count);
    assert_eq!(aftermath.player_match_summaries.len(), summary_count);

    let manifest = first_playable_content_manifest();
    let scenario = first_playable_scenario();
    let faction_slugs = manifest
        .factions
        .iter()
        .map(|faction| faction.slug.as_str())
        .collect::<BTreeSet<_>>();
    let class_slugs = manifest
        .champion_classes
        .iter()
        .map(|class| class.slug.as_str())
        .collect::<BTreeSet<_>>();
    let unit_slugs = manifest
        .units
        .iter()
        .map(|unit| unit.slug.as_str())
        .collect::<BTreeSet<_>>();
    let building_slugs = manifest
        .buildings
        .iter()
        .map(|building| building.slug.as_str())
        .collect::<BTreeSet<_>>();
    let object_slugs = manifest
        .map_objects
        .iter()
        .map(|object| object.slug.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(faction_slugs.len(), manifest.factions.len());
    assert_eq!(class_slugs.len(), manifest.champion_classes.len());
    assert_eq!(unit_slugs.len(), manifest.units.len());
    assert_eq!(building_slugs.len(), manifest.buildings.len());
    assert_eq!(object_slugs.len(), manifest.map_objects.len());
    for start in &scenario.starts {
        assert!(faction_slugs.contains(start.faction_slug.as_str()));
        assert!(class_slugs.contains(start.champion_class_slug.as_str()));
        for stack in &start.starting_army_stacks {
            assert!(unit_slugs.contains(stack.unit_slug.as_str()));
        }
    }
    for object in scenario
        .mines
        .iter()
        .chain(scenario.external_dwellings.iter())
        .chain(scenario.central_objectives.iter())
    {
        assert!(object_slugs.contains(object.object_slug.as_str()));
    }
    for pile in &scenario.resource_piles {
        assert!(object_slugs.contains(pile.object_slug.as_str()));
    }
    for step in &scenario.walkthrough.steps {
        if let Some(slug) = step.target_key.strip_prefix("building:") {
            assert!(building_slugs.contains(slug));
        } else if let Some(slug) = step.target_key.strip_prefix("unit:") {
            assert!(unit_slugs.contains(slug));
        }
    }
}

#[test]
fn property_seeded_command_walk_replays_and_preserves_invariants() {
    const SEED: u64 = 0xd00d_f00d_cafe_babe;
    const CHAMPION_ID: &str = "champion:west";

    let fixture = first_playable_fixture();
    let caller = fixture.principals.player_one;
    let audience_key = format!("participant:{}", fixture.ids.participant_one_id);
    let mut backend = FixtureApiBackend::new(fixture);
    let session = backend.start_first_playable_session();
    let mut rng = SeededWalkRng::new(SEED);
    let mut nonce_receipts = BTreeMap::new();

    for step in 0..6 {
        let view = backend
            .get_default_game_view(caller, &session.session_id)
            .unwrap_or_else(|error| {
                panic!("seed {SEED:#x} step {step}: game view failed: {error:?}")
            });
        let current_turn = view.session.current_turn;
        let submit_at = u64::from(current_turn.saturating_sub(1))
            .saturating_mul(TURN_DURATION_MS)
            .saturating_add(1_000 + u64::from(step));
        let sync_at = u64::from(current_turn).saturating_mul(TURN_DURATION_MS);

        let bad_nonce = format!("nonce:property-walk:{SEED:x}:bad:{step}");
        let bad = backend.submit_move_intent(
            caller,
            &session.session_id,
            CHAMPION_ID,
            vec![MoveCoord::new(
                FIRST_PLAYABLE_MAP_WIDTH,
                FIRST_PLAYABLE_MAP_HEIGHT,
            )],
            &bad_nonce,
            submit_at,
        );
        assert_eq!(bad.status, CommandStatus::Failed);
        assert_stored_status(
            &backend,
            &bad_nonce,
            &bad.command_id,
            CommandStatus::Failed,
            SEED,
            step,
        );
        assert_stable_nonce_receipt(&mut nonce_receipts, &bad, SEED, step);
        let bad_replay = backend.submit_move_intent(
            caller,
            &session.session_id,
            CHAMPION_ID,
            vec![MoveCoord::new(
                FIRST_PLAYABLE_MAP_WIDTH,
                FIRST_PLAYABLE_MAP_HEIGHT,
            )],
            &bad_nonce,
            submit_at,
        );
        assert_eq!(bad_replay.command_id, bad.command_id);
        assert_stable_nonce_receipt(&mut nonce_receipts, &bad_replay, SEED, step);

        let champion = backend
            .get_champion_view(caller, &session.session_id, CHAMPION_ID)
            .unwrap_or_else(|error| {
                panic!("seed {SEED:#x} step {step}: champion view failed: {error:?}")
            });
        let path = choose_seeded_legal_step(
            &mut backend,
            caller,
            &session.session_id,
            CHAMPION_ID,
            champion.x,
            champion.y,
            submit_at,
            &mut rng,
            SEED,
            step,
        );
        let move_nonce = format!("nonce:property-walk:{SEED:x}:move:{step}");
        let submitted = backend.submit_move_intent(
            caller,
            &session.session_id,
            CHAMPION_ID,
            path.clone(),
            &move_nonce,
            submit_at,
        );
        assert_eq!(submitted.status, CommandStatus::Applied);
        assert_stored_status(
            &backend,
            &move_nonce,
            &submitted.command_id,
            CommandStatus::Applied,
            SEED,
            step,
        );
        assert_stable_nonce_receipt(&mut nonce_receipts, &submitted, SEED, step);
        let replay = backend.submit_move_intent(
            caller,
            &session.session_id,
            CHAMPION_ID,
            path.clone(),
            &move_nonce,
            submit_at,
        );
        assert_eq!(replay.command_id, submitted.command_id);
        assert_eq!(replay.status, CommandStatus::Applied);
        assert_stable_nonce_receipt(&mut nonce_receipts, &replay, SEED, step);

        let closed_nonce = format!("nonce:property-walk:{SEED:x}:closed:{step}");
        let closed = backend.submit_move_intent(
            caller,
            &session.session_id,
            CHAMPION_ID,
            path,
            &closed_nonce,
            sync_at,
        );
        assert_eq!(closed.status, CommandStatus::Failed);
        assert_stored_status(
            &backend,
            &closed_nonce,
            &closed.command_id,
            CommandStatus::Failed,
            SEED,
            step,
        );
        assert_stable_nonce_receipt(&mut nonce_receipts, &closed, SEED, step);

        let sync_nonce = format!("nonce:property-walk:{SEED:x}:sync:{step}");
        let synced = backend.sync_session_turn(caller, &session.session_id, sync_at, &sync_nonce);
        assert_eq!(synced.status, CommandStatus::Applied);
        assert_stored_status(
            &backend,
            &sync_nonce,
            &synced.command_id,
            CommandStatus::Applied,
            SEED,
            step,
        );
        assert_stable_nonce_receipt(&mut nonce_receipts, &synced, SEED, step);
        let synced_replay =
            backend.sync_session_turn(caller, &session.session_id, sync_at, &sync_nonce);
        assert_eq!(synced_replay.command_id, synced.command_id);
        assert_stable_nonce_receipt(&mut nonce_receipts, &synced_replay, SEED, step);

        let events = backend.get_events_after(&session.session_id, &audience_key, 0, 128);
        assert_unique_event_sequences(&events.events, SEED, step);
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == "submit_move_intent"),
            "seed {SEED:#x} step {step}: expected move event in feed"
        );
        let participant = backend
            .get_my_participant(caller, &session.session_id)
            .unwrap_or_else(|error| {
                panic!("seed {SEED:#x} step {step}: participant view failed: {error:?}")
            });
        assert!(participant.resources.gold <= 1_000_000);
        assert!(participant.resources.wood <= 1_000_000);
    }
}

fn choose_seeded_legal_step(
    backend: &mut FixtureApiBackend,
    caller: candid::Principal,
    session_id: &str,
    champion_id: &str,
    x: u16,
    y: u16,
    now_ms: u64,
    rng: &mut SeededWalkRng,
    seed: u64,
    step: u32,
) -> Vec<MoveCoord> {
    for (dx, dy) in rng.shuffled_dirs() {
        let Some(next_x) = offset_coord(x, dx) else {
            continue;
        };
        let Some(next_y) = offset_coord(y, dy) else {
            continue;
        };
        if !(7..=10).contains(&next_x) || !(22..=25).contains(&next_y) {
            continue;
        }
        if next_x >= FIRST_PLAYABLE_MAP_WIDTH || next_y >= FIRST_PLAYABLE_MAP_HEIGHT {
            continue;
        }
        let path = vec![MoveCoord::new(next_x, next_y)];
        if backend
            .preview_move(caller, session_id, champion_id, path.clone(), now_ms)
            .is_ok()
        {
            return path;
        }
    }

    panic!("seed {seed:#x} step {step}: no legal adjacent walk candidate from ({x},{y})");
}

fn offset_coord(value: u16, delta: i16) -> Option<u16> {
    if delta.is_negative() {
        value.checked_sub(delta.unsigned_abs())
    } else {
        value.checked_add(delta as u16)
    }
}

fn assert_stored_status(
    backend: &FixtureApiBackend,
    nonce: &str,
    command_id: &str,
    expected: CommandStatus,
    seed: u64,
    step: u32,
) {
    let by_nonce = backend.get_command_status(nonce).unwrap_or_else(|| {
        panic!("seed {seed:#x} step {step}: missing command status for nonce {nonce}")
    });
    let by_id = backend.get_command_status(command_id).unwrap_or_else(|| {
        panic!("seed {seed:#x} step {step}: missing command status for id {command_id}")
    });
    assert_eq!(by_nonce.command_id, command_id);
    assert_eq!(by_id.command_id, command_id);
    assert_eq!(by_nonce.status, expected);
    assert_eq!(by_id.status, expected);
}

fn assert_stable_nonce_receipt(
    receipts: &mut BTreeMap<String, String>,
    response: &CommandResponse,
    seed: u64,
    step: u32,
) {
    let key = format!("{}:{}", response.command_type, response.client_nonce);
    if let Some(existing) = receipts.insert(key.clone(), response.command_id.clone()) {
        assert_eq!(
            existing, response.command_id,
            "seed {seed:#x} step {step}: nonce {key} changed command receipt"
        );
    }
}

fn assert_unique_event_sequences(events: &[crate::ApiEventView], seed: u64, step: u32) {
    let mut seen = BTreeSet::new();
    for event in events {
        assert!(
            seen.insert(event.event_seq),
            "seed {seed:#x} step {step}: duplicate event seq {}",
            event.event_seq
        );
    }
}

#[derive(Clone, Debug)]
struct SeededWalkRng {
    state: u64,
}

impl SeededWalkRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn shuffled_dirs(&mut self) -> [(i16, i16); 4] {
        let mut dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)];
        for index in (1..dirs.len()).rev() {
            let swap_with = (self.next_u64() as usize) % (index + 1);
            dirs.swap(index, swap_with);
        }
        dirs
    }
}
