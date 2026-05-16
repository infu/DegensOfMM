use crate::schema::DegensStore;
use icydb::design::prelude::*;

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "account_principal", unique),
    index(fields = "username", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(ident = "account_principal", value(item(prim = "Principal"))),
        field(ident = "username", value(opt, item(prim = "Text", max_len = 32))),
        field(ident = "display_name", value(opt, item(prim = "Text", max_len = 64)))
    )
)]
pub struct PlayerAccount {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "slug, version", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "version", value(item(prim = "Nat32")), default = 1u32),
        field(ident = "name", value(item(prim = "Text", max_len = 96))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(
            ident = "content_manifest_hash",
            value(opt, item(prim = "Text", max_len = 64))
        )
    )
)]
pub struct RulesetDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "created_by_player_id"),
    index(fields = "state, current_turn"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "created_by_player_id",
            value(item(rel = "PlayerAccount", prim = "Ulid", strong))
        ),
        field(ident = "name", value(item(prim = "Text", max_len = 96))),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 16)),
            default = "lobby"
        ),
        field(ident = "seed", value(item(prim = "Nat64"))),
        field(ident = "map_width", value(item(prim = "Nat16"))),
        field(ident = "map_height", value(item(prim = "Nat16"))),
        field(ident = "chunk_size", value(item(prim = "Nat8")), default = 16u8),
        field(
            ident = "simultaneous_turns",
            value(item(prim = "Bool")),
            default = true
        ),
        field(
            ident = "turn_duration_ms",
            value(item(prim = "Nat32")),
            default = 60000u32
        ),
        field(ident = "max_turns", value(item(prim = "Nat32")), default = 30u32),
        field(
            ident = "turn_catchup_cap",
            value(item(prim = "Nat32")),
            default = 10u32
        ),
        field(ident = "current_turn", value(item(prim = "Nat32")), default = 1u32),
        field(
            ident = "next_event_seq",
            value(item(prim = "Nat64")),
            default = 1u64,
            db_default = 1u64
        ),
        field(
            ident = "turn_started_at",
            value(item(prim = "Timestamp")),
            default = "Timestamp::now",
            generated(insert = "Timestamp::now")
        ),
        field(ident = "turn_deadline_at", value(item(prim = "Timestamp"))),
        field(
            ident = "winner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(ident = "finish_reason", value(opt, item(prim = "Text", max_len = 32))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct GameSession {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, player_id", unique),
    index(fields = "session_id, status"),
    index(fields = "player_id, status"),
    index(fields = "session_id, slot_index", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "player_id",
            value(item(rel = "PlayerAccount", prim = "Ulid", strong))
        ),
        field(
            ident = "faction_id",
            value(item(rel = "FactionDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slot_index", value(item(prim = "Nat8"))),
        field(ident = "team_index", value(item(prim = "Nat8")), default = 0u8),
        field(ident = "color_key", value(item(prim = "Text", max_len = 24))),
        field(ident = "primary_color", value(opt, item(prim = "Text", max_len = 16))),
        field(
            ident = "secondary_color",
            value(opt, item(prim = "Text", max_len = 16))
        ),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "gold", value(item(prim = "Nat64")), default = 10000u64),
        field(ident = "wood", value(item(prim = "Nat32")), default = 10u32),
        field(ident = "stone", value(item(prim = "Nat32")), default = 10u32),
        field(ident = "iron", value(item(prim = "Nat32")), default = 3u32),
        field(ident = "crystal", value(item(prim = "Nat32")), default = 3u32),
        field(ident = "ember", value(item(prim = "Nat32")), default = 3u32),
        field(ident = "aether", value(item(prim = "Nat32")), default = 3u32),
        field(
            ident = "last_income_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_action_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(ident = "ready_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "last_resource_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "joined_at",
            value(item(prim = "Timestamp")),
            default = "Timestamp::now",
            generated(insert = "Timestamp::now")
        ),
        field(ident = "champion_ids", value(many, item(prim = "Ulid")))
    )
)]
pub struct GameParticipant {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id, ledger_key", unique),
    index(fields = "participant_id, turn_number"),
    index(fields = "session_id, participant_id"),
    index(fields = "session_id, resource_key"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "ledger_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "resource_key", value(item(prim = "Text", max_len = 16))),
        field(ident = "delta", value(item(prim = "Int64"))),
        field(ident = "balance_after", value(item(prim = "Nat64"))),
        field(ident = "reason", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        )
    )
)]
pub struct ResourceLedgerEntry {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, participant_id, turn_number", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "summary_json", value(item(prim = "Text", max_len = 4096)))
    )
)]
pub struct ResourceLedgerTurnSummary {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "player_id, session_id", unique),
    index(fields = "player_id, finished_at"),
    index(fields = "session_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "player_id",
            value(item(rel = "PlayerAccount", prim = "Ulid", weak))
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", weak))
        ),
        field(ident = "result", value(item(prim = "Text", max_len = 16))),
        field(ident = "opponent_name", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "turns_played", value(item(prim = "Nat32"))),
        field(
            ident = "summary_json",
            value(opt, item(prim = "Text", max_len = 2048))
        ),
        field(
            ident = "finished_at",
            value(item(prim = "Timestamp")),
            default = "Timestamp::now",
            generated(insert = "Timestamp::now")
        )
    )
)]
pub struct PlayerMatchSummary {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, actor_kind, actor_id_text", unique),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "actor_kind", value(item(prim = "Text", max_len = 24))),
        field(ident = "actor_id_text", value(item(prim = "Text", max_len = 64))),
        field(ident = "profile_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "last_decision_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(ident = "cursor_json", value(opt, item(prim = "Text", max_len = 2048))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct AiActorState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "ruleset_id, name", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "theme", value(opt, item(prim = "Text", max_len = 256))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "banner_key", value(opt, item(prim = "Text", max_len = 64))),
        field(
            ident = "native_terrain",
            value(opt, item(prim = "Text", max_len = 32))
        ),
        field(ident = "trait_key", value(item(prim = "Text", max_len = 64)))
    )
)]
pub struct FactionDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "faction_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "faction_id",
            value(opt, item(rel = "FactionDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "portrait_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "base_movement", value(item(prim = "Nat16")), default = 240u16),
        field(ident = "base_vision", value(item(prim = "Nat8")), default = 5u8)
    )
)]
pub struct ChampionClassDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, terrain_key", unique),
    index(fields = "ruleset_id, terrain_code", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(ident = "terrain_key", value(item(prim = "Text", max_len = 32))),
        field(ident = "terrain_code", value(item(prim = "Nat8"))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "movement_cost", value(item(prim = "Nat16"))),
        field(ident = "passable", value(item(prim = "Bool")), default = true),
        field(ident = "sprite_key", value(opt, item(prim = "Text", max_len = 64)))
    )
)]
pub struct TerrainDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "faction_id, tier"),
    index(fields = "ruleset_id, tier"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "faction_id",
            value(opt, item(rel = "FactionDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "sprite_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "animation_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "tier", value(item(prim = "Nat8"))),
        field(ident = "attack", value(item(prim = "Int16"))),
        field(ident = "defense", value(item(prim = "Int16"))),
        field(ident = "damage_min", value(item(prim = "Nat16"))),
        field(ident = "damage_max", value(item(prim = "Nat16"))),
        field(ident = "max_hp", value(item(prim = "Nat16"))),
        field(ident = "speed", value(item(prim = "Nat8"))),
        field(ident = "initiative", value(item(prim = "Nat8"))),
        field(ident = "ranged", value(item(prim = "Bool")), default = false),
        field(ident = "flying", value(item(prim = "Bool")), default = false),
        field(ident = "shots", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "gold_cost", value(item(prim = "Nat32"))),
        field(ident = "wood_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "stone_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "iron_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "crystal_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "ember_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "aether_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "weekly_growth", value(item(prim = "Nat16"))),
        field(ident = "ability_keys", value(many, item(prim = "Text", max_len = 64)))
    )
)]
pub struct UnitDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "faction_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "faction_id",
            value(opt, item(rel = "FactionDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "building_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "gold_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "wood_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "stone_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "iron_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "crystal_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "ember_cost", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "aether_cost", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "requires_building_slugs",
            value(many, item(prim = "Text", max_len = 64))
        ),
        field(
            ident = "unlocks_unit_slug",
            value(opt, item(prim = "Text", max_len = 64))
        ),
        field(ident = "effect_key", value(opt, item(prim = "Text", max_len = 64)))
    )
)]
pub struct BuildingDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "ruleset_id, school, level"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "school", value(item(prim = "Text", max_len = 32))),
        field(ident = "level", value(item(prim = "Nat8"))),
        field(ident = "mana_cost", value(item(prim = "Nat16"))),
        field(ident = "target_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "effect_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "duration_rounds", value(item(prim = "Nat8")), default = 0u8)
    )
)]
pub struct SpellDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "ruleset_id, rarity"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "slot", value(item(prim = "Text", max_len = 32))),
        field(ident = "rarity", value(item(prim = "Text", max_len = 16))),
        field(ident = "effect_key", value(item(prim = "Text", max_len = 64)))
    )
)]
pub struct ArtifactDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "ruleset_id, slug", unique),
    index(fields = "ruleset_id, object_type"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "ruleset_id",
            value(item(rel = "RulesetDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "description", value(opt, item(prim = "Text", max_len = 512))),
        field(ident = "sprite_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "icon_key", value(opt, item(prim = "Text", max_len = 64))),
        field(ident = "object_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "footprint_w", value(item(prim = "Nat8")), default = 1u8),
        field(ident = "footprint_h", value(item(prim = "Nat8")), default = 1u8),
        field(ident = "blocking", value(item(prim = "Bool")), default = false),
        field(ident = "interaction_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "refresh_rule",
            value(item(prim = "Text", max_len = 32)),
            default = "never"
        )
    )
)]
pub struct MapObjectDefinition {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, chunk_x, chunk_y", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(ident = "width", value(item(prim = "Nat8"))),
        field(ident = "height", value(item(prim = "Nat8"))),
        field(ident = "terrain_blob", value(item(prim = "Blob", max_len = 4096))),
        field(ident = "movement_blob", value(item(prim = "Blob", max_len = 4096))),
        field(ident = "flags_blob", value(item(prim = "Blob", max_len = 4096)))
    )
)]
pub struct MapChunk {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "participant_id, chunk_x, chunk_y", unique),
    index(fields = "session_id, participant_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(ident = "discovered_blob", value(item(prim = "Blob", max_len = 4096))),
        field(ident = "visible_blob", value(item(prim = "Blob", max_len = 4096))),
        field(ident = "visible_turn", value(item(prim = "Nat32")), default = 0u32)
    )
)]
pub struct VisibilityChunk {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y, layer", unique),
    index(fields = "session_id, chunk_x, chunk_y"),
    index(fields = "occupant_kind, occupant_id_text"),
    index(
        fields = "session_id, occupant_kind, occupant_id_text, occupant_cell_index",
        unique
    ),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(ident = "layer", value(item(prim = "Text", max_len = 24))),
        field(ident = "occupant_kind", value(item(prim = "Text", max_len = 32))),
        field(ident = "occupant_id_text", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "occupant_cell_index",
            value(item(prim = "Nat8")),
            default = 0u8
        ),
        field(ident = "blocking", value(item(prim = "Bool")), default = true),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct MapOccupancy {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y"),
    index(fields = "session_id, chunk_x, chunk_y, state"),
    index(fields = "session_id, owner_participant_id"),
    index(fields = "session_id, scoring_kind, owner_participant_id, state"),
    index(fields = "object_def_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "object_def_id",
            value(item(rel = "MapObjectDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "owner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "guarded_neutral_army_id",
            value(opt, item(rel = "NeutralArmy", prim = "Ulid", weak))
        ),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "scoring_kind",
            value(item(prim = "Text", max_len = 24)),
            default = "none"
        ),
        field(
            ident = "last_visited_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(ident = "captured_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "income_started_turn",
            value(item(prim = "Nat32")),
            default = 1u32
        ),
        field(
            ident = "instance_json",
            value(opt, item(prim = "Text", max_len = 2048))
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct WorldObject {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "object_id, participant_id, visit_key", unique),
    index(fields = "session_id, participant_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "object_id",
            value(item(rel = "WorldObject", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "visit_key",
            value(item(prim = "Text", max_len = 32)),
            default = "once"
        ),
        field(ident = "visit_kind", value(item(prim = "Text", max_len = 24))),
        field(ident = "visited_turn", value(item(prim = "Nat32")))
    )
)]
pub struct ParticipantObjectVisit {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "object_id, champion_id, visit_key", unique),
    index(fields = "session_id, champion_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "object_id",
            value(item(rel = "WorldObject", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "visit_key",
            value(item(prim = "Text", max_len = 32)),
            default = "once"
        ),
        field(ident = "visit_kind", value(item(prim = "Text", max_len = 24))),
        field(ident = "visited_turn", value(item(prim = "Nat32")))
    )
)]
pub struct ChampionObjectVisit {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "participant_id, subject_kind, subject_id_text", unique),
    index(fields = "session_id, participant_id, chunk_x, chunk_y"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "subject_kind", value(item(prim = "Text", max_len = 32))),
        field(ident = "subject_id_text", value(item(prim = "Text", max_len = 64))),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(ident = "visibility", value(item(prim = "Text", max_len = 24))),
        field(ident = "last_seen_turn", value(item(prim = "Nat32"))),
        field(
            ident = "redacted_json",
            value(opt, item(prim = "Text", max_len = 2048))
        )
    )
)]
pub struct ParticipantKnownObject {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y"),
    index(fields = "session_id, chunk_x, chunk_y, status"),
    index(fields = "session_id, owner_participant_id"),
    index(fields = "owner_participant_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "owner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "faction_id",
            value(item(rel = "FactionDefinition", prim = "Ulid", strong))
        ),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "hall_level", value(item(prim = "Nat8")), default = 1u8),
        field(ident = "fort_level", value(item(prim = "Nat8")), default = 0u8),
        field(ident = "last_built_turn", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "captured_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "income_started_turn",
            value(item(prim = "Nat32")),
            default = 1u32
        ),
        field(
            ident = "unrest_until_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct Town {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "town_id, building_def_id", unique),
    index(fields = "town_id"),
    index(fields = "session_id"),
    index(fields = "building_def_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "town_id", value(item(rel = "Town", prim = "Ulid", strong))),
        field(
            ident = "building_def_id",
            value(item(rel = "BuildingDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "building_slug",
            value(item(prim = "Text", max_len = 64)),
            default = ""
        ),
        field(ident = "built_turn", value(item(prim = "Nat32")))
    )
)]
pub struct TownBuilding {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "town_id, unit_id", unique),
    index(fields = "town_id"),
    index(fields = "session_id"),
    index(fields = "unit_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "town_id", value(item(rel = "Town", prim = "Ulid", strong))),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_slug",
            value(item(prim = "Text", max_len = 64)),
            default = ""
        ),
        field(ident = "available", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "last_growth_week",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct TownRecruitPool {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "town_id, slot_index", unique),
    index(fields = "town_id"),
    index(fields = "session_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "town_id", value(item(rel = "Town", prim = "Ulid", strong))),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_slug",
            value(item(prim = "Text", max_len = 64)),
            default = ""
        ),
        field(ident = "slot_index", value(item(prim = "Nat8"))),
        field(ident = "quantity", value(item(prim = "Nat32"))),
        field(ident = "front_hp", value(item(prim = "Nat16"))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct TownGarrisonStack {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y"),
    index(fields = "session_id, chunk_x, chunk_y, status"),
    index(fields = "session_id, participant_id, status"),
    index(fields = "participant_id, status"),
    index(fields = "in_battle_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "class_def_id",
            value(item(rel = "ChampionClassDefinition", prim = "Ulid", strong))
        ),
        field(ident = "name", value(item(prim = "Text", max_len = 64))),
        field(ident = "class_key", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "in_battle_id",
            value(opt, item(rel = "Battle", prim = "Ulid", weak))
        ),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(ident = "level", value(item(prim = "Nat16")), default = 1u16),
        field(ident = "experience", value(item(prim = "Nat64")), default = 0u64),
        field(ident = "might", value(item(prim = "Int16")), default = 1i16),
        field(ident = "guard", value(item(prim = "Int16")), default = 1i16),
        field(ident = "wisdom", value(item(prim = "Int16")), default = 1i16),
        field(ident = "command", value(item(prim = "Int16")), default = 1i16),
        field(ident = "mana", value(item(prim = "Nat16")), default = 10u16),
        field(ident = "movement_max", value(item(prim = "Nat16")), default = 240u16),
        field(
            ident = "movement_remaining",
            value(item(prim = "Nat16")),
            default = 240u16
        ),
        field(ident = "movement_turn", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "vision_radius", value(item(prim = "Nat8")), default = 5u8),
        field(ident = "defeated_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "mana_max", value(item(prim = "Nat16")), default = 10u16),
        field(ident = "mana_turn", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "skill_points", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "skill_keys", value(many, item(prim = "Text", max_len = 64)))
    )
)]
pub struct Champion {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "champion_id, slot_index", unique),
    index(fields = "champion_id"),
    index(fields = "session_id"),
    index(fields = "unit_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slot_index", value(item(prim = "Nat8"))),
        field(ident = "quantity", value(item(prim = "Nat32"))),
        field(ident = "front_hp", value(item(prim = "Nat16"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ChampionArmyStack {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "champion_id, spell_id", unique),
    index(fields = "champion_id"),
    index(fields = "session_id"),
    index(fields = "spell_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "spell_id",
            value(item(rel = "SpellDefinition", prim = "Ulid", strong))
        ),
        field(ident = "learned_turn", value(item(prim = "Nat32"))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ChampionSpell {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, town_id, week_number"),
    index(fields = "town_id, week_number, offer_slot", unique),
    index(fields = "session_id, participant_id, status"),
    index(fields = "offer_key", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "town_id", value(item(rel = "Town", prim = "Ulid", strong))),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "week_number", value(item(prim = "Nat32"))),
        field(ident = "offer_slot", value(item(prim = "Nat8"))),
        field(ident = "offer_key", value(item(prim = "Text", max_len = 96))),
        field(
            ident = "champion_class_id",
            value(item(rel = "ChampionClassDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_class_slug",
            value(item(prim = "Text", max_len = 64))
        ),
        field(ident = "candidate_name", value(item(prim = "Text", max_len = 64))),
        field(ident = "cost_gold", value(item(prim = "Nat32"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "available"
        ),
        field(
            ident = "hired_champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(
            ident = "hired_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct TavernOffer {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id", unique),
    index(fields = "session_id, participant_id"),
    index(fields = "session_id, town_id"),
    index(fields = "offer_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "town_id", value(item(rel = "Town", prim = "Ulid", strong))),
        field(
            ident = "offer_id",
            value(item(rel = "TavernOffer", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(ident = "cost_gold", value(item(prim = "Nat32"))),
        field(ident = "hired_turn", value(item(prim = "Nat32"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "applied"
        )
    )
)]
pub struct ChampionHire {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id", unique),
    index(fields = "session_id, participant_id, turn_number"),
    index(fields = "session_id, from_resource, to_resource"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "from_resource", value(item(prim = "Text", max_len = 16))),
        field(ident = "to_resource", value(item(prim = "Text", max_len = 16))),
        field(ident = "amount_in", value(item(prim = "Nat64"))),
        field(ident = "amount_out", value(item(prim = "Nat64"))),
        field(ident = "rate_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "applied"
        )
    )
)]
pub struct MarketTrade {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "object_id, unit_id", unique),
    index(fields = "session_id, participant_id"),
    index(fields = "session_id, object_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "object_id",
            value(item(rel = "WorldObject", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(ident = "unit_slug", value(item(prim = "Text", max_len = 64))),
        field(ident = "available", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "last_growth_week",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(ident = "growth_per_week", value(item(prim = "Nat16")), default = 4u16),
        field(ident = "direct_recruit", value(item(prim = "Bool")), default = true),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct DwellingPool {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id", unique),
    index(fields = "session_id, participant_id"),
    index(fields = "object_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "object_id",
            value(item(rel = "WorldObject", prim = "Ulid", strong))
        ),
        field(
            ident = "pool_id",
            value(item(rel = "DwellingPool", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(ident = "unit_slug", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "quantity", value(item(prim = "Nat32"))),
        field(ident = "recruited_turn", value(item(prim = "Nat32"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "applied"
        )
    )
)]
pub struct DwellingRecruitment {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, objective_key", unique),
    index(fields = "session_id, participant_id"),
    index(fields = "object_id"),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "object_id",
            value(opt, item(rel = "WorldObject", prim = "Ulid", strong))
        ),
        field(ident = "objective_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "objective_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "progress_value", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "required_value", value(item(prim = "Nat32")), default = 1u32),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "visible_to",
            value(item(prim = "Text", max_len = 32)),
            default = "public"
        ),
        field(
            ident = "last_scored_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ObjectiveProgress {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, participant_id, quest_key", unique),
    index(fields = "session_id, participant_id, status"),
    index(fields = "session_id, quest_key"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "quest_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "title", value(item(prim = "Text", max_len = 96))),
        field(ident = "objective_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "available"
        ),
        field(ident = "progress_value", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "required_value", value(item(prim = "Nat32")), default = 1u32),
        field(ident = "reward_gold", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "accepted_turn", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "claimed_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "accepted_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "claimed_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct QuestState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, event_key", unique),
    index(fields = "session_id, event_window"),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "event_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "event_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "event_window", value(item(prim = "Text", max_len = 32))),
        field(ident = "starts_turn", value(item(prim = "Nat32"))),
        field(ident = "ends_turn", value(item(prim = "Nat32"))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 2048))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct WorldEventState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, rule_key", unique),
    index(fields = "session_id, victory_state"),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "rule_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "rule_type", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "victory_state",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "required_value", value(item(prim = "Nat32")), default = 1u32),
        field(ident = "current_value", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "owner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(
            ident = "winner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(
            ident = "disabled_reason",
            value(opt, item(prim = "Text", max_len = 128))
        ),
        field(
            ident = "last_checked_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ScenarioRuleState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id", unique),
    index(fields = "profile_key, status"),
    index(fields = "generation_key"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "profile_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "map_seed", value(item(prim = "Nat64"))),
        field(ident = "map_width", value(item(prim = "Nat16"))),
        field(ident = "map_height", value(item(prim = "Nat16"))),
        field(ident = "chunk_size", value(item(prim = "Nat8"))),
        field(ident = "player_count", value(item(prim = "Nat8"))),
        field(ident = "fog_enabled", value(item(prim = "Bool")), default = true),
        field(
            ident = "neutral_difficulty",
            value(item(prim = "Text", max_len = 24)),
            default = "standard"
        ),
        field(
            ident = "victory_condition",
            value(item(prim = "Text", max_len = 32)),
            default = "conquest"
        ),
        field(ident = "generation_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "naval_enabled", value(item(prim = "Bool")), default = false),
        field(ident = "siege_enabled", value(item(prim = "Bool")), default = false),
        field(
            ident = "larger_map_enabled",
            value(item(prim = "Bool")),
            default = false
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct SkirmishSettingsState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, generation_key", unique),
    index(fields = "session_id, status"),
    index(fields = "generation_key"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "generation_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "planned"
        ),
        field(ident = "map_seed", value(item(prim = "Nat64"))),
        field(ident = "map_width", value(item(prim = "Nat16"))),
        field(ident = "map_height", value(item(prim = "Nat16"))),
        field(ident = "chunk_size", value(item(prim = "Nat8"))),
        field(ident = "chunk_count", value(item(prim = "Nat32"))),
        field(ident = "land_tile_count", value(item(prim = "Nat32"))),
        field(ident = "water_tile_count", value(item(prim = "Nat32"))),
        field(ident = "road_tile_count", value(item(prim = "Nat32"))),
        field(ident = "town_count", value(item(prim = "Nat32"))),
        field(ident = "mine_count", value(item(prim = "Nat32"))),
        field(ident = "scenario_hash", value(item(prim = "Text", max_len = 64))),
        field(ident = "generated_turn", value(item(prim = "Nat32")), default = 0u32),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ProceduralMapState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, route_key", unique),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "route_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "disabled"
        ),
        field(ident = "from_x", value(item(prim = "Nat16"))),
        field(ident = "from_y", value(item(prim = "Nat16"))),
        field(ident = "to_x", value(item(prim = "Nat16"))),
        field(ident = "to_y", value(item(prim = "Nat16"))),
        field(ident = "water_crossings", value(item(prim = "Nat16"))),
        field(ident = "boat_required", value(item(prim = "Bool")), default = true),
        field(
            ident = "disabled_reason",
            value(opt, item(prim = "Text", max_len = 128))
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct NavalRouteState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, rule_key", unique),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "rule_key", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "disabled"
        ),
        field(
            ident = "fortification_level",
            value(item(prim = "Text", max_len = 24)),
            default = "palisade"
        ),
        field(ident = "wall_segments", value(item(prim = "Nat16"))),
        field(ident = "gate_count", value(item(prim = "Nat8"))),
        field(ident = "tower_count", value(item(prim = "Nat8"))),
        field(ident = "siege_engine_slots", value(item(prim = "Nat8"))),
        field(ident = "battle_obstacle_cap", value(item(prim = "Nat16"))),
        field(
            ident = "disabled_reason",
            value(opt, item(prim = "Text", max_len = 128))
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct SiegeRuleState {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y"),
    index(fields = "session_id, chunk_x, chunk_y, state"),
    index(fields = "owner_champion_id, slot"),
    index(fields = "artifact_def_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "artifact_def_id",
            value(item(rel = "ArtifactDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "owner_champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(ident = "slot", value(opt, item(prim = "Text", max_len = 32))),
        field(ident = "x", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "y", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "chunk_x", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "chunk_y", value(item(prim = "Nat16")), default = 0u16),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 24)),
            default = "stored"
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ArtifactInstance {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "champion_id, slot", unique),
    index(fields = "artifact_id", unique),
    index(fields = "session_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "artifact_id",
            value(item(rel = "ArtifactInstance", prim = "Ulid", strong))
        ),
        field(ident = "slot", value(item(prim = "Text", max_len = 32))),
        field(ident = "equipped_turn", value(item(prim = "Nat32"))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct ArtifactEquipment {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, x, y"),
    index(fields = "session_id, chunk_x, chunk_y, state"),
    index(fields = "session_id, state"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "x", value(item(prim = "Nat16"))),
        field(ident = "y", value(item(prim = "Nat16"))),
        field(ident = "chunk_x", value(item(prim = "Nat16"))),
        field(ident = "chunk_y", value(item(prim = "Nat16"))),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "aggression",
            value(item(prim = "Text", max_len = 24)),
            default = "guard"
        ),
        field(
            ident = "growth_rule_key",
            value(item(prim = "Text", max_len = 64)),
            default = "none"
        ),
        field(
            ident = "last_growth_week",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct NeutralArmy {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "neutral_army_id, slot_index", unique),
    index(fields = "neutral_army_id"),
    index(fields = "session_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "neutral_army_id",
            value(item(rel = "NeutralArmy", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(ident = "slot_index", value(item(prim = "Nat8"))),
        field(ident = "quantity", value(item(prim = "Nat32"))),
        field(ident = "front_hp", value(item(prim = "Nat16"))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct NeutralArmyStack {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, state"),
    index(fields = "attacker_champion_id"),
    index(fields = "defender_champion_id"),
    index(fields = "created_turn"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(ident = "battle_type", value(item(prim = "Text", max_len = 24))),
        field(
            ident = "attacker_champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(
            ident = "defender_champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(
            ident = "defender_town_id",
            value(opt, item(rel = "Town", prim = "Ulid", weak))
        ),
        field(
            ident = "defender_neutral_army_id",
            value(opt, item(rel = "NeutralArmy", prim = "Ulid", weak))
        ),
        field(ident = "current_round", value(item(prim = "Nat16")), default = 1u16),
        field(
            ident = "active_side",
            value(item(prim = "Text", max_len = 16)),
            default = "attacker"
        ),
        field(
            ident = "active_stack_id",
            value(opt, item(rel = "BattleStack", prim = "Ulid", weak))
        ),
        field(ident = "grid_width", value(item(prim = "Nat8")), default = 12u8),
        field(ident = "grid_height", value(item(prim = "Nat8")), default = 10u8),
        field(ident = "max_rounds", value(item(prim = "Nat16")), default = 20u16),
        field(ident = "turn_seed", value(item(prim = "Nat64"))),
        field(
            ident = "winner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(ident = "created_turn", value(item(prim = "Nat32"))),
        field(ident = "action_deadline_at", value(opt, item(prim = "Timestamp"))),
        field(ident = "resolved_at", value(opt, item(prim = "Timestamp"))),
        field(
            ident = "cleanup_after_turn",
            value(item(prim = "Nat32")),
            default = 0u32
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct Battle {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "battle_id, obstacle_type"),
    index(fields = "battle_id, battle_x, battle_y", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "battle_id",
            value(item(rel = "Battle", prim = "Ulid", strong))
        ),
        field(ident = "obstacle_type", value(item(prim = "Text", max_len = 24))),
        field(ident = "battle_x", value(item(prim = "Nat8"))),
        field(ident = "battle_y", value(item(prim = "Nat8"))),
        field(ident = "width", value(item(prim = "Nat8")), default = 1u8),
        field(ident = "height", value(item(prim = "Nat8")), default = 1u8),
        field(ident = "hp", value(item(prim = "Nat16")), default = 0u16),
        field(
            ident = "state",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct BattleObstacle {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "battle_id, side"),
    index(fields = "battle_id, side, slot_index", unique),
    index(fields = "unit_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "battle_id",
            value(item(rel = "Battle", prim = "Ulid", strong))
        ),
        field(
            ident = "unit_id",
            value(item(rel = "UnitDefinition", prim = "Ulid", strong))
        ),
        field(
            ident = "owner_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(ident = "side", value(item(prim = "Text", max_len = 16))),
        field(ident = "slot_index", value(item(prim = "Nat8"))),
        field(ident = "origin_kind", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "origin_stack_id_text",
            value(opt, item(prim = "Text", max_len = 64))
        ),
        field(ident = "origin_slot_index", value(item(prim = "Nat8"))),
        field(ident = "attack", value(item(prim = "Int16"))),
        field(ident = "defense", value(item(prim = "Int16"))),
        field(ident = "damage_min", value(item(prim = "Nat16"))),
        field(ident = "damage_max", value(item(prim = "Nat16"))),
        field(ident = "max_hp", value(item(prim = "Nat16"))),
        field(ident = "speed", value(item(prim = "Nat8"))),
        field(ident = "initiative", value(item(prim = "Nat8"))),
        field(ident = "ranged", value(item(prim = "Bool")), default = false),
        field(ident = "flying", value(item(prim = "Bool")), default = false),
        field(ident = "quantity", value(item(prim = "Nat32"))),
        field(ident = "front_hp", value(item(prim = "Nat16"))),
        field(ident = "shots_remaining", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "battle_x", value(item(prim = "Nat8"))),
        field(ident = "battle_y", value(item(prim = "Nat8"))),
        field(ident = "readiness", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "acted_round", value(item(prim = "Nat16")), default = 0u16),
        field(
            ident = "retaliated_round",
            value(item(prim = "Nat16")),
            default = 0u16
        ),
        field(ident = "defended_round", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "waited_round", value(item(prim = "Nat16")), default = 0u16),
        field(ident = "cast_round", value(item(prim = "Nat16")), default = 0u16),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "active"
        ),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "status_keys", value(many, item(prim = "Text", max_len = 64)))
    )
)]
pub struct BattleStack {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "battle_id, battle_x, battle_y", unique),
    index(fields = "battle_stack_id", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "battle_id",
            value(item(rel = "Battle", prim = "Ulid", strong))
        ),
        field(
            ident = "battle_stack_id",
            value(item(rel = "BattleStack", prim = "Ulid", strong))
        ),
        field(ident = "battle_x", value(item(prim = "Nat8"))),
        field(ident = "battle_y", value(item(prim = "Nat8"))),
        field(
            ident = "last_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        )
    )
)]
pub struct BattleOccupancy {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, actor_kind, actor_id_text, client_nonce", unique),
    index(fields = "session_id, status, created_at"),
    index(fields = "session_id, turn_number"),
    index(fields = "actor_participant_id, turn_number"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "actor_kind", value(item(prim = "Text", max_len = 24))),
        field(ident = "actor_id_text", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "actor_player_id",
            value(opt, item(rel = "PlayerAccount", prim = "Ulid", weak))
        ),
        field(
            ident = "actor_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(
            ident = "champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", weak))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "client_nonce", value(item(prim = "Nat64"))),
        field(ident = "command_type", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        ),
        field(
            ident = "phase",
            value(item(prim = "Text", max_len = 32)),
            default = "created"
        ),
        field(ident = "payload_hash", value(item(prim = "Text", max_len = 64))),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 4096))),
        field(ident = "result_json", value(opt, item(prim = "Text", max_len = 4096))),
        field(ident = "error_code", value(opt, item(prim = "Text", max_len = 64))),
        field(
            ident = "error_message",
            value(opt, item(prim = "Text", max_len = 256))
        ),
        field(
            ident = "error_details_json",
            value(opt, item(prim = "Text", max_len = 2048))
        ),
        field(ident = "retryable", value(item(prim = "Bool")), default = false),
        field(ident = "applied_at", value(opt, item(prim = "Timestamp"))),
        field(ident = "failed_at", value(opt, item(prim = "Timestamp")))
    )
)]
pub struct GameCommand {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "actor_principal, client_nonce", unique),
    index(fields = "status, created_at"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(ident = "actor_principal", value(item(prim = "Principal"))),
        field(
            ident = "actor_player_id",
            value(opt, item(rel = "PlayerAccount", prim = "Ulid", weak))
        ),
        field(ident = "client_nonce", value(item(prim = "Nat64"))),
        field(ident = "payload_hash", value(item(prim = "Text", max_len = 64))),
        field(ident = "command_type", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        ),
        field(
            ident = "phase",
            value(item(prim = "Text", max_len = 32)),
            default = "created"
        ),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 4096))),
        field(ident = "result_json", value(opt, item(prim = "Text", max_len = 4096))),
        field(ident = "error_code", value(opt, item(prim = "Text", max_len = 64))),
        field(
            ident = "error_message",
            value(opt, item(prim = "Text", max_len = 256))
        ),
        field(
            ident = "error_details_json",
            value(opt, item(prim = "Text", max_len = 2048))
        ),
        field(ident = "retryable", value(item(prim = "Bool")), default = false),
        field(ident = "applied_at", value(opt, item(prim = "Timestamp"))),
        field(ident = "failed_at", value(opt, item(prim = "Timestamp")))
    )
)]
pub struct LobbyCommand {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, turn_number, status"),
    index(fields = "session_id, champion_id, turn_number", unique),
    index(fields = "command_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(
            ident = "actor_participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        ),
        field(ident = "path_json", value(item(prim = "Text", max_len = 2048))),
        field(ident = "path_hash", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "submitted_at",
            value(item(prim = "Timestamp")),
            default = "Timestamp::now",
            generated(insert = "Timestamp::now")
        ),
        field(ident = "resolved_at", value(opt, item(prim = "Timestamp")))
    )
)]
pub struct MovementIntent {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id, intent_id, step_index", unique),
    index(fields = "session_id, turn_number, champion_id"),
    index(fields = "session_id, turn_number, participant_id"),
    index(fields = "intent_id"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "intent_id",
            value(item(rel = "MovementIntent", prim = "Ulid", strong))
        ),
        field(
            ident = "champion_id",
            value(item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(
            ident = "participant_id",
            value(item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "step_index", value(item(prim = "Nat16"))),
        field(ident = "from_x", value(item(prim = "Nat16"))),
        field(ident = "from_y", value(item(prim = "Nat16"))),
        field(ident = "to_x", value(item(prim = "Nat16"))),
        field(ident = "to_y", value(item(prim = "Nat16"))),
        field(ident = "movement_cost", value(item(prim = "Nat16"))),
        field(ident = "remaining_after", value(item(prim = "Nat16"))),
        field(ident = "outcome", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "interaction_kind",
            value(opt, item(prim = "Text", max_len = 32))
        ),
        field(
            ident = "interaction_id_text",
            value(opt, item(prim = "Text", max_len = 64))
        )
    )
)]
pub struct MovementSnapshot {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "command_id, effect_key", unique),
    index(fields = "session_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(ident = "effect_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "effect_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "target_kind", value(item(prim = "Text", max_len = 32))),
        field(ident = "target_id_text", value(item(prim = "Text", max_len = 64))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        ),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 2048))),
        field(ident = "applied_at", value(opt, item(prim = "Timestamp")))
    )
)]
pub struct CommandEffect {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, event_seq", unique),
    index(fields = "session_id, turn_number"),
    index(fields = "session_id, event_type"),
    index(fields = "actor_participant_id"),
    index(fields = "command_id"),
    index(fields = "session_id, event_key", unique),
    index(fields = "session_id, audience_key, event_seq"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "actor_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", weak))
        ),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "event_seq", value(item(prim = "Nat64"))),
        field(ident = "event_key", value(item(prim = "Text", max_len = 128))),
        field(
            ident = "audience_key",
            value(item(prim = "Text", max_len = 96)),
            default = "public",
            db_default = "public"
        ),
        field(ident = "event_type", value(item(prim = "Text", max_len = 32))),
        field(ident = "subject_kind", value(opt, item(prim = "Text", max_len = 32))),
        field(
            ident = "subject_id_text",
            value(opt, item(prim = "Text", max_len = 64))
        ),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 4096)))
    )
)]
pub struct GameEvent {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, audience_key, turn_number", unique),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(ident = "audience_key", value(item(prim = "Text", max_len = 96))),
        field(ident = "turn_number", value(item(prim = "Nat32"))),
        field(ident = "first_event_seq", value(item(prim = "Nat64"))),
        field(ident = "last_event_seq", value(item(prim = "Nat64"))),
        field(ident = "event_count", value(item(prim = "Nat32"))),
        field(ident = "summary_json", value(item(prim = "Text", max_len = 4096)))
    )
)]
pub struct GameEventTurnSummary {}

