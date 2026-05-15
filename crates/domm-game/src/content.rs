use std::fmt::Write as _;

use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::fixtures::FIRST_PLAYABLE_SCENARIO_SEED;

pub const FIRST_PLAYABLE_RULESET_ID: &str = "ruleset:first-playable:v1";
pub const FIRST_PLAYABLE_RULESET_SLUG: &str = "domm-first-playable";
pub const FIRST_PLAYABLE_RULESET_VERSION: u32 = 1;
pub const FIRST_PLAYABLE_MAP_WIDTH: u16 = 48;
pub const FIRST_PLAYABLE_MAP_HEIGHT: u16 = 48;
pub const FIRST_PLAYABLE_CHUNK_SIZE: u8 = 16;
pub const FIRST_PLAYABLE_MAX_TURNS: u32 = 30;
pub const FIRST_PLAYABLE_PLAYER_COUNT: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ContentManifest {
    pub ruleset: RulesetContent,
    pub factions: Vec<FactionContent>,
    pub champion_classes: Vec<ChampionClassContent>,
    pub terrain: Vec<TerrainContent>,
    pub units: Vec<UnitContent>,
    pub buildings: Vec<BuildingContent>,
    pub spells: Vec<SpellContent>,
    pub artifacts: Vec<ArtifactContent>,
    pub map_objects: Vec<MapObjectContent>,
    pub asset_keys: Vec<String>,
}

impl ContentManifest {
    #[must_use]
    pub fn computed_content_hash(&self) -> String {
        compute_content_manifest_hash(self)
    }

    #[must_use]
    pub fn faction(&self, slug: &str) -> Option<&FactionContent> {
        self.factions.iter().find(|item| item.slug == slug)
    }

    #[must_use]
    pub fn champion_class(&self, slug: &str) -> Option<&ChampionClassContent> {
        self.champion_classes.iter().find(|item| item.slug == slug)
    }

    #[must_use]
    pub fn terrain(&self, terrain_key: &str) -> Option<&TerrainContent> {
        self.terrain
            .iter()
            .find(|item| item.terrain_key == terrain_key)
    }

    #[must_use]
    pub fn unit(&self, slug: &str) -> Option<&UnitContent> {
        self.units.iter().find(|item| item.slug == slug)
    }

    #[must_use]
    pub fn building(&self, slug: &str) -> Option<&BuildingContent> {
        self.buildings.iter().find(|item| item.slug == slug)
    }

    #[must_use]
    pub fn artifact(&self, slug: &str) -> Option<&ArtifactContent> {
        self.artifacts.iter().find(|item| item.slug == slug)
    }