#[entity(
    store = "DegensStore",
    pk(field = "id"),
    index(fields = "session_id, effect_key", unique),
    index(fields = "session_id, status, due_turn"),
    index(fields = "source_command_id"),
    index(fields = "target_participant_id, status"),
    index(fields = "target_champion_id, status"),
    fields(
        field(
            ident = "id",
            value(item(prim = "Ulid")),
            default = "Ulid::generate",
            generated(insert = "Ulid::generate")
        ),
        field(
            ident = "session_id",
            value(item(rel = "GameSession", prim = "Ulid", strong))
        ),
        field(
            ident = "source_command_id",
            value(opt, item(rel = "GameCommand", prim = "Ulid", weak))
        ),
        field(
            ident = "target_participant_id",
            value(opt, item(rel = "GameParticipant", prim = "Ulid", strong))
        ),
        field(
            ident = "target_champion_id",
            value(opt, item(rel = "Champion", prim = "Ulid", strong))
        ),
        field(ident = "effect_key", value(item(prim = "Text", max_len = 64))),
        field(ident = "due_turn", value(item(prim = "Nat32")), default = 0u32),
        field(ident = "effect_type", value(item(prim = "Text", max_len = 32))),
        field(
            ident = "status",
            value(item(prim = "Text", max_len = 24)),
            default = "pending"
        ),
        field(ident = "payload_json", value(item(prim = "Text", max_len = 4096))),
        field(ident = "applied_at", value(opt, item(prim = "Timestamp")))
    )
)]
pub struct PendingEffect {}