    #[must_use]
    pub fn map_object(&self, slug: &str) -> Option<&MapObjectContent> {
        self.map_objects.iter().find(|item| item.slug == slug)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RulesetContent {
    pub id: String,
    pub slug: String,
    pub version: u32,
    pub name: String,
    pub description: Option<String>,
    pub content_manifest_hash: String,
    pub map_width: u16,
    pub map_height: u16,
    pub chunk_size: u8,
    pub player_count: u8,
    pub max_turns: u32,
    pub turn_duration_ms: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FactionContent {
    pub id: String,
    pub ruleset_id: String,
    pub slug: String,
    pub name: String,
    pub theme: Option<String>,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub banner_key: Option<String>,
    pub native_terrain: Option<String>,
    pub trait_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ChampionClassContent {
    pub id: String,
    pub ruleset_id: String,
    pub faction_slug: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub portrait_key: Option<String>,
    pub base_movement: u16,
    pub base_vision: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TerrainContent {
    pub id: String,
    pub ruleset_id: String,
    pub terrain_key: String,
    pub terrain_code: u8,
    pub name: String,
    pub movement_cost: u16,
    pub passable: bool,
    pub sprite_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourceCost {
    pub gold: u32,
    pub wood: u32,
    pub stone: u32,
    pub iron: u32,
    pub crystal: u32,
    pub ember: u32,
    pub aether: u32,
}

impl ResourceCost {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            gold: 0,
            wood: 0,
            stone: 0,
            iron: 0,
            crystal: 0,
            ember: 0,
            aether: 0,
        }
    }

    #[must_use]
    pub const fn starting_resources() -> Self {
        Self {
            gold: 10_000,
            wood: 10,
            stone: 10,
            iron: 3,
            crystal: 3,
            ember: 3,
            aether: 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct UnitContent {
    pub id: String,
    pub ruleset_id: String,
    pub faction_slug: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub sprite_key: Option<String>,
    pub icon_key: Option<String>,
    pub animation_key: Option<String>,
    pub tier: u8,
    pub attack: i16,
    pub defense: i16,
    pub damage_min: u16,
    pub damage_max: u16,
    pub max_hp: u16,
    pub speed: u8,
    pub initiative: u8,
    pub ranged: bool,
    pub flying: bool,
    pub shots: u16,
    pub cost: ResourceCost,
    pub weekly_growth: u16,
    pub ability_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BuildingContent {
    pub id: String,
    pub ruleset_id: String,
    pub faction_slug: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub building_type: String,
    pub cost: ResourceCost,
    pub requires_building_slugs: Vec<String>,
    pub unlocks_unit_slug: Option<String>,
    pub effect_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct SpellContent {
    pub id: String,
    pub ruleset_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub school: String,
    pub level: u8,
    pub mana_cost: u16,
    pub target_type: String,
    pub effect_key: String,
    pub duration_rounds: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArtifactContent {
    pub id: String,
    pub ruleset_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_key: Option<String>,
    pub slot: String,
    pub rarity: String,
    pub effect_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct MapObjectContent {
    pub id: String,
    pub ruleset_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub sprite_key: Option<String>,
    pub icon_key: Option<String>,
    pub object_type: String,
    pub footprint_w: u8,
    pub footprint_h: u8,
    pub blocking: bool,
    pub interaction_key: String,
    pub refresh_rule: String,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FirstPlayableScenario {
    pub scenario_key: String,
    pub scenario_seed: String,
    pub scenario_hash: String,
    pub ruleset_slug: String,
    pub ruleset_version: u32,
    pub map: HandAuthoredMap,
    pub starting_state: StartingState,
    pub starts: Vec<PlayerStart>,
    pub mines: Vec<ObjectSeed>,
    pub resource_piles: Vec<ResourcePileSeed>,
    pub central_objectives: Vec<ObjectSeed>,
    pub neutral_armies: Vec<NeutralArmySeed>,
    pub walkthrough: FirstPlayableWalkthrough,
}

impl FirstPlayableScenario {
    #[must_use]
    pub fn computed_scenario_hash(&self) -> String {
        compute_scenario_hash(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct HandAuthoredMap {
    pub width: u16,
    pub height: u16,
    pub chunk_size: u8,
    pub default_terrain_key: String,
    pub terrain_patches: Vec<TerrainPatch>,
    pub road_paths: Vec<RoadPath>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TerrainPatch {
    pub terrain_key: String,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RoadPath {
    pub key: String,
    pub waypoints: Vec<TileCoord>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct StartingState {
    pub resources: ResourceCost,
    pub champion_level: u8,
    pub champion_movement: u16,
    pub champion_vision: u8,
    pub town_hall_level: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct PlayerStart {
    pub slot_index: u8,
    pub faction_slug: String,
    pub champion_class_slug: String,
    pub town_key: String,
    pub town_name: String,
    pub town_x: u16,
    pub town_y: u16,
    pub champion_key: String,
    pub champion_name: String,
    pub champion_x: u16,
    pub champion_y: u16,
    pub starting_army_stacks: Vec<ArmyStackSeed>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ArmyStackSeed {
    pub unit_slug: String,
    pub quantity: u16,
    pub slot_index: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ObjectSeed {
    pub key: String,
    pub object_slug: String,
    pub x: u16,
    pub y: u16,
    pub owner_slot_index: Option<u8>,
    pub guard_neutral_army_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ResourcePileSeed {
    pub key: String,
    pub object_slug: String,
    pub x: u16,
    pub y: u16,
    pub reward: ResourceCost,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct NeutralArmySeed {
    pub key: String,
    pub strength_band: String,
    pub x: u16,
    pub y: u16,
    pub stacks: Vec<ArmyStackSeed>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FirstPlayableWalkthrough {
    pub steps: Vec<WalkthroughStep>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct WalkthroughStep {
    pub step_key: String,
    pub actor_slot_index: u8,
    pub command_hint: String,
    pub target_key: String,
    pub expected_result: String,
}

#[must_use]
pub fn get_content_manifest(ruleset_slug: &str, version: u32) -> Option<ContentManifest> {
    (ruleset_slug == FIRST_PLAYABLE_RULESET_SLUG && version == FIRST_PLAYABLE_RULESET_VERSION)
        .then(first_playable_content_manifest)
}

#[must_use]
pub fn first_playable_content_manifest() -> ContentManifest {
    let ruleset_id = FIRST_PLAYABLE_RULESET_ID.to_string();
    let mut manifest = ContentManifest {
        ruleset: RulesetContent {
            id: ruleset_id.clone(),
            slug: FIRST_PLAYABLE_RULESET_SLUG.to_string(),
            version: FIRST_PLAYABLE_RULESET_VERSION,
            name: "Degens First Playable".to_string(),
            description: Some(
                "A compact 1v1 conquest ruleset for the first playable map.".to_string(),
            ),
            content_manifest_hash: String::new(),
            map_width: FIRST_PLAYABLE_MAP_WIDTH,
            map_height: FIRST_PLAYABLE_MAP_HEIGHT,
            chunk_size: FIRST_PLAYABLE_CHUNK_SIZE,
            player_count: FIRST_PLAYABLE_PLAYER_COUNT,
            max_turns: FIRST_PLAYABLE_MAX_TURNS,
            turn_duration_ms: 60_000,
        },
        factions: first_playable_factions(&ruleset_id),
        champion_classes: first_playable_champion_classes(&ruleset_id),
        terrain: first_playable_terrain(&ruleset_id),
        units: first_playable_units(&ruleset_id),
        buildings: first_playable_buildings(&ruleset_id),
        spells: Vec::new(),
        artifacts: first_playable_artifacts(&ruleset_id),
        map_objects: first_playable_map_objects(&ruleset_id),
        asset_keys: first_playable_asset_keys(),
    };
    manifest.ruleset.content_manifest_hash = manifest.computed_content_hash();
    manifest
}

#[must_use]
pub fn first_playable_scenario() -> FirstPlayableScenario {
    let mut scenario = FirstPlayableScenario {
        scenario_key: "scenario:first-playable:v1".to_string(),
        scenario_seed: FIRST_PLAYABLE_SCENARIO_SEED.to_string(),
        scenario_hash: String::new(),
        ruleset_slug: FIRST_PLAYABLE_RULESET_SLUG.to_string(),
        ruleset_version: FIRST_PLAYABLE_RULESET_VERSION,
        map: first_playable_map(),
        starting_state: StartingState {
            resources: ResourceCost::starting_resources(),
            champion_level: 1,
            champion_movement: 240,
            champion_vision: 5,
            town_hall_level: 1,
        },
        starts: vec![
            PlayerStart {
                slot_index: 0,
                faction_slug: "gutterborn-freehold".to_string(),
                champion_class_slug: "toll-broken-captain".to_string(),
                town_key: "town:west".to_string(),
                town_name: "West Woe".to_string(),
                town_x: 6,
                town_y: 24,
                champion_key: "champion:west".to_string(),
                champion_name: "Mara of the Toll".to_string(),
                champion_x: 8,
                champion_y: 24,
                starting_army_stacks: vec![
                    ArmyStackSeed {
                        unit_slug: "mudhook-levy".to_string(),
                        quantity: 24,
                        slot_index: 0,
                    },
                    ArmyStackSeed {
                        unit_slug: "tollroad-skirmisher".to_string(),
                        quantity: 10,
                        slot_index: 1,
                    },
                ],
            },
            PlayerStart {
                slot_index: 1,
                faction_slug: "ashen-ledger".to_string(),
                champion_class_slug: "ash-auditor".to_string(),
                town_key: "town:east".to_string(),
                town_name: "East Due".to_string(),
                town_x: 41,
                town_y: 24,
                champion_key: "champion:east".to_string(),
                champion_name: "Korrin of Receipts".to_string(),
                champion_x: 39,
                champion_y: 24,
                starting_army_stacks: vec![
                    ArmyStackSeed {
                        unit_slug: "ash-scripter".to_string(),
                        quantity: 24,
                        slot_index: 0,
                    },
                    ArmyStackSeed {
                        unit_slug: "cinder-crossbow".to_string(),
                        quantity: 10,
                        slot_index: 1,
                    },
                ],
            },
        ],
        mines: vec![
            ObjectSeed {
                key: "mine:west-gold".to_string(),
                object_slug: "gold-mine".to_string(),
                x: 12,
                y: 22,
                owner_slot_index: None,
                guard_neutral_army_key: Some("neutral:west-mine".to_string()),
            },
            ObjectSeed {
                key: "mine:west-crystal".to_string(),
                object_slug: "crystal-mine".to_string(),
                x: 14,
                y: 30,
                owner_slot_index: None,
                guard_neutral_army_key: None,
            },
            ObjectSeed {
                key: "mine:east-gold".to_string(),
                object_slug: "gold-mine".to_string(),
                x: 35,
                y: 26,
                owner_slot_index: None,
                guard_neutral_army_key: Some("neutral:east-mine".to_string()),
            },
            ObjectSeed {
                key: "mine:east-crystal".to_string(),
                object_slug: "crystal-mine".to_string(),
                x: 33,
                y: 18,
                owner_slot_index: None,
                guard_neutral_army_key: None,
            },
        ],
        resource_piles: first_playable_resource_piles(),
        central_objectives: vec![
            ObjectSeed {
                key: "objective:north".to_string(),
                object_slug: "misery-beacon".to_string(),
                x: 24,
                y: 20,
                owner_slot_index: None,
                guard_neutral_army_key: Some("neutral:north-objective".to_string()),
            },
            ObjectSeed {
                key: "objective:south".to_string(),
                object_slug: "misery-beacon".to_string(),
                x: 24,
                y: 28,
                owner_slot_index: None,
                guard_neutral_army_key: Some("neutral:south-objective".to_string()),
            },
        ],
        neutral_armies: first_playable_neutral_armies(),
        walkthrough: FirstPlayableWalkthrough {
            steps: vec![
                WalkthroughStep {
                    step_key: "open-pickup".to_string(),
                    actor_slot_index: 0,
                    command_hint: "move_to_resource_pile".to_string(),
                    target_key: "pile:west-wood-1".to_string(),
                    expected_result: "west player gains wood for first build".to_string(),
                },
                WalkthroughStep {
                    step_key: "capture-nearby-mine".to_string(),
                    actor_slot_index: 0,
                    command_hint: "move_to_mine_and_fight_guard".to_string(),
                    target_key: "mine:west-gold".to_string(),
                    expected_result: "west player starts gold income".to_string(),
                },
                WalkthroughStep {
                    step_key: "first-build".to_string(),
                    actor_slot_index: 0,
                    command_hint: "build_town_structure".to_string(),
                    target_key: "building:freehold-training-yard".to_string(),
                    expected_result: "tier 1 recruitment is available".to_string(),
                },
                WalkthroughStep {
                    step_key: "first-recruit".to_string(),
                    actor_slot_index: 0,
                    command_hint: "recruit_units".to_string(),
                    target_key: "unit:mudhook-levy".to_string(),
                    expected_result: "starting champion can reinforce".to_string(),
                },
                WalkthroughStep {
                    step_key: "central-fight".to_string(),
                    actor_slot_index: 0,
                    command_hint: "move_to_central_objective".to_string(),
                    target_key: "objective:north".to_string(),
                    expected_result: "central guard battle starts".to_string(),
                },
                WalkthroughStep {
                    step_key: "town-capture".to_string(),
                    actor_slot_index: 0,
                    command_hint: "attack_enemy_town".to_string(),
                    target_key: "town:east".to_string(),
                    expected_result: "east town can be captured after battle aftermath".to_string(),
                },
                WalkthroughStep {
                    step_key: "victory-check".to_string(),
                    actor_slot_index: 0,
                    command_hint: "resolve_victory".to_string(),
                    target_key: "participant:east".to_string(),
                    expected_result: "west player wins once east has no town or active champion"
                        .to_string(),
                },
            ],
        },
    };
    scenario.scenario_hash = scenario.computed_scenario_hash();
    scenario
}

fn first_playable_factions(ruleset_id: &str) -> Vec<FactionContent> {
    vec![
        FactionContent {
            id: "faction:gutterborn-freehold".to_string(),
            ruleset_id: ruleset_id.to_string(),
            slug: "gutterborn-freehold".to_string(),
            name: "Gutterborn Freehold".to_string(),
            theme: Some("Roadside militia with cheap bodies and fast early claims.".to_string()),
            description: Some(
                "Wins by flooding the map before anyone can afford dignity.".to_string(),
            ),
            icon_key: Some("icon:faction:gutterborn".to_string()),
            banner_key: Some("banner:faction:gutterborn".to_string()),
            native_terrain: Some("grass".to_string()),
            trait_key: "scrap_claim_bonus".to_string(),
        },
        FactionContent {
            id: "faction:ashen-ledger".to_string(),
            ruleset_id: ruleset_id.to_string(),
            slug: "ashen-ledger".to_string(),
            name: "Ashen Ledger".to_string(),
            theme: Some("Disciplined collectors with stronger ranged pressure.".to_string()),
            description: Some(
                "Turns every fight into a balance sheet and every town into collateral."
                    .to_string(),
            ),
            icon_key: Some("icon:faction:ashen-ledger".to_string()),
            banner_key: Some("banner:faction:ashen-ledger".to_string()),
            native_terrain: Some("rubble".to_string()),
            trait_key: "ranged_pressure_bonus".to_string(),
        },
    ]
}

fn first_playable_champion_classes(ruleset_id: &str) -> Vec<ChampionClassContent> {
    vec![
        ChampionClassContent {
            id: "class:toll-broken-captain".to_string(),
            ruleset_id: ruleset_id.to_string(),
            faction_slug: Some("gutterborn-freehold".to_string()),
            slug: "toll-broken-captain".to_string(),
            name: "Toll-Broken Captain".to_string(),
            description: Some(
                "A practical commander built for early pickups and cheap recruits.".to_string(),
            ),
            portrait_key: Some("portrait:class:toll-broken-captain".to_string()),
            base_movement: 240,
            base_vision: 5,
        },
        ChampionClassContent {
            id: "class:ash-auditor".to_string(),
            ruleset_id: ruleset_id.to_string(),
            faction_slug: Some("ashen-ledger".to_string()),
            slug: "ash-auditor".to_string(),
            name: "Ash Auditor".to_string(),
            description: Some(
                "A precise leader with enough range support to punish sloppy advances.".to_string(),
            ),
            portrait_key: Some("portrait:class:ash-auditor".to_string()),
            base_movement: 240,
            base_vision: 5,
        },
    ]
}

fn first_playable_terrain(ruleset_id: &str) -> Vec<TerrainContent> {
    vec![
        terrain(ruleset_id, "grass", 1, "Sour Grass", 10, true),
        terrain(ruleset_id, "road", 2, "Rutted Road", 5, true),
        terrain(ruleset_id, "forest", 3, "Debtwood", 15, true),
        terrain(ruleset_id, "swamp", 4, "Audit Mire", 20, true),
        terrain(ruleset_id, "rubble", 5, "Cinder Rubble", 12, true),
        terrain(ruleset_id, "mountain", 6, "Broke Ridge", 255, false),
    ]
}

fn first_playable_units(ruleset_id: &str) -> Vec<UnitContent> {
    vec![
        unit(
            ruleset_id,
            Some("gutterborn-freehold"),
            "mudhook-levy",
            "Mudhook Levy",
            1,
            4,
            3,
            2,
            4,
            9,
            4,
            8,
            false,
            0,
            70,
            16,
            &[],
        ),
        unit(
            ruleset_id,
            Some("gutterborn-freehold"),
            "tollroad-skirmisher",
            "Tollroad Skirmisher",
            2,
            6,
            4,
            3,
            5,
            14,
            5,
            10,
            true,
            8,
            145,
            9,
            &["ranged"],
        ),
        unit(
            ruleset_id,
            Some("gutterborn-freehold"),
            "scrap-bulwark",
            "Scrap Bulwark",
            3,
            8,
            9,
            5,
            8,
            28,
            4,
            7,
            false,
            0,
            310,
            5,
            &["guarded"],
        ),
        unit(
            ruleset_id,
            Some("ashen-ledger"),
            "ash-scripter",
            "Ash Scripter",
            1,
            5,
            3,
            2,
            3,
            8,
            4,
            9,
            false,
            0,
            75,
            15,
            &[],
        ),
        unit(
            ruleset_id,
            Some("ashen-ledger"),
            "cinder-crossbow",
            "Cinder Crossbow",
            2,
            7,
            4,
            3,
            6,
            12,
            4,
            11,
            true,
            10,
            160,
            8,
            &["ranged"],
        ),
        unit(
            ruleset_id,
            Some("ashen-ledger"),
            "iron-bailiff",
            "Iron Bailiff",
            3,
            9,
            8,
            6,
            9,
            30,
            4,
            8,
            false,
            0,
            330,
            5,
            &["guarded"],
        ),
        unit(
            ruleset_id,
            None,
            "broken-pike",
            "Broken Pike",
            1,
            4,
            4,
            2,
            4,
            10,
            3,
            6,
            false,
            0,
            60,
            18,
            &[],
        ),
        unit(
            ruleset_id,
            None,
            "roadside-binder",
            "Roadside Binder",
            2,
            6,
            6,
            3,
            6,
            18,
            4,
            8,
            false,
            0,
            130,
            10,
            &["guarded"],
        ),
        unit(
            ruleset_id,
            None,
            "central-enforcer",
            "Central Enforcer",
            3,
            9,
            9,
            6,
            10,
            34,
            4,
            9,
            false,
            0,
            340,
            4,
            &["guarded"],
        ),
    ]
}

fn first_playable_buildings(ruleset_id: &str) -> Vec<BuildingContent> {
    vec![
        building(
            ruleset_id,
            None,
            "crumbling-hall",
            "Crumbling Hall",
            "hall",
            ResourceCost::zero(),
            &[],
            None,
            Some("town_hall_level_1"),
        ),
        building(
            ruleset_id,
            Some("gutterborn-freehold"),
            "freehold-training-yard",
            "Freehold Training Yard",
            "dwelling",
            ResourceCost {
                gold: 1_000,
                wood: 6,
                stone: 2,
                iron: 0,
                crystal: 0,
                ember: 0,
                aether: 0,
            },
            &["crumbling-hall"],
            Some("mudhook-levy"),
            None,
        ),
        building(
            ruleset_id,
            Some("gutterborn-freehold"),
            "skirmisher-stall",
            "Skirmisher Stall",
            "dwelling",
            ResourceCost {
                gold: 1_600,
                wood: 5,
                stone: 4,
                iron: 1,
                crystal: 0,
                ember: 0,
                aether: 0,
            },
            &["freehold-training-yard"],
            Some("tollroad-skirmisher"),
            None,
        ),
        building(
            ruleset_id,
            Some("gutterborn-freehold"),
            "scrap-bastion",
            "Scrap Bastion",
            "dwelling",
            ResourceCost {
                gold: 2_600,
                wood: 2,
                stone: 7,
                iron: 2,
                crystal: 0,
                ember: 0,
                aether: 0,
            },
            &["skirmisher-stall"],
            Some("scrap-bulwark"),
            None,
        ),
        building(
            ruleset_id,
            Some("ashen-ledger"),
            "scripter-office",
            "Scripter Office",
            "dwelling",
            ResourceCost {
                gold: 1_050,
                wood: 4,
                stone: 4,
                iron: 0,
                crystal: 0,
                ember: 0,
                aether: 0,
            },
            &["crumbling-hall"],
            Some("ash-scripter"),
            None,
        ),
        building(
            ruleset_id,
            Some("ashen-ledger"),
            "cinder-range",
            "Cinder Range",
            "dwelling",
            ResourceCost {
                gold: 1_700,
                wood: 6,
                stone: 3,
                iron: 1,
                crystal: 0,
                ember: 1,
                aether: 0,
            },
            &["scripter-office"],
            Some("cinder-crossbow"),
            None,
        ),
        building(
            ruleset_id,
            Some("ashen-ledger"),
            "bailiff-forge",
            "Bailiff Forge",
            "dwelling",
            ResourceCost {
                gold: 2_750,
                wood: 2,
                stone: 6,
                iron: 3,
                crystal: 0,
                ember: 1,
                aether: 0,
            },
            &["cinder-range"],
            Some("iron-bailiff"),
            None,
        ),
        building(
            ruleset_id,
            None,
            "weighhouse",
            "Weighhouse",
            "income",
            ResourceCost {
                gold: 1_250,
                wood: 2,
                stone: 4,
                iron: 0,
                crystal: 0,
                ember: 0,
                aether: 0,
            },
            &["crumbling-hall"],
            None,
            Some("town_income_gold_250"),
        ),
    ]
}

fn first_playable_artifacts(ruleset_id: &str) -> Vec<ArtifactContent> {
    vec![ArtifactContent {
        id: "artifact:bent-banner".to_string(),
        ruleset_id: ruleset_id.to_string(),
        slug: "bent-banner".to_string(),
        name: "Bent Banner".to_string(),
        description: Some(
            "A v1-safe minor might artifact reserved for later pickup placement.".to_string(),
        ),
        icon_key: Some("icon:artifact:bent-banner".to_string()),
        slot: "banner".to_string(),
        rarity: "common".to_string(),
        effect_key: "minor_might_plus_1".to_string(),
    }]
}

fn first_playable_map_objects(ruleset_id: &str) -> Vec<MapObjectContent> {
    vec![
        map_object(
            ruleset_id,
            "gold-mine",
            "Gold Mine",
            "mine",
            true,
            "capture_gold_income",
            "owner_income",
        ),
        map_object(
            ruleset_id,
            "crystal-mine",
            "Crystal Mine",
            "mine",
            true,
            "capture_crystal_income",
            "owner_income",
        ),
        map_object(
            ruleset_id,
            "resource-pile",
            "Resource Pile",
            "resource_pile",
            false,
            "grant_resource_reward",
            "once",
        ),
        map_object(
            ruleset_id,
            "misery-beacon",
            "Misery Beacon",
            "central_objective",
            true,
            "score_central_objective",
            "owner_score",
        ),
    ]
}

fn first_playable_map() -> HandAuthoredMap {
    HandAuthoredMap {
        width: FIRST_PLAYABLE_MAP_WIDTH,
        height: FIRST_PLAYABLE_MAP_HEIGHT,
        chunk_size: FIRST_PLAYABLE_CHUNK_SIZE,
        default_terrain_key: "grass".to_string(),
        terrain_patches: vec![
            TerrainPatch {
                terrain_key: "forest".to_string(),
                x: 2,
                y: 5,
                width: 12,
                height: 8,
            },
            TerrainPatch {
                terrain_key: "forest".to_string(),
                x: 34,
                y: 35,
                width: 12,
                height: 8,
            },
            TerrainPatch {
                terrain_key: "swamp".to_string(),
                x: 20,
                y: 18,
                width: 8,
                height: 13,
            },
            TerrainPatch {
                terrain_key: "rubble".to_string(),
                x: 18,
                y: 0,
                width: 12,
                height: 8,
            },
            TerrainPatch {
                terrain_key: "rubble".to_string(),
                x: 18,
                y: 40,
                width: 12,
                height: 8,
            },
            TerrainPatch {
                terrain_key: "mountain".to_string(),
                x: 0,
                y: 0,
                width: 48,
                height: 1,
            },
            TerrainPatch {
                terrain_key: "mountain".to_string(),
                x: 0,
                y: 47,
                width: 48,
                height: 1,
            },
        ],
        road_paths: vec![
            RoadPath {
                key: "road:west-town-gold-center".to_string(),
                waypoints: coords(&[(6, 24), (12, 22), (18, 22), (24, 20)]),
            },
            RoadPath {
                key: "road:east-town-gold-center".to_string(),
                waypoints: coords(&[(41, 24), (35, 26), (30, 24), (24, 20)]),
            },
            RoadPath {
                key: "road:west-town-crystal-south".to_string(),
                waypoints: coords(&[(6, 24), (14, 30), (20, 28), (24, 28)]),
            },
            RoadPath {
                key: "road:east-town-crystal-south".to_string(),
                waypoints: coords(&[(41, 24), (33, 18), (28, 20), (24, 28)]),
            },
        ],
    }
}

fn first_playable_resource_piles() -> Vec<ResourcePileSeed> {
    vec![
        pile(
            "pile:west-wood-1",
            9,
            23,
            ResourceCost {
                wood: 5,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:west-gold-1",
            10,
            27,
            ResourceCost {
                gold: 1_000,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:west-stone-1",
            13,
            18,
            ResourceCost {
                stone: 4,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:west-iron-1",
            16,
            31,
            ResourceCost {
                iron: 2,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:west-ember-1",
            19,
            20,
            ResourceCost {
                ember: 1,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:west-aether-1",
            21,
            30,
            ResourceCost {
                aether: 1,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-wood-1",
            38,
            25,
            ResourceCost {
                wood: 5,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-gold-1",
            37,
            21,
            ResourceCost {
                gold: 1_000,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-stone-1",
            34,
            30,
            ResourceCost {
                stone: 4,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-iron-1",
            31,
            17,
            ResourceCost {
                iron: 2,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-ember-1",
            28,
            28,
            ResourceCost {
                ember: 1,
                ..ResourceCost::zero()
            },
        ),
        pile(
            "pile:east-aether-1",
            26,
            18,
            ResourceCost {
                aether: 1,
                ..ResourceCost::zero()
            },
        ),
    ]
}

fn first_playable_neutral_armies() -> Vec<NeutralArmySeed> {
    vec![
        neutral("neutral:west-mine", "early", 12, 22, &[("broken-pike", 12)]),
        neutral("neutral:east-mine", "early", 35, 26, &[("broken-pike", 12)]),
        neutral(
            "neutral:west-road",
            "roadblock",
            18,
            22,
            &[("broken-pike", 16), ("roadside-binder", 4)],
        ),
        neutral(
            "neutral:east-road",
            "roadblock",
            30,
            24,
            &[("broken-pike", 16), ("roadside-binder", 4)],
        ),
        neutral(
            "neutral:north-objective",
            "central",
            24,
            20,
            &[("roadside-binder", 8), ("central-enforcer", 3)],
        ),
        neutral(
            "neutral:south-objective",
            "central",
            24,
            28,
            &[("roadside-binder", 8), ("central-enforcer", 3)],
        ),
    ]
}

fn first_playable_asset_keys() -> Vec<String> {
    let mut keys = [
        "banner:faction:ashen-ledger",
        "banner:faction:gutterborn",
        "icon:artifact:bent-banner",
        "icon:building:bailiff-forge",
        "icon:building:cinder-range",
        "icon:building:crumbling-hall",
        "icon:building:freehold-training-yard",
        "icon:building:scripter-office",
        "icon:building:scrap-bastion",
        "icon:building:skirmisher-stall",
        "icon:building:weighhouse",
        "icon:faction:ashen-ledger",
        "icon:faction:gutterborn",
        "icon:object:crystal-mine",
        "icon:object:gold-mine",
        "icon:object:misery-beacon",
        "icon:object:resource-pile",
        "portrait:class:ash-auditor",
        "portrait:class:toll-broken-captain",
        "sprite:object:crystal-mine",
        "sprite:object:gold-mine",
        "sprite:object:misery-beacon",
        "sprite:object:resource-pile",
        "sprite:terrain:forest",
        "sprite:terrain:grass",
        "sprite:terrain:mountain",
        "sprite:terrain:road",
        "sprite:terrain:rubble",
        "sprite:terrain:swamp",
        "sprite:unit:ash-scripter",
        "sprite:unit:broken-pike",
        "sprite:unit:central-enforcer",
        "sprite:unit:cinder-crossbow",
        "sprite:unit:iron-bailiff",
        "sprite:unit:mudhook-levy",
        "sprite:unit:roadside-binder",
        "sprite:unit:scrap-bulwark",
        "sprite:unit:tollroad-skirmisher",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

fn terrain(
    ruleset_id: &str,
    terrain_key: &str,
    terrain_code: u8,
    name: &str,
    movement_cost: u16,
    passable: bool,
) -> TerrainContent {
    TerrainContent {
        id: format!("terrain:{terrain_key}"),
        ruleset_id: ruleset_id.to_string(),
        terrain_key: terrain_key.to_string(),
        terrain_code,
        name: name.to_string(),
        movement_cost,
        passable,
        sprite_key: Some(format!("sprite:terrain:{terrain_key}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn unit(
    ruleset_id: &str,
    faction_slug: Option<&str>,
    slug: &str,
    name: &str,
    tier: u8,
    attack: i16,
    defense: i16,
    damage_min: u16,
    damage_max: u16,
    max_hp: u16,
    speed: u8,
    initiative: u8,
    ranged: bool,
    shots: u16,
    gold_cost: u32,
    weekly_growth: u16,
    ability_keys: &[&str],
) -> UnitContent {
    UnitContent {
        id: format!("unit:{slug}"),
        ruleset_id: ruleset_id.to_string(),
        faction_slug: faction_slug.map(str::to_string),
        slug: slug.to_string(),
        name: name.to_string(),
        description: Some(format!("Tier {tier} first playable unit.")),
        sprite_key: Some(format!("sprite:unit:{slug}")),
        icon_key: Some(format!("sprite:unit:{slug}")),
        animation_key: None,
        tier,
        attack,
        defense,
        damage_min,
        damage_max,
        max_hp,
        speed,
        initiative,
        ranged,
        flying: false,
        shots,
        cost: ResourceCost {
            gold: gold_cost,
            ..ResourceCost::zero()
        },
        weekly_growth,
        ability_keys: ability_keys.iter().map(|key| (*key).to_string()).collect(),
    }
}

fn building(
    ruleset_id: &str,
    faction_slug: Option<&str>,
    slug: &str,
    name: &str,
    building_type: &str,
    cost: ResourceCost,
    requires_building_slugs: &[&str],
    unlocks_unit_slug: Option<&str>,
    effect_key: Option<&str>,
) -> BuildingContent {
    BuildingContent {
        id: format!("building:{slug}"),
        ruleset_id: ruleset_id.to_string(),
        faction_slug: faction_slug.map(str::to_string),
        slug: slug.to_string(),
        name: name.to_string(),
        description: Some(format!("First playable {building_type} building.")),
        icon_key: Some(format!("icon:building:{slug}")),
        building_type: building_type.to_string(),
        cost,
        requires_building_slugs: requires_building_slugs
            .iter()
            .map(|slug| (*slug).to_string())
            .collect(),
        unlocks_unit_slug: unlocks_unit_slug.map(str::to_string),
        effect_key: effect_key.map(str::to_string),
    }
}

fn map_object(
    ruleset_id: &str,
    slug: &str,
    name: &str,
    object_type: &str,
    blocking: bool,
    interaction_key: &str,
    refresh_rule: &str,
) -> MapObjectContent {
    MapObjectContent {
        id: format!("object:{slug}"),
        ruleset_id: ruleset_id.to_string(),
        slug: slug.to_string(),
        name: name.to_string(),
        description: Some(format!("First playable {object_type} object.")),
        sprite_key: Some(format!("sprite:object:{slug}")),
        icon_key: Some(format!("icon:object:{slug}")),
        object_type: object_type.to_string(),
        footprint_w: 1,
        footprint_h: 1,
        blocking,
        interaction_key: interaction_key.to_string(),
        refresh_rule: refresh_rule.to_string(),
    }
}

fn coords(values: &[(u16, u16)]) -> Vec<TileCoord> {
    values
        .iter()
        .map(|(x, y)| TileCoord { x: *x, y: *y })
        .collect()
}

fn pile(key: &str, x: u16, y: u16, reward: ResourceCost) -> ResourcePileSeed {
    ResourcePileSeed {
        key: key.to_string(),
        object_slug: "resource-pile".to_string(),
        x,
        y,
        reward,
    }
}

fn neutral(
    key: &str,
    strength_band: &str,
    x: u16,
    y: u16,
    stacks: &[(&str, u16)],
) -> NeutralArmySeed {
    NeutralArmySeed {
        key: key.to_string(),
        strength_band: strength_band.to_string(),
        x,
        y,
        stacks: stacks
            .iter()
            .enumerate()
            .map(|(index, (unit_slug, quantity))| ArmyStackSeed {
                unit_slug: (*unit_slug).to_string(),
                quantity: *quantity,
                slot_index: index as u8,
            })
            .collect(),
    }
}

fn compute_content_manifest_hash(manifest: &ContentManifest) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "content_schema", "domm.content.v1");
    hash_text(&mut hasher, "ruleset.id", &manifest.ruleset.id);
    hash_text(&mut hasher, "ruleset.slug", &manifest.ruleset.slug);
    hash_u32(&mut hasher, "ruleset.version", manifest.ruleset.version);
    hash_text(&mut hasher, "ruleset.name", &manifest.ruleset.name);
    hash_optional_text(
        &mut hasher,
        "ruleset.description",
        manifest.ruleset.description.as_deref(),
    );
    hash_u16(&mut hasher, "ruleset.map_width", manifest.ruleset.map_width);
    hash_u16(
        &mut hasher,
        "ruleset.map_height",
        manifest.ruleset.map_height,
    );
    hash_u8(
        &mut hasher,
        "ruleset.chunk_size",
        manifest.ruleset.chunk_size,
    );
    hash_u8(
        &mut hasher,
        "ruleset.player_count",
        manifest.ruleset.player_count,
    );
    hash_u32(&mut hasher, "ruleset.max_turns", manifest.ruleset.max_turns);
    hash_u32(
        &mut hasher,
        "ruleset.turn_duration_ms",
        manifest.ruleset.turn_duration_ms,
    );

    for item in &manifest.factions {
        hash_text(&mut hasher, "faction.id", &item.id);
        hash_text(&mut hasher, "faction.slug", &item.slug);
        hash_text(&mut hasher, "faction.name", &item.name);
        hash_optional_text(&mut hasher, "faction.theme", item.theme.as_deref());
        hash_optional_text(
            &mut hasher,
            "faction.description",
            item.description.as_deref(),
        );
        hash_optional_text(&mut hasher, "faction.icon_key", item.icon_key.as_deref());
        hash_optional_text(
            &mut hasher,
            "faction.banner_key",
            item.banner_key.as_deref(),
        );
        hash_optional_text(
            &mut hasher,
            "faction.native_terrain",
            item.native_terrain.as_deref(),
        );
        hash_text(&mut hasher, "faction.trait_key", &item.trait_key);
    }

    for item in &manifest.champion_classes {
        hash_text(&mut hasher, "class.id", &item.id);
        hash_optional_text(
            &mut hasher,
            "class.faction_slug",
            item.faction_slug.as_deref(),
        );
        hash_text(&mut hasher, "class.slug", &item.slug);
        hash_text(&mut hasher, "class.name", &item.name);
        hash_optional_text(
            &mut hasher,
            "class.description",
            item.description.as_deref(),
        );
        hash_optional_text(
            &mut hasher,
            "class.portrait_key",
            item.portrait_key.as_deref(),
        );
        hash_u16(&mut hasher, "class.base_movement", item.base_movement);
        hash_u8(&mut hasher, "class.base_vision", item.base_vision);
    }

    for item in &manifest.terrain {
        hash_text(&mut hasher, "terrain.key", &item.terrain_key);
        hash_u8(&mut hasher, "terrain.code", item.terrain_code);
        hash_text(&mut hasher, "terrain.name", &item.name);
        hash_u16(&mut hasher, "terrain.movement_cost", item.movement_cost);
        hash_bool(&mut hasher, "terrain.passable", item.passable);
        hash_optional_text(
            &mut hasher,
            "terrain.sprite_key",
            item.sprite_key.as_deref(),
        );
    }

    for item in &manifest.units {
        hash_unit(&mut hasher, item);
    }
    for item in &manifest.buildings {
        hash_building(&mut hasher, item);
    }
    for item in &manifest.spells {
        hash_spell(&mut hasher, item);
    }
    for item in &manifest.artifacts {
        hash_artifact(&mut hasher, item);
    }
    for item in &manifest.map_objects {
        hash_map_object(&mut hasher, item);
    }
    for key in &manifest.asset_keys {
        hash_text(&mut hasher, "asset_key", key);
    }

    to_hex(&hasher.finalize())
}

fn compute_scenario_hash(scenario: &FirstPlayableScenario) -> String {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "scenario_schema", "domm.scenario.v1");
    hash_text(&mut hasher, "scenario_key", &scenario.scenario_key);
    hash_text(&mut hasher, "scenario_seed", &scenario.scenario_seed);
    hash_text(&mut hasher, "ruleset_slug", &scenario.ruleset_slug);
    hash_u32(&mut hasher, "ruleset_version", scenario.ruleset_version);
    hash_u16(&mut hasher, "map.width", scenario.map.width);
    hash_u16(&mut hasher, "map.height", scenario.map.height);
    hash_u8(&mut hasher, "map.chunk_size", scenario.map.chunk_size);
    hash_text(
        &mut hasher,
        "map.default_terrain",
        &scenario.map.default_terrain_key,
    );
    for patch in &scenario.map.terrain_patches {
        hash_text(&mut hasher, "patch.terrain", &patch.terrain_key);
        hash_u16(&mut hasher, "patch.x", patch.x);
        hash_u16(&mut hasher, "patch.y", patch.y);
        hash_u16(&mut hasher, "patch.width", patch.width);
        hash_u16(&mut hasher, "patch.height", patch.height);
    }
    for road in &scenario.map.road_paths {
        hash_text(&mut hasher, "road.key", &road.key);
        for coord in &road.waypoints {
            hash_coord(&mut hasher, "road.coord", coord);
        }
    }
    hash_resource_cost(
        &mut hasher,
        "starting.resources",
        &scenario.starting_state.resources,
    );
    hash_u8(
        &mut hasher,
        "starting.champion_level",
        scenario.starting_state.champion_level,
    );
    hash_u16(
        &mut hasher,
        "starting.champion_movement",
        scenario.starting_state.champion_movement,
    );
    hash_u8(
        &mut hasher,
        "starting.champion_vision",
        scenario.starting_state.champion_vision,
    );
    hash_u8(
        &mut hasher,
        "starting.town_hall_level",
        scenario.starting_state.town_hall_level,
    );
    for start in &scenario.starts {
        hash_u8(&mut hasher, "start.slot", start.slot_index);
        hash_text(&mut hasher, "start.faction", &start.faction_slug);
        hash_text(&mut hasher, "start.class", &start.champion_class_slug);
        hash_text(&mut hasher, "start.town_key", &start.town_key);
        hash_coord(
            &mut hasher,
            "start.town_coord",
            &TileCoord {
                x: start.town_x,
                y: start.town_y,
            },
        );
        hash_text(&mut hasher, "start.champion_key", &start.champion_key);
        hash_coord(
            &mut hasher,
            "start.champion_coord",
            &TileCoord {
                x: start.champion_x,
                y: start.champion_y,
            },
        );
        for stack in &start.starting_army_stacks {
            hash_stack(&mut hasher, "start.stack", stack);
        }
    }
    for object in &scenario.mines {
        hash_object_seed(&mut hasher, "mine", object);
    }
    for pile in &scenario.resource_piles {
        hash_text(&mut hasher, "pile.key", &pile.key);
        hash_text(&mut hasher, "pile.slug", &pile.object_slug);
        hash_u16(&mut hasher, "pile.x", pile.x);
        hash_u16(&mut hasher, "pile.y", pile.y);
        hash_resource_cost(&mut hasher, "pile.reward", &pile.reward);
    }
    for object in &scenario.central_objectives {
        hash_object_seed(&mut hasher, "objective", object);
    }
    for neutral in &scenario.neutral_armies {
        hash_text(&mut hasher, "neutral.key", &neutral.key);
        hash_text(&mut hasher, "neutral.band", &neutral.strength_band);
        hash_u16(&mut hasher, "neutral.x", neutral.x);
        hash_u16(&mut hasher, "neutral.y", neutral.y);
        for stack in &neutral.stacks {
            hash_stack(&mut hasher, "neutral.stack", stack);
        }
    }
    for step in &scenario.walkthrough.steps {
        hash_text(&mut hasher, "walk.step", &step.step_key);
        hash_u8(&mut hasher, "walk.actor", step.actor_slot_index);
        hash_text(&mut hasher, "walk.command", &step.command_hint);
        hash_text(&mut hasher, "walk.target", &step.target_key);
        hash_text(&mut hasher, "walk.result", &step.expected_result);
    }
    to_hex(&hasher.finalize())
}

fn hash_unit(hasher: &mut Sha256, item: &UnitContent) {
    hash_text(hasher, "unit.id", &item.id);
    hash_optional_text(hasher, "unit.faction_slug", item.faction_slug.as_deref());
    hash_text(hasher, "unit.slug", &item.slug);
    hash_text(hasher, "unit.name", &item.name);
    hash_optional_text(hasher, "unit.description", item.description.as_deref());
    hash_optional_text(hasher, "unit.sprite_key", item.sprite_key.as_deref());
    hash_optional_text(hasher, "unit.icon_key", item.icon_key.as_deref());
    hash_u8(hasher, "unit.tier", item.tier);
    hash_i16(hasher, "unit.attack", item.attack);
    hash_i16(hasher, "unit.defense", item.defense);
    hash_u16(hasher, "unit.damage_min", item.damage_min);
    hash_u16(hasher, "unit.damage_max", item.damage_max);
    hash_u16(hasher, "unit.max_hp", item.max_hp);
    hash_u8(hasher, "unit.speed", item.speed);
    hash_u8(hasher, "unit.initiative", item.initiative);
    hash_bool(hasher, "unit.ranged", item.ranged);
    hash_bool(hasher, "unit.flying", item.flying);
    hash_u16(hasher, "unit.shots", item.shots);
    hash_resource_cost(hasher, "unit.cost", &item.cost);
    hash_u16(hasher, "unit.weekly_growth", item.weekly_growth);
    for key in &item.ability_keys {
        hash_text(hasher, "unit.ability", key);
    }
}

fn hash_building(hasher: &mut Sha256, item: &BuildingContent) {
    hash_text(hasher, "building.id", &item.id);
    hash_optional_text(
        hasher,
        "building.faction_slug",
        item.faction_slug.as_deref(),
    );
    hash_text(hasher, "building.slug", &item.slug);
    hash_text(hasher, "building.name", &item.name);
    hash_optional_text(hasher, "building.description", item.description.as_deref());
    hash_optional_text(hasher, "building.icon_key", item.icon_key.as_deref());
    hash_text(hasher, "building.type", &item.building_type);
    hash_resource_cost(hasher, "building.cost", &item.cost);
    for slug in &item.requires_building_slugs {
        hash_text(hasher, "building.requires", slug);
    }
    hash_optional_text(
        hasher,
        "building.unlocks_unit",
        item.unlocks_unit_slug.as_deref(),
    );
    hash_optional_text(hasher, "building.effect", item.effect_key.as_deref());
}

fn hash_spell(hasher: &mut Sha256, item: &SpellContent) {
    hash_text(hasher, "spell.id", &item.id);
    hash_text(hasher, "spell.slug", &item.slug);
    hash_text(hasher, "spell.name", &item.name);
    hash_text(hasher, "spell.school", &item.school);
    hash_u8(hasher, "spell.level", item.level);
    hash_u16(hasher, "spell.mana_cost", item.mana_cost);
    hash_text(hasher, "spell.target", &item.target_type);
    hash_text(hasher, "spell.effect", &item.effect_key);
    hash_u8(hasher, "spell.duration", item.duration_rounds);
}

fn hash_artifact(hasher: &mut Sha256, item: &ArtifactContent) {
    hash_text(hasher, "artifact.id", &item.id);
    hash_text(hasher, "artifact.slug", &item.slug);
    hash_text(hasher, "artifact.name", &item.name);
    hash_optional_text(hasher, "artifact.description", item.description.as_deref());
    hash_optional_text(hasher, "artifact.icon_key", item.icon_key.as_deref());
    hash_text(hasher, "artifact.slot", &item.slot);
    hash_text(hasher, "artifact.rarity", &item.rarity);
    hash_text(hasher, "artifact.effect", &item.effect_key);
}

fn hash_map_object(hasher: &mut Sha256, item: &MapObjectContent) {
    hash_text(hasher, "object.id", &item.id);
    hash_text(hasher, "object.slug", &item.slug);
    hash_text(hasher, "object.name", &item.name);
    hash_optional_text(hasher, "object.description", item.description.as_deref());
    hash_optional_text(hasher, "object.sprite_key", item.sprite_key.as_deref());
    hash_optional_text(hasher, "object.icon_key", item.icon_key.as_deref());
    hash_text(hasher, "object.type", &item.object_type);
    hash_u8(hasher, "object.footprint_w", item.footprint_w);
    hash_u8(hasher, "object.footprint_h", item.footprint_h);
    hash_bool(hasher, "object.blocking", item.blocking);
    hash_text(hasher, "object.interaction", &item.interaction_key);
    hash_text(hasher, "object.refresh", &item.refresh_rule);
}

fn hash_object_seed(hasher: &mut Sha256, label: &str, item: &ObjectSeed) {
    hash_text(hasher, &format!("{label}.key"), &item.key);
    hash_text(hasher, &format!("{label}.slug"), &item.object_slug);
    hash_u16(hasher, &format!("{label}.x"), item.x);
    hash_u16(hasher, &format!("{label}.y"), item.y);
    hash_optional_u8(hasher, &format!("{label}.owner"), item.owner_slot_index);
    hash_optional_text(
        hasher,
        &format!("{label}.guard"),
        item.guard_neutral_army_key.as_deref(),
    );
}

fn hash_stack(hasher: &mut Sha256, label: &str, item: &ArmyStackSeed) {
    hash_text(hasher, &format!("{label}.unit"), &item.unit_slug);
    hash_u16(hasher, &format!("{label}.quantity"), item.quantity);
    hash_u8(hasher, &format!("{label}.slot"), item.slot_index);
}

fn hash_coord(hasher: &mut Sha256, label: &str, coord: &TileCoord) {
    hash_u16(hasher, &format!("{label}.x"), coord.x);
    hash_u16(hasher, &format!("{label}.y"), coord.y);
}

fn hash_resource_cost(hasher: &mut Sha256, label: &str, cost: &ResourceCost) {
    hash_u32(hasher, &format!("{label}.gold"), cost.gold);
    hash_u32(hasher, &format!("{label}.wood"), cost.wood);
    hash_u32(hasher, &format!("{label}.stone"), cost.stone);
    hash_u32(hasher, &format!("{label}.iron"), cost.iron);
    hash_u32(hasher, &format!("{label}.crystal"), cost.crystal);
    hash_u32(hasher, &format!("{label}.ember"), cost.ember);
    hash_u32(hasher, &format!("{label}.aether"), cost.aether);
}

fn hash_optional_text(hasher: &mut Sha256, label: &str, value: Option<&str>) {
    match value {
        Some(value) => hash_text(hasher, label, value),
        None => hash_text(hasher, label, "<none>"),
    }
}

fn hash_optional_u8(hasher: &mut Sha256, label: &str, value: Option<u8>) {
    match value {
        Some(value) => hash_u8(hasher, label, value),
        None => hash_text(hasher, label, "<none>"),
    }
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hash_bytes(hasher, label, value.as_bytes());
}

fn hash_bool(hasher: &mut Sha256, label: &str, value: bool) {
    hash_bytes(hasher, label, &[u8::from(value)]);
}

fn hash_i16(hasher: &mut Sha256, label: &str, value: i16) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_u8(hasher: &mut Sha256, label: &str, value: u8) {
    hash_bytes(hasher, label, &[value]);
}

fn hash_u16(hasher: &mut Sha256, label: &str, value: u16) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_u32(hasher: &mut Sha256, label: &str, value: u32) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_bytes(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update(b":");
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value);
    hasher.update(b"\n");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use candid::{Decode, Encode};

    use super::{
        ContentManifest, FIRST_PLAYABLE_CHUNK_SIZE, FIRST_PLAYABLE_MAP_HEIGHT,
        FIRST_PLAYABLE_MAP_WIDTH, FIRST_PLAYABLE_MAX_TURNS, FIRST_PLAYABLE_PLAYER_COUNT,
        FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION, FirstPlayableScenario,
        first_playable_content_manifest, first_playable_scenario, get_content_manifest,
    };

    #[test]
    fn content_manifest_loads_by_ruleset_and_has_stable_hash() {
        let manifest =
            get_content_manifest(FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION)
                .expect("first playable ruleset should load");

        assert_eq!(manifest.ruleset.slug, FIRST_PLAYABLE_RULESET_SLUG);
        assert_eq!(manifest.ruleset.version, FIRST_PLAYABLE_RULESET_VERSION);
        assert_eq!(
            manifest.ruleset.content_manifest_hash,
            manifest.computed_content_hash()
        );
        assert_eq!(
            manifest.ruleset.content_manifest_hash,
            "915464be3bcad6ec19d5fbf0a891afa20c9dcb94a789fcf84dc682fc8802a799"
        );
        assert_eq!(manifest.ruleset.content_manifest_hash.len(), 64);
        assert!(get_content_manifest("missing", FIRST_PLAYABLE_RULESET_VERSION).is_none());
    }

    #[test]
    fn manifest_contains_first_playable_surface() {
        let manifest = first_playable_content_manifest();

        assert_eq!(manifest.factions.len(), 2);
        assert_eq!(manifest.champion_classes.len(), 2);
        assert_eq!(manifest.terrain.len(), 6);
        assert_eq!(manifest.units.len(), 9);
        assert_eq!(manifest.buildings.len(), 8);
        assert!(manifest.spells.is_empty());
        assert_eq!(manifest.artifacts.len(), 1);
        assert_eq!(manifest.map_objects.len(), 4);
        assert!(!manifest.asset_keys.is_empty());
        assert_sorted_unique(&manifest.asset_keys);
    }

    #[test]
    fn definition_lookup_finds_core_walkthrough_content() {
        let manifest = first_playable_content_manifest();

        assert_eq!(
            manifest
                .faction("gutterborn-freehold")
                .expect("faction should exist")
                .trait_key,
            "scrap_claim_bonus"
        );
        assert_eq!(
            manifest
                .champion_class("ash-auditor")
                .expect("class should exist")
                .base_movement,
            240
        );
        assert_eq!(
            manifest
                .terrain("road")
                .expect("road should exist")
                .movement_cost,
            5
        );
        assert_eq!(
            manifest
                .unit("cinder-crossbow")
                .expect("unit should exist")
                .shots,
            10
        );
        assert_eq!(
            manifest
                .building("freehold-training-yard")
                .expect("building should exist")
                .unlocks_unit_slug,
            Some("mudhook-levy".to_string())
        );
        assert_eq!(
            manifest
                .map_object("gold-mine")
                .expect("object should exist")
                .interaction_key,
            "capture_gold_income"
        );
    }

    #[test]
    fn first_playable_numeric_caps_match_product_scope() {
        let manifest = first_playable_content_manifest();
        let scenario = first_playable_scenario();

        assert_eq!(manifest.ruleset.map_width, FIRST_PLAYABLE_MAP_WIDTH);
        assert_eq!(manifest.ruleset.map_height, FIRST_PLAYABLE_MAP_HEIGHT);
        assert_eq!(manifest.ruleset.chunk_size, FIRST_PLAYABLE_CHUNK_SIZE);
        assert_eq!(manifest.ruleset.max_turns, FIRST_PLAYABLE_MAX_TURNS);
        assert_eq!(manifest.ruleset.player_count, FIRST_PLAYABLE_PLAYER_COUNT);
        assert_eq!(scenario.starts.len(), 2);
        assert_eq!(scenario.mines.len(), 4);
        assert_eq!(scenario.resource_piles.len(), 12);
        assert_eq!(scenario.neutral_armies.len(), 6);
        assert_eq!(scenario.central_objectives.len(), 2);
        assert_eq!(scenario.starting_state.resources.gold, 10_000);
        assert_eq!(scenario.starting_state.champion_movement, 240);
        assert_eq!(scenario.starting_state.champion_vision, 5);
        assert_eq!(scenario.starting_state.town_hall_level, 1);
        assert_eq!(scenario.map.width * scenario.map.height, 2_304);
    }

    #[test]
    fn first_playable_scenario_references_existing_content() {
        let manifest = first_playable_content_manifest();
        let scenario = first_playable_scenario();

        for start in &scenario.starts {
            assert!(manifest.faction(&start.faction_slug).is_some());
            assert!(
                manifest
                    .champion_class(&start.champion_class_slug)
                    .is_some()
            );
            assert_eq!(start.starting_army_stacks.len(), 2);
            let mut tiers = start
                .starting_army_stacks
                .iter()
                .map(|stack| {
                    let unit = manifest
                        .unit(&stack.unit_slug)
                        .expect("start stack unit should exist");
                    assert_eq!(
                        unit.faction_slug.as_deref(),
                        Some(start.faction_slug.as_str())
                    );
                    assert!(stack.quantity > 0);
                    unit.tier
                })
                .collect::<Vec<_>>();
            tiers.sort_unstable();
            assert_eq!(tiers, vec![1, 2]);
            assert_in_bounds(start.town_x, start.town_y, &scenario);
            assert_in_bounds(start.champion_x, start.champion_y, &scenario);
        }

        for mine in &scenario.mines {
            assert!(manifest.map_object(&mine.object_slug).is_some());
            assert_in_bounds(mine.x, mine.y, &scenario);
        }
        for pile in &scenario.resource_piles {
            assert!(manifest.map_object(&pile.object_slug).is_some());
            assert_in_bounds(pile.x, pile.y, &scenario);
        }
        for objective in &scenario.central_objectives {
            assert!(manifest.map_object(&objective.object_slug).is_some());
            assert_in_bounds(objective.x, objective.y, &scenario);
        }
        for neutral in &scenario.neutral_armies {
            assert_in_bounds(neutral.x, neutral.y, &scenario);
            for stack in &neutral.stacks {
                assert!(manifest.unit(&stack.unit_slug).is_some());
            }
        }
    }

    #[test]
    fn first_playable_walkthrough_targets_exist() {
        let manifest = first_playable_content_manifest();
        let scenario = first_playable_scenario();
        let target_keys = scenario_target_keys(&scenario, &manifest);

        for step in &scenario.walkthrough.steps {
            assert!(
                target_keys.contains(&step.target_key),
                "{} targets missing key {}",
                step.step_key,
                step.target_key
            );
            assert!(step.actor_slot_index < FIRST_PLAYABLE_PLAYER_COUNT);
        }
    }

    #[test]
    fn scenario_hash_and_candid_roundtrip_are_stable() {
        let manifest = first_playable_content_manifest();
        let scenario = first_playable_scenario();
        let encoded_manifest = Encode!(&manifest).expect("manifest should encode");
        let decoded_manifest =
            Decode!(&encoded_manifest, ContentManifest).expect("manifest should decode");
        let encoded_scenario = Encode!(&scenario).expect("scenario should encode");
        let decoded_scenario =
            Decode!(&encoded_scenario, FirstPlayableScenario).expect("scenario should decode");

        assert_eq!(decoded_manifest, manifest);
        assert_eq!(decoded_scenario, scenario);
        assert_eq!(scenario.scenario_hash, scenario.computed_scenario_hash());
        assert_eq!(
            scenario.scenario_hash,
            "f510b2f7196a53644efc9a4aa092512cbb845a822756dfb943420cda899b6979"
        );
    }

    #[test]
    fn unit_tiers_and_building_unlocks_stay_within_v1_caps() {
        let manifest = first_playable_content_manifest();

        for unit in &manifest.units {
            assert!((1..=3).contains(&unit.tier));
            assert!(unit.damage_min <= unit.damage_max);
            assert!(unit.max_hp > 0);
            assert!(unit.weekly_growth > 0);
            if unit.ranged {
                assert!(unit.shots > 0);
            }
        }

        for faction in ["gutterborn-freehold", "ashen-ledger"] {
            let faction_units = manifest
                .units
                .iter()
                .filter(|unit| unit.faction_slug.as_deref() == Some(faction))
                .collect::<Vec<_>>();
            assert_eq!(faction_units.len(), 3);
        }

        for building in &manifest.buildings {
            for required in &building.requires_building_slugs {
                assert!(manifest.building(required).is_some());
            }
            if let Some(unit_slug) = &building.unlocks_unit_slug {
                assert!(manifest.unit(unit_slug).is_some());
            }
        }
    }

    #[test]
    fn all_referenced_assets_are_manifested() {
        let manifest = first_playable_content_manifest();
        let asset_keys = manifest
            .asset_keys
            .iter()
            .collect::<std::collections::BTreeSet<_>>();

        for faction in &manifest.factions {
            assert_optional_asset(faction.icon_key.as_ref(), &asset_keys);
            assert_optional_asset(faction.banner_key.as_ref(), &asset_keys);
        }
        for class in &manifest.champion_classes {
            assert_optional_asset(class.portrait_key.as_ref(), &asset_keys);
        }
        for terrain in &manifest.terrain {
            assert_optional_asset(terrain.sprite_key.as_ref(), &asset_keys);
        }
        for unit in &manifest.units {
            assert_optional_asset(unit.sprite_key.as_ref(), &asset_keys);
            assert_optional_asset(unit.icon_key.as_ref(), &asset_keys);
        }
        for building in &manifest.buildings {
            assert_optional_asset(building.icon_key.as_ref(), &asset_keys);
        }
        for artifact in &manifest.artifacts {
            assert_optional_asset(artifact.icon_key.as_ref(), &asset_keys);
        }
        for object in &manifest.map_objects {
            assert_optional_asset(object.sprite_key.as_ref(), &asset_keys);
            assert_optional_asset(object.icon_key.as_ref(), &asset_keys);
        }
    }

    fn assert_in_bounds(x: u16, y: u16, scenario: &FirstPlayableScenario) {
        assert!(x < scenario.map.width, "x {x} outside map");
        assert!(y < scenario.map.height, "y {y} outside map");
    }

    fn assert_sorted_unique(values: &[String]) {
        let mut sorted = values.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(values, sorted);
    }

    fn assert_optional_asset(
        key: Option<&String>,
        asset_keys: &std::collections::BTreeSet<&String>,
    ) {
        if let Some(key) = key {
            assert!(asset_keys.contains(key), "missing asset key {key}");
        }
    }

    fn scenario_target_keys(
        scenario: &FirstPlayableScenario,
        manifest: &ContentManifest,
    ) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        keys.extend(scenario.starts.iter().map(|start| start.town_key.clone()));
        keys.extend(scenario.starts.iter().map(|start| {
            format!(
                "participant:{}",
                if start.slot_index == 0 {
                    "west"
                } else {
                    "east"
                }
            )
        }));
        keys.extend(scenario.mines.iter().map(|object| object.key.clone()));
        keys.extend(
            scenario
                .resource_piles
                .iter()
                .map(|object| object.key.clone()),
        );
        keys.extend(
            scenario
                .central_objectives
                .iter()
                .map(|object| object.key.clone()),
        );
        keys.extend(
            scenario
                .neutral_armies
                .iter()
                .map(|neutral| neutral.key.clone()),
        );
        keys.extend(
            manifest
                .buildings
                .iter()
                .map(|building| format!("building:{}", building.slug)),
        );
        keys.extend(
            manifest
                .units
                .iter()
                .map(|unit| format!("unit:{}", unit.slug)),
        );
        keys
    }
}
