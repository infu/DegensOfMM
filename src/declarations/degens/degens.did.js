export const idlFactory = ({ IDL }) => {
  const CommandStatus = IDL.Variant({
    'Applied' : IDL.Null,
    'Failed' : IDL.Null,
    'Superseded' : IDL.Null,
    'AppliedNoop' : IDL.Null,
    'Cancelled' : IDL.Null,
    'Pending' : IDL.Null,
    'Applying' : IDL.Null,
  });
  const ChampionMagicReceipt = IDL.Record({
    'action' : IDL.Text,
    'mana_after' : IDL.Nat16,
    'command_id' : IDL.Text,
    'skill_key' : IDL.Opt(IDL.Text),
    'champion_id' : IDL.Text,
    'status_keys' : IDL.Vec(IDL.Text),
    'movement_remaining_after' : IDL.Nat16,
    'spell_slug' : IDL.Opt(IDL.Text),
  });
  const ResourceBalances = IDL.Record({
    'aether' : IDL.Nat32,
    'gold' : IDL.Nat64,
    'iron' : IDL.Nat32,
    'wood' : IDL.Nat32,
    'ember' : IDL.Nat32,
    'stone' : IDL.Nat32,
    'crystal' : IDL.Nat32,
  });
  const RecruitPreview = IDL.Record({
    'town_id' : IDL.Text,
    'target_slot_index' : IDL.Opt(IDL.Nat8),
    'total_cost' : ResourceBalances,
    'allowed' : IDL.Bool,
    'unit_slug' : IDL.Text,
    'available' : IDL.Nat32,
    'quantity' : IDL.Nat32,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const BuildPreview = IDL.Record({
    'town_id' : IDL.Text,
    'cost' : ResourceBalances,
    'allowed' : IDL.Bool,
    'building_slug' : IDL.Text,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const ObjectStopDraft = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'interaction_key' : IDL.Text,
    'object_id' : IDL.Text,
    'champion_id' : IDL.Text,
  });
  const BattleStartDraft = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'attacker_champion_id' : IDL.Text,
    'defender_id_text' : IDL.Text,
    'defender_kind' : IDL.Text,
    'battle_type' : IDL.Text,
    'battle_key' : IDL.Text,
  });
  const MovementResolutionCursor = IDL.Record({
    'gameplay_rows_written' : IDL.Nat32,
    'session_id' : IDL.Text,
    'command_id' : IDL.Text,
    'next_step_index' : IDL.Nat16,
    'turn_number' : IDL.Nat32,
  });
  const MovementSnapshotRecord = IDL.Record({
    'movement_cost' : IDL.Nat16,
    'session_id' : IDL.Text,
    'remaining_after' : IDL.Nat16,
    'command_id' : IDL.Text,
    'to_x' : IDL.Nat16,
    'to_y' : IDL.Nat16,
    'step_index' : IDL.Nat16,
    'created_at_ms' : IDL.Nat64,
    'champion_id' : IDL.Text,
    'from_x' : IDL.Nat16,
    'from_y' : IDL.Nat16,
    'participant_id' : IDL.Text,
    'turn_number' : IDL.Nat32,
    'outcome' : IDL.Text,
    'interaction_id_text' : IDL.Opt(IDL.Text),
    'snapshot_id' : IDL.Text,
    'interaction_kind' : IDL.Opt(IDL.Text),
    'intent_id' : IDL.Text,
  });
  const MovementSyncOutcome = IDL.Record({
    'recovered_commands_advanced' : IDL.Nat32,
    'gameplay_rows_written' : IDL.Nat32,
    'superseded_intent_ids' : IDL.Vec(IDL.Text),
    'from_turn' : IDL.Nat32,
    'object_stops' : IDL.Vec(ObjectStopDraft),
    'battle_starts' : IDL.Vec(BattleStartDraft),
    'session_id' : IDL.Text,
    'resolved_intent_ids' : IDL.Vec(IDL.Text),
    'cursor' : IDL.Opt(MovementResolutionCursor),
    'command_id' : IDL.Text,
    'snapshots' : IDL.Vec(MovementSnapshotRecord),
    'budget_exhausted' : IDL.Bool,
    'advanced_turn' : IDL.Bool,
    'recovered_commands_inspected' : IDL.Nat32,
    'current_turn' : IDL.Nat32,
    'recovery_checked' : IDL.Bool,
  });
  const BattleActionReceipt = IDL.Record({
    'event_seq' : IDL.Opt(IDL.Nat64),
    'status' : IDL.Text,
    'command_id' : IDL.Text,
    'active_stack_id' : IDL.Opt(IDL.Text),
    'current_round' : IDL.Nat16,
  });
  const WorldGenerationReceipt = IDL.Record({
    'scenario_hash' : IDL.Text,
    'action' : IDL.Text,
    'command_id' : IDL.Text,
    'map_height' : IDL.Nat16,
    'generation_key' : IDL.Text,
    'map_width' : IDL.Nat16,
    'state' : IDL.Text,
    'chunk_count' : IDL.Nat32,
    'current_turn' : IDL.Nat32,
  });
  const MoveCoord = IDL.Record({ 'x' : IDL.Nat16, 'y' : IDL.Nat16 });
  const MovementPathStop = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'subject_kind' : IDL.Text,
    'subject_id_text' : IDL.Text,
    'reason' : IDL.Text,
  });
  const MovementPreview = IDL.Record({
    'available_movement' : IDL.Nat16,
    'chunks_touched' : IDL.Nat32,
    'path' : IDL.Vec(MoveCoord),
    'stop' : IDL.Opt(MovementPathStop),
    'total_cost' : IDL.Nat16,
    'champion_id' : IDL.Text,
    'participant_id' : IDL.Text,
    'turn_number' : IDL.Nat32,
  });
  const BattleSyncOutcome = IDL.Record({
    'battle_id' : IDL.Text,
    'battle_sync_incomplete' : IDL.Bool,
    'active_stack_id' : IDL.Opt(IDL.Text),
    'timeout_actions_applied' : IDL.Nat32,
    'recovered_commands' : IDL.Nat32,
  });
  const StrategicCommandReceipt = IDL.Record({
    'command_count' : IDL.Nat32,
    'command_id' : IDL.Text,
    'event_count' : IDL.Nat32,
    'command_kind' : IDL.Text,
    'current_turn' : IDL.Nat32,
  });
  const ExpandedEconomyReceipt = IDL.Record({
    'town_id' : IDL.Opt(IDL.Text),
    'action' : IDL.Text,
    'offer_key' : IDL.Opt(IDL.Text),
    'to_resource' : IDL.Opt(IDL.Text),
    'from_resource' : IDL.Opt(IDL.Text),
    'object_id' : IDL.Opt(IDL.Text),
    'command_id' : IDL.Text,
    'unit_slug' : IDL.Opt(IDL.Text),
    'resources_after' : ResourceBalances,
    'amount_out' : IDL.Nat64,
    'champion_id' : IDL.Opt(IDL.Text),
    'quantity' : IDL.Nat32,
    'amount_in' : IDL.Nat64,
  });
  const AdvancedScenarioReceipt = IDL.Record({
    'event_key' : IDL.Opt(IDL.Text),
    'rule_key' : IDL.Opt(IDL.Text),
    'action' : IDL.Text,
    'command_id' : IDL.Text,
    'resources_after' : IDL.Opt(ResourceBalances),
    'reward_gold' : IDL.Nat32,
    'state' : IDL.Text,
    'quest_key' : IDL.Opt(IDL.Text),
    'current_turn' : IDL.Nat32,
    'objective_key' : IDL.Opt(IDL.Text),
  });
  const CommandResult = IDL.Variant({
    'ChampionMagic' : ChampionMagicReceipt,
    'RecruitPreview' : RecruitPreview,
    'BuildPreview' : BuildPreview,
    'None' : IDL.Null,
    'MovementSync' : MovementSyncOutcome,
    'BattleAction' : BattleActionReceipt,
    'WorldGeneration' : WorldGenerationReceipt,
    'MovementPreview' : MovementPreview,
    'BattleSync' : BattleSyncOutcome,
    'StrategicReceipt' : StrategicCommandReceipt,
    'ExpandedEconomy' : ExpandedEconomyReceipt,
    'AdvancedScenario' : AdvancedScenarioReceipt,
  });
  const ChangedSubject = IDL.Record({
    'subject_kind' : IDL.Text,
    'operation' : IDL.Text,
    'subject_id_text' : IDL.Text,
  });
  const ApiError = IDL.Record({
    'code' : IDL.Text,
    'details_json' : IDL.Opt(IDL.Text),
    'message' : IDL.Text,
    'retryable' : IDL.Bool,
  });
  const ApiEventView = IDL.Record({
    'event_key' : IDL.Text,
    'event_seq' : IDL.Nat64,
    'session_id' : IDL.Text,
    'subject_kind' : IDL.Opt(IDL.Text),
    'turn_number' : IDL.Nat32,
    'audience_key' : IDL.Text,
    'subject_id_text' : IDL.Opt(IDL.Text),
    'event_type' : IDL.Text,
    'payload' : IDL.Opt(IDL.Text),
    'redacted' : IDL.Bool,
  });
  const CommandPhase = IDL.Variant({
    'EffectsApplied' : IDL.Null,
    'Failed' : IDL.Null,
    'Complete' : IDL.Null,
    'Recovered' : IDL.Null,
    'Created' : IDL.Null,
    'Validated' : IDL.Null,
    'EventsApplied' : IDL.Null,
    'Applying' : IDL.Null,
  });
  const CommandResponse = IDL.Record({
    'status' : CommandStatus,
    'result' : CommandResult,
    'changed_subjects' : IDL.Vec(ChangedSubject),
    'command_id' : IDL.Text,
    'error' : IDL.Opt(ApiError),
    'command_type' : IDL.Text,
    'events' : IDL.Vec(ApiEventView),
    'effective_turn' : IDL.Nat32,
    'actor_participant_id' : IDL.Opt(IDL.Text),
    'actor_principal' : IDL.Principal,
    'phase' : CommandPhase,
    'retryable' : IDL.Bool,
    'client_nonce' : IDL.Text,
    'durable_turn' : IDL.Nat32,
    'payload_hash' : IDL.Text,
  });
  const Result = IDL.Variant({ 'Ok' : CommandResponse, 'Err' : ApiError });
  const SessionView = IDL.Record({
    'session_id' : IDL.Text,
    'participant_ids' : IDL.Vec(IDL.Text),
    'state' : IDL.Text,
  });
  const PlayerView = IDL.Record({
    'player_id' : IDL.Text,
    'principal' : IDL.Principal,
    'display_name' : IDL.Text,
  });
  const LobbyCommandResult = IDL.Variant({
    'None' : IDL.Null,
    'Session' : SessionView,
    'Player' : PlayerView,
  });
  const LobbyCommandResponse = IDL.Record({
    'status' : CommandStatus,
    'result' : LobbyCommandResult,
    'changed_subjects' : IDL.Vec(ChangedSubject),
    'command_id' : IDL.Text,
    'error' : IDL.Opt(ApiError),
    'command_type' : IDL.Text,
    'events' : IDL.Vec(ApiEventView),
    'effective_turn' : IDL.Nat32,
    'actor_principal' : IDL.Principal,
    'phase' : CommandPhase,
    'retryable' : IDL.Bool,
    'client_nonce' : IDL.Text,
    'durable_turn' : IDL.Nat32,
    'payload_hash' : IDL.Text,
  });
  const Result_1 = IDL.Variant({
    'Ok' : LobbyCommandResponse,
    'Err' : ApiError,
  });
  const DiagnosticSystemJobView = IDL.Record({
    'last_error' : IDL.Opt(IDL.Text),
    'status' : IDL.Text,
    'due_at_ms' : IDL.Nat64,
    'battle_id' : IDL.Opt(IDL.Text),
    'session_id' : IDL.Text,
    'command_id' : IDL.Opt(IDL.Text),
    'job_key' : IDL.Text,
    'lease_expires_at_ms' : IDL.Opt(IDL.Nat64),
    'attempt_count' : IDL.Nat32,
    'lease_owner' : IDL.Opt(IDL.Text),
    'turn_number' : IDL.Opt(IDL.Nat32),
    'job_kind' : IDL.Text,
    'cursor_json' : IDL.Opt(IDL.Text),
  });
  const Result_2 = IDL.Variant({
    'Ok' : DiagnosticSystemJobView,
    'Err' : ApiError,
  });
  const BattleStackView = IDL.Record({
    'status' : IDL.Text,
    'shots_remaining' : IDL.Nat16,
    'owner_participant_id' : IDL.Opt(IDL.Text),
    'battle_x' : IDL.Nat8,
    'battle_y' : IDL.Nat8,
    'flying' : IDL.Bool,
    'side' : IDL.Text,
    'battle_stack_id' : IDL.Text,
    'champion_guard' : IDL.Int16,
    'front_hp' : IDL.Nat16,
    'defended_round' : IDL.Nat16,
    'acted_round' : IDL.Nat16,
    'speed' : IDL.Nat8,
    'waited_round' : IDL.Nat16,
    'defense' : IDL.Int16,
    'quantity' : IDL.Nat32,
    'ranged' : IDL.Bool,
    'damage_max' : IDL.Nat16,
    'damage_min' : IDL.Nat16,
    'champion_might' : IDL.Int16,
    'max_hp' : IDL.Nat16,
    'unit_id' : IDL.Text,
    'status_keys' : IDL.Vec(IDL.Text),
    'attack' : IDL.Int16,
    'initiative' : IDL.Nat8,
  });
  const BattleCoord = IDL.Record({ 'x' : IDL.Nat8, 'y' : IDL.Nat8 });
  const DamagePreview = IDL.Record({
    'max_damage' : IDL.Nat32,
    'min_damage' : IDL.Nat32,
    'estimated_kills_max' : IDL.Nat32,
    'estimated_kills_min' : IDL.Nat32,
    'target_stack_id' : IDL.Text,
  });
  const LegalBattleAction = IDL.Record({
    'action' : IDL.Text,
    'path' : IDL.Vec(BattleCoord),
    'damage_preview' : IDL.Opt(DamagePreview),
    'ability_key' : IDL.Opt(IDL.Text),
    'enabled' : IDL.Bool,
    'targets' : IDL.Vec(IDL.Text),
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const BattleGridView = IDL.Record({
    'height' : IDL.Nat8,
    'width' : IDL.Nat8,
  });
  const BattleObstacleView = IDL.Record({
    'hp' : IDL.Nat16,
    'height' : IDL.Nat8,
    'battle_x' : IDL.Nat8,
    'battle_y' : IDL.Nat8,
    'obstacle_type' : IDL.Text,
    'battle_obstacle_id' : IDL.Text,
    'state' : IDL.Text,
    'width' : IDL.Nat8,
  });
  const BattleEventView = IDL.Record({
    'event_key' : IDL.Text,
    'event_seq' : IDL.Nat64,
    'subject_id_text' : IDL.Text,
    'event_type' : IDL.Text,
    'payload' : IDL.Text,
  });
  const BattleMoraleLuckPolicy = IDL.Record({
    'morale_disabled_reason' : IDL.Opt(IDL.Text),
    'morale_enabled' : IDL.Bool,
    'luck_enabled' : IDL.Bool,
    'luck_disabled_reason' : IDL.Opt(IDL.Text),
  });
  const BattleView = IDL.Record({
    'stacks' : IDL.Vec(BattleStackView),
    'legal_actions_for_caller' : IDL.Vec(LegalBattleAction),
    'battle_id' : IDL.Text,
    'grid' : BattleGridView,
    'active_participant_id' : IDL.Opt(IDL.Text),
    'action_deadline_at' : IDL.Opt(IDL.Nat64),
    'active_stack_id' : IDL.Opt(IDL.Text),
    'state' : IDL.Text,
    'obstacles' : IDL.Vec(BattleObstacleView),
    'initiative_order' : IDL.Vec(IDL.Text),
    'events' : IDL.Vec(BattleEventView),
    'current_round' : IDL.Nat16,
    'battle_type' : IDL.Text,
    'next_event_seq' : IDL.Nat64,
    'remaining_ms' : IDL.Opt(IDL.Nat64),
    'morale_luck_policy' : BattleMoraleLuckPolicy,
  });
  const Result_3 = IDL.Variant({ 'Ok' : BattleView, 'Err' : ApiError });
  const EndpointKind = IDL.Variant({ 'Update' : IDL.Null, 'Query' : IDL.Null });
  const CanisterEndpointView = IDL.Record({
    'kind' : EndpointKind,
    'name' : IDL.Text,
    'group' : IDL.Text,
    'fixture_mapping' : IDL.Text,
  });
  const ArtifactView = IDL.Record({
    'artifact_id' : IDL.Text,
    'artifact_def_id' : IDL.Text,
    'slot' : IDL.Text,
    'state' : IDL.Text,
  });
  const ChampionArmyStackRecord = IDL.Record({
    'status' : IDL.Text,
    'session_id' : IDL.Text,
    'front_hp' : IDL.Nat16,
    'unit_slug' : IDL.Text,
    'stack_id' : IDL.Text,
    'champion_id' : IDL.Text,
    'quantity' : IDL.Nat32,
    'slot_index' : IDL.Nat8,
    'last_command_id' : IDL.Opt(IDL.Text),
  });
  const ChampionView = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'status' : IDL.Text,
    'vision_radius' : IDL.Nat8,
    'owner_participant_id' : IDL.Text,
    'mana_max' : IDL.Nat16,
    'artifacts' : IDL.Vec(ArtifactView),
    'mana' : IDL.Nat16,
    'name' : IDL.Opt(IDL.Text),
    'spell_slugs' : IDL.Vec(IDL.Text),
    'movement_max' : IDL.Nat16,
    'champion_id' : IDL.Text,
    'skill_keys' : IDL.Vec(IDL.Text),
    'army_stacks' : IDL.Vec(ChampionArmyStackRecord),
    'skill_points' : IDL.Nat16,
    'class_key' : IDL.Text,
    'strength_label' : IDL.Text,
    'class_def_id' : IDL.Text,
    'redacted' : IDL.Bool,
    'effective_movement' : IDL.Nat16,
  });
  const Result_4 = IDL.Variant({ 'Ok' : ChampionView, 'Err' : ApiError });
  const CommandStatusView = IDL.Record({
    'status' : CommandStatus,
    'result_json' : IDL.Opt(IDL.Text),
    'command_id' : IDL.Text,
    'error_message' : IDL.Opt(IDL.Text),
    'phase' : CommandPhase,
    'error_code' : IDL.Opt(IDL.Text),
    'retryable' : IDL.Bool,
  });
  const Result_5 = IDL.Variant({ 'Ok' : CommandStatusView, 'Err' : ApiError });
  const TerrainContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'movement_cost' : IDL.Nat16,
    'passable' : IDL.Bool,
    'name' : IDL.Text,
    'terrain_code' : IDL.Nat8,
    'sprite_key' : IDL.Opt(IDL.Text),
    'terrain_key' : IDL.Text,
  });
  const FactionContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'theme' : IDL.Opt(IDL.Text),
    'banner_key' : IDL.Opt(IDL.Text),
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'icon_key' : IDL.Opt(IDL.Text),
    'trait_key' : IDL.Text,
    'native_terrain' : IDL.Opt(IDL.Text),
  });
  const ArtifactContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'name' : IDL.Text,
    'slot' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'icon_key' : IDL.Opt(IDL.Text),
    'rarity' : IDL.Text,
    'effect_key' : IDL.Text,
  });
  const ChampionClassContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'portrait_key' : IDL.Opt(IDL.Text),
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'faction_slug' : IDL.Opt(IDL.Text),
    'base_movement' : IDL.Nat16,
    'base_vision' : IDL.Nat8,
  });
  const ResourceCost = IDL.Record({
    'aether' : IDL.Nat32,
    'gold' : IDL.Nat32,
    'iron' : IDL.Nat32,
    'wood' : IDL.Nat32,
    'ember' : IDL.Nat32,
    'stone' : IDL.Nat32,
    'crystal' : IDL.Nat32,
  });
  const UnitContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'weekly_growth' : IDL.Nat16,
    'cost' : ResourceCost,
    'animation_key' : IDL.Opt(IDL.Text),
    'flying' : IDL.Bool,
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'tier' : IDL.Nat8,
    'description' : IDL.Opt(IDL.Text),
    'faction_slug' : IDL.Opt(IDL.Text),
    'shots' : IDL.Nat16,
    'icon_key' : IDL.Opt(IDL.Text),
    'speed' : IDL.Nat8,
    'defense' : IDL.Int16,
    'ranged' : IDL.Bool,
    'damage_max' : IDL.Nat16,
    'damage_min' : IDL.Nat16,
    'max_hp' : IDL.Nat16,
    'ability_keys' : IDL.Vec(IDL.Text),
    'attack' : IDL.Int16,
    'initiative' : IDL.Nat8,
    'sprite_key' : IDL.Opt(IDL.Text),
  });
  const SpellContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'mana_cost' : IDL.Nat16,
    'school' : IDL.Text,
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'level' : IDL.Nat8,
    'icon_key' : IDL.Opt(IDL.Text),
    'duration_rounds' : IDL.Nat8,
    'effect_key' : IDL.Text,
    'target_type' : IDL.Text,
  });
  const RulesetContent = IDL.Record({
    'id' : IDL.Text,
    'player_count' : IDL.Nat8,
    'max_turns' : IDL.Nat32,
    'turn_duration_ms' : IDL.Nat32,
    'map_height' : IDL.Nat16,
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'map_width' : IDL.Nat16,
    'version' : IDL.Nat32,
    'content_manifest_hash' : IDL.Text,
    'chunk_size' : IDL.Nat8,
  });
  const MapObjectContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'interaction_key' : IDL.Text,
    'blocking' : IDL.Bool,
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'refresh_rule' : IDL.Text,
    'icon_key' : IDL.Opt(IDL.Text),
    'footprint_h' : IDL.Nat8,
    'footprint_w' : IDL.Nat8,
    'object_type' : IDL.Text,
    'sprite_key' : IDL.Opt(IDL.Text),
  });
  const BuildingContent = IDL.Record({
    'id' : IDL.Text,
    'ruleset_id' : IDL.Text,
    'cost' : ResourceCost,
    'name' : IDL.Text,
    'slug' : IDL.Text,
    'description' : IDL.Opt(IDL.Text),
    'faction_slug' : IDL.Opt(IDL.Text),
    'icon_key' : IDL.Opt(IDL.Text),
    'requires_building_slugs' : IDL.Vec(IDL.Text),
    'building_type' : IDL.Text,
    'effect_key' : IDL.Opt(IDL.Text),
    'unlocks_unit_slug' : IDL.Opt(IDL.Text),
  });
  const ContentManifest = IDL.Record({
    'asset_keys' : IDL.Vec(IDL.Text),
    'terrain' : IDL.Vec(TerrainContent),
    'factions' : IDL.Vec(FactionContent),
    'artifacts' : IDL.Vec(ArtifactContent),
    'champion_classes' : IDL.Vec(ChampionClassContent),
    'units' : IDL.Vec(UnitContent),
    'spells' : IDL.Vec(SpellContent),
    'ruleset' : RulesetContent,
    'map_objects' : IDL.Vec(MapObjectContent),
    'buildings' : IDL.Vec(BuildingContent),
  });
  const ContentManifestResponse = IDL.Record({ 'manifest' : ContentManifest });
  const Result_6 = IDL.Variant({
    'Ok' : ContentManifestResponse,
    'Err' : ApiError,
  });
  const DiagnosticProjectionFlushView = IDL.Record({
    'queue_len_after' : IDL.Nat64,
    'entries_processed' : IDL.Nat64,
    'stable_pages_delta' : IDL.Nat64,
    'queue_len_before' : IDL.Nat64,
    'flushed_at_ms' : IDL.Nat64,
    'flush_truncated' : IDL.Bool,
    'flush_instructions' : IDL.Nat64,
    'rows_flushed' : IDL.Nat64,
  });
  const DiagnosticProjectionKernelView = IDL.Record({
    'dirty_queue_len' : IDL.Nat64,
    'session_id' : IDL.Text,
    'oldest_dirty_age_ms' : IDL.Opt(IDL.Nat64),
    'lag_ms' : IDL.Nat64,
    'lag_generations' : IDL.Nat64,
    'flushed_at_ms' : IDL.Nat64,
    'kernel_id' : IDL.Text,
    'pending_entries' : IDL.Nat64,
    'kernel_generation' : IDL.Nat64,
    'turn_number' : IDL.Nat32,
    'flushed_generation' : IDL.Nat64,
  });
  const DiagnosticProjectionSnapshot = IDL.Record({
    'oldest_dirty_age_ms' : IDL.Opt(IDL.Nat64),
    'total_dirty_queue_len' : IDL.Nat64,
    'last_flush' : IDL.Opt(DiagnosticProjectionFlushView),
    'kernels' : IDL.Vec(DiagnosticProjectionKernelView),
  });
  const Result_7 = IDL.Variant({
    'Ok' : DiagnosticProjectionSnapshot,
    'Err' : ApiError,
  });
  const DiagnosticRowCount = IDL.Record({
    'entity' : IDL.Text,
    'count' : IDL.Nat32,
  });
  const DiagnosticStorageSnapshot = IDL.Record({
    'stable_memory_pages' : IDL.Nat64,
    'total_rows' : IDL.Nat32,
    'row_counts' : IDL.Vec(DiagnosticRowCount),
  });
  const Result_8 = IDL.Variant({
    'Ok' : DiagnosticStorageSnapshot,
    'Err' : ApiError,
  });
  const DiagnosticSystemJobPage = IDL.Record({
    'jobs' : IDL.Vec(DiagnosticSystemJobView),
    'limit' : IDL.Nat32,
    'next_cursor' : IDL.Opt(IDL.Text),
  });
  const Result_9 = IDL.Variant({
    'Ok' : DiagnosticSystemJobPage,
    'Err' : ApiError,
  });
  const DwellingPoolView = IDL.Record({
    'owner_participant_id' : IDL.Opt(IDL.Text),
    'direct_recruit' : IDL.Bool,
    'growth_per_week' : IDL.Nat16,
    'object_id' : IDL.Text,
    'unit_slug' : IDL.Text,
    'available' : IDL.Nat32,
    'last_growth_week' : IDL.Nat32,
  });
  const Result_10 = IDL.Variant({ 'Ok' : DwellingPoolView, 'Err' : ApiError });
  const EventPageInfo = IDL.Record({
    'limit' : IDL.Nat32,
    'next_event_seq' : IDL.Opt(IDL.Nat64),
    'has_more' : IDL.Bool,
  });
  const ApiEventPage = IDL.Record({
    'page_info' : EventPageInfo,
    'events' : IDL.Vec(ApiEventView),
  });
  const Result_11 = IDL.Variant({ 'Ok' : ApiEventPage, 'Err' : ApiError });
  const Viewport = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'height' : IDL.Nat16,
    'width' : IDL.Nat16,
  });
  const GameViewRequest = IDL.Record({
    'object_limit' : IDL.Nat32,
    'include_battle' : IDL.Bool,
    'event_limit' : IDL.Nat32,
    'viewport' : Viewport,
    'chunk_limit' : IDL.Nat32,
    'object_cursor' : IDL.Opt(IDL.Nat32),
    'chunk_cursor' : IDL.Opt(IDL.Nat32),
    'events_after_seq' : IDL.Nat64,
  });
  const MapChunkView = IDL.Record({
    'height' : IDL.Nat16,
    'movement_blob' : IDL.Vec(IDL.Nat8),
    'chunk_x' : IDL.Nat16,
    'chunk_y' : IDL.Nat16,
    'discovered_blob' : IDL.Vec(IDL.Nat8),
    'chunk_id' : IDL.Text,
    'width' : IDL.Nat16,
    'visible_blob' : IDL.Vec(IDL.Nat8),
    'terrain_blob' : IDL.Vec(IDL.Nat8),
    'flags_blob' : IDL.Vec(IDL.Nat8),
  });
  const TownRecord = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'status' : IDL.Text,
    'town_id' : IDL.Text,
    'owner_participant_id' : IDL.Text,
    'income_started_turn' : IDL.Nat32,
    'hall_level' : IDL.Nat8,
    'session_id' : IDL.Text,
    'unrest_until_turn' : IDL.Nat32,
    'name' : IDL.Text,
    'faction_slug' : IDL.Text,
    'captured_turn' : IDL.Nat32,
    'fort_level' : IDL.Nat8,
    'last_command_id' : IDL.Opt(IDL.Text),
    'last_built_turn' : IDL.Nat32,
  });
  const ArmyStackRecord = IDL.Record({
    'status' : IDL.Text,
    'session_id' : IDL.Text,
    'front_hp' : IDL.Nat16,
    'unit_slug' : IDL.Text,
    'owner_id' : IDL.Text,
    'stack_id' : IDL.Text,
    'quantity' : IDL.Nat32,
    'slot_index' : IDL.Nat8,
    'last_command_id' : IDL.Opt(IDL.Text),
    'owner_kind' : IDL.Text,
  });
  const TownRecruitPoolRecord = IDL.Record({
    'town_id' : IDL.Text,
    'session_id' : IDL.Text,
    'unit_slug' : IDL.Text,
    'available' : IDL.Nat32,
    'pool_id' : IDL.Text,
    'last_command_id' : IDL.Opt(IDL.Text),
    'last_growth_week' : IDL.Nat32,
  });
  const TownBuildingRecord = IDL.Record({
    'town_id' : IDL.Text,
    'session_id' : IDL.Text,
    'built_turn' : IDL.Nat32,
    'building_slug' : IDL.Text,
    'building_id' : IDL.Text,
  });
  const ApiTownView = IDL.Record({
    'town' : TownRecord,
    'garrison_stacks' : IDL.Vec(ArmyStackRecord),
    'recruit_pools' : IDL.Vec(TownRecruitPoolRecord),
    'buildings' : IDL.Vec(TownBuildingRecord),
  });
  const BattleSummary = IDL.Record({
    'battle_id' : IDL.Text,
    'active_participant_id' : IDL.Opt(IDL.Text),
    'active_stack_id' : IDL.Opt(IDL.Text),
    'state' : IDL.Text,
    'current_round' : IDL.Nat16,
    'battle_type' : IDL.Text,
  });
  const RenderTimeMeta = IDL.Record({
    'sync_required' : IDL.Bool,
    'turn_started_at_ms' : IDL.Nat64,
    'turn_duration_ms' : IDL.Nat64,
    'server_now_ms' : IDL.Nat64,
  });
  const PageInfo = IDL.Record({
    'limit' : IDL.Nat32,
    'next_cursor' : IDL.Opt(IDL.Nat32),
    'has_more' : IDL.Bool,
  });
  const ParticipantSummary = IDL.Record({
    'player_id' : IDL.Text,
    'status' : IDL.Text,
    'resources' : ResourceBalances,
    'faction_slug' : IDL.Text,
    'slot_index' : IDL.Nat8,
    'participant_id' : IDL.Text,
    'ready' : IDL.Bool,
  });
  const ObjectView = IDL.Record({
    'x' : IDL.Nat16,
    'y' : IDL.Nat16,
    'owner_participant_id' : IDL.Opt(IDL.Text),
    'subject_kind' : IDL.Text,
    'display_name' : IDL.Opt(IDL.Text),
    'details_json' : IDL.Text,
    'last_seen_turn' : IDL.Opt(IDL.Nat32),
    'asset_key' : IDL.Opt(IDL.Text),
    'visibility' : IDL.Text,
    'subject_id_text' : IDL.Text,
    'redaction_level' : IDL.Text,
  });
  const SessionSummary = IDL.Record({
    'session_id' : IDL.Text,
    'participant_ids' : IDL.Vec(IDL.Text),
    'state' : IDL.Text,
    'current_turn' : IDL.Nat32,
  });
  const ActionAffordance = IDL.Record({
    'action' : IDL.Text,
    'target_id' : IDL.Opt(IDL.Text),
    'enabled' : IDL.Bool,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const GameView = IDL.Record({
    'map_chunks' : IDL.Vec(MapChunkView),
    'towns' : IDL.Vec(ApiTownView),
    'event_page_info' : EventPageInfo,
    'omitted_fields' : IDL.Vec(IDL.Text),
    'battle_summary' : IDL.Opt(BattleSummary),
    'render_time' : RenderTimeMeta,
    'battle' : IDL.Opt(BattleView),
    'map_page_info' : PageInfo,
    'participant' : ParticipantSummary,
    'objects' : IDL.Vec(ObjectView),
    'session' : SessionSummary,
    'events' : IDL.Vec(ApiEventView),
    'content_manifest_hash' : IDL.Text,
    'viewport' : Viewport,
    'action_affordances' : IDL.Vec(ActionAffordance),
    'object_page_info' : PageInfo,
    'champions' : IDL.Vec(ChampionView),
  });
  const Result_12 = IDL.Variant({ 'Ok' : GameView, 'Err' : ApiError });
  const MatchHistoryEntry = IDL.Record({
    'result' : IDL.Text,
    'session_id' : IDL.Text,
    'opponent_name' : IDL.Opt(IDL.Text),
    'turns_played' : IDL.Nat32,
    'summary_json' : IDL.Opt(IDL.Text),
  });
  const MatchHistoryPage = IDL.Record({
    'page_info' : PageInfo,
    'entries' : IDL.Vec(MatchHistoryEntry),
  });
  const Result_13 = IDL.Variant({ 'Ok' : MatchHistoryPage, 'Err' : ApiError });
  const Result_14 = IDL.Variant({
    'Ok' : IDL.Vec(ChampionView),
    'Err' : ApiError,
  });
  const ParticipantView = IDL.Record({
    'player_id' : IDL.Text,
    'status' : IDL.Text,
    'session_id' : IDL.Text,
    'resources' : ResourceCost,
    'faction_slug' : IDL.Text,
    'slot_index' : IDL.Nat8,
    'participant_id' : IDL.Text,
    'ready' : IDL.Bool,
  });
  const Result_15 = IDL.Variant({ 'Ok' : ParticipantView, 'Err' : ApiError });
  const Result_16 = IDL.Variant({ 'Ok' : PlayerView, 'Err' : ApiError });
  const NavalRouteRecord = IDL.Record({
    'status' : IDL.Text,
    'boat_required' : IDL.Bool,
    'to_x' : IDL.Nat16,
    'to_y' : IDL.Nat16,
    'actionable' : IDL.Bool,
    'route_key' : IDL.Text,
    'from_x' : IDL.Nat16,
    'from_y' : IDL.Nat16,
    'disabled_reason' : IDL.Opt(IDL.Text),
    'water_crossings' : IDL.Nat16,
  });
  const NavalRoutesView = IDL.Record({
    'session_id' : IDL.Text,
    'current_turn' : IDL.Nat32,
    'routes' : IDL.Vec(NavalRouteRecord),
  });
  const Result_17 = IDL.Variant({ 'Ok' : NavalRoutesView, 'Err' : ApiError });
  const Result_18 = IDL.Variant({ 'Ok' : ObjectView, 'Err' : ApiError });
  const ObjectiveProgressRecord = IDL.Record({
    'status' : IDL.Text,
    'owner_participant_id' : IDL.Opt(IDL.Text),
    'objective_type' : IDL.Text,
    'object_id' : IDL.Opt(IDL.Text),
    'last_scored_turn' : IDL.Nat32,
    'visible_to' : IDL.Text,
    'objective_key' : IDL.Text,
    'required_value' : IDL.Nat32,
    'redacted' : IDL.Bool,
    'progress_value' : IDL.Nat32,
  });
  const ObjectiveProgressView = IDL.Record({
    'session_id' : IDL.Text,
    'objectives' : IDL.Vec(ObjectiveProgressRecord),
  });
  const Result_19 = IDL.Variant({
    'Ok' : ObjectiveProgressView,
    'Err' : ApiError,
  });
  const ProceduralMapRecord = IDL.Record({
    'status' : IDL.Text,
    'scenario_hash' : IDL.Text,
    'road_tile_count' : IDL.Nat32,
    'map_height' : IDL.Nat16,
    'town_count' : IDL.Nat32,
    'water_tile_count' : IDL.Nat32,
    'generation_key' : IDL.Text,
    'map_width' : IDL.Nat16,
    'land_tile_count' : IDL.Nat32,
    'chunk_count' : IDL.Nat32,
    'mine_count' : IDL.Nat32,
    'generated_turn' : IDL.Nat32,
    'chunk_size' : IDL.Nat8,
    'map_seed' : IDL.Nat64,
  });
  const ProceduralMapView = IDL.Record({
    'session_id' : IDL.Text,
    'maps' : IDL.Vec(ProceduralMapRecord),
    'current_turn' : IDL.Nat32,
  });
  const Result_20 = IDL.Variant({ 'Ok' : ProceduralMapView, 'Err' : ApiError });
  const ScenarioRuleView = IDL.Record({
    'rule_type' : IDL.Text,
    'status' : IDL.Text,
    'rule_key' : IDL.Text,
    'owner_participant_id' : IDL.Opt(IDL.Text),
    'victory_state' : IDL.Text,
    'last_checked_turn' : IDL.Nat32,
    'winner_participant_id' : IDL.Opt(IDL.Text),
    'disabled_reason' : IDL.Opt(IDL.Text),
    'current_value' : IDL.Nat32,
    'required_value' : IDL.Nat32,
  });
  const ScenarioRulesView = IDL.Record({
    'session_id' : IDL.Text,
    'current_turn' : IDL.Nat32,
    'rules' : IDL.Vec(ScenarioRuleView),
  });
  const Result_21 = IDL.Variant({ 'Ok' : ScenarioRulesView, 'Err' : ApiError });
  const Result_22 = IDL.Variant({ 'Ok' : SessionView, 'Err' : ApiError });
  const SetupProgressView = IDL.Record({
    'session_id' : IDL.Text,
    'setup_job_attempt_count' : IDL.Nat32,
    'total_effect_count' : IDL.Nat32,
    'setup_command_id' : IDL.Opt(IDL.Text),
    'session_state' : IDL.Text,
    'last_effect_key' : IDL.Opt(IDL.Text),
    'setup_command_status' : IDL.Opt(IDL.Text),
    'setup_job_status' : IDL.Opt(IDL.Text),
    'completed_effect_count' : IDL.Nat32,
    'setup_complete' : IDL.Bool,
    'next_effect_key' : IDL.Opt(IDL.Text),
  });
  const Result_23 = IDL.Variant({ 'Ok' : SetupProgressView, 'Err' : ApiError });
  const SiegeRuleRecord = IDL.Record({
    'status' : IDL.Text,
    'rule_key' : IDL.Text,
    'battle_obstacle_cap' : IDL.Nat16,
    'tower_count' : IDL.Nat8,
    'actionable' : IDL.Bool,
    'wall_segments' : IDL.Nat16,
    'disabled_reason' : IDL.Opt(IDL.Text),
    'fortification_level' : IDL.Text,
    'siege_engine_slots' : IDL.Nat8,
    'gate_count' : IDL.Nat8,
  });
  const SiegeRulesView = IDL.Record({
    'session_id' : IDL.Text,
    'current_turn' : IDL.Nat32,
    'rules' : IDL.Vec(SiegeRuleRecord),
  });
  const Result_24 = IDL.Variant({ 'Ok' : SiegeRulesView, 'Err' : ApiError });
  const SkirmishSettingsRecord = IDL.Record({
    'larger_map_enabled' : IDL.Bool,
    'status' : IDL.Text,
    'player_count' : IDL.Nat8,
    'neutral_difficulty' : IDL.Text,
    'map_height' : IDL.Nat16,
    'fog_enabled' : IDL.Bool,
    'generation_key' : IDL.Text,
    'siege_enabled' : IDL.Bool,
    'map_width' : IDL.Nat16,
    'victory_condition' : IDL.Text,
    'chunk_size' : IDL.Nat8,
    'naval_enabled' : IDL.Bool,
    'map_seed' : IDL.Nat64,
    'profile_key' : IDL.Text,
  });
  const SkirmishSettingsView = IDL.Record({
    'session_id' : IDL.Text,
    'settings' : SkirmishSettingsRecord,
    'current_turn' : IDL.Nat32,
  });
  const Result_25 = IDL.Variant({
    'Ok' : SkirmishSettingsView,
    'Err' : ApiError,
  });
  const TavernOfferView = IDL.Record({
    'status' : IDL.Text,
    'town_id' : IDL.Text,
    'offer_key' : IDL.Text,
    'offer_slot' : IDL.Nat8,
    'hired_champion_id' : IDL.Opt(IDL.Text),
    'cost_gold' : IDL.Nat32,
    'candidate_name' : IDL.Text,
    'champion_class_slug' : IDL.Text,
    'week_number' : IDL.Nat32,
  });
  const TavernOffersView = IDL.Record({
    'town_id' : IDL.Text,
    'offers' : IDL.Vec(TavernOfferView),
    'week_number' : IDL.Nat32,
  });
  const Result_26 = IDL.Variant({ 'Ok' : TavernOffersView, 'Err' : ApiError });
  const Result_27 = IDL.Variant({ 'Ok' : ApiTownView, 'Err' : ApiError });
  const MapChunkPage = IDL.Record({
    'next_cursor' : IDL.Opt(IDL.Nat32),
    'chunks' : IDL.Vec(MapChunkView),
    'has_more' : IDL.Bool,
  });
  const Result_28 = IDL.Variant({ 'Ok' : MapChunkPage, 'Err' : ApiError });
  const ObjectViewPage = IDL.Record({
    'objects' : IDL.Vec(ObjectView),
    'next_cursor' : IDL.Opt(IDL.Nat32),
    'has_more' : IDL.Bool,
  });
  const Result_29 = IDL.Variant({ 'Ok' : ObjectViewPage, 'Err' : ApiError });
  const WorldEventView = IDL.Record({
    'event_key' : IDL.Text,
    'status' : IDL.Text,
    'ends_turn' : IDL.Nat32,
    'starts_turn' : IDL.Nat32,
    'event_window' : IDL.Text,
    'event_type' : IDL.Text,
    'payload' : IDL.Opt(IDL.Text),
    'redacted' : IDL.Bool,
  });
  const WorldEventsView = IDL.Record({
    'session_id' : IDL.Text,
    'events' : IDL.Vec(WorldEventView),
    'current_turn' : IDL.Nat32,
  });
  const Result_30 = IDL.Variant({ 'Ok' : WorldEventsView, 'Err' : ApiError });
  const EntitySummary = IDL.Record({
    'schema_reconcile_rejected_field_slot' : IDL.Nat64,
    'plan_index_multi_lookup' : IDL.Nat64,
    'sql_compile_reject_parse' : IDL.Nat64,
    'schema_transition_rejected_row_layout' : IDL.Nat64,
    'reverse_index_removes' : IDL.Nat64,
    'cache_shared_query_plan_misses' : IDL.Nat64,
    'schema_reconcile_rejected_other' : IDL.Nat64,
    'cache_sql_compiled_command_miss_method_version' : IDL.Nat64,
    'load_candidate_rows_scanned' : IDL.Nat64,
    'schema_store_snapshots' : IDL.Nat64,
    'relation_reverse_lookups' : IDL.Nat64,
    'prepared_shape_generated_fallback' : IDL.Nat64,
    'load_result_rows_emitted' : IDL.Nat64,
    'index_removes' : IDL.Nat64,
    'schema_transition_exact_match' : IDL.Nat64,
    'plan_grouped_ordered_materialized' : IDL.Nat64,
    'plan_by_keys' : IDL.Nat64,
    'plan_keys' : IDL.Nat64,
    'write_rows_touched' : IDL.Nat64,
    'cache_shared_query_plan_hits' : IDL.Nat64,
    'rows_updated' : IDL.Nat64,
    'plan_full_scan' : IDL.Nat64,
    'save_insert_calls' : IDL.Nat64,
    'exec_aborted' : IDL.Nat64,
    'cache_sql_compiled_command_miss_distinct_key' : IDL.Nat64,
    'cache_shared_query_plan_miss_visibility' : IDL.Nat64,
    'cache_sql_compiled_command_misses' : IDL.Nat64,
    'sql_write_error_delete' : IDL.Nat64,
    'plan_choice_planner_full_scan_fallback' : IDL.Nat64,
    'rows_filtered' : IDL.Nat64,
    'cache_sql_compiled_command_miss_schema_fingerprint' : IDL.Nat64,
    'load_candidate_rows_filtered' : IDL.Nat64,
    'rows_replaced' : IDL.Nat64,
    'schema_reconcile_rejected_row_layout' : IDL.Nat64,
    'plan_choice_required_order_primary_key_range_preferred' : IDL.Nat64,
    'sql_write_error_not_found' : IDL.Nat64,
    'cache_sql_compiled_command_hits' : IDL.Nat64,
    'cache_shared_query_plan_miss_method_version' : IDL.Nat64,
    'rows_emitted' : IDL.Nat64,
    'sql_update_calls' : IDL.Nat64,
    'sql_write_error_update' : IDL.Nat64,
    'rows_loaded' : IDL.Nat64,
    'path' : IDL.Text,
    'sql_write_error_insert_select' : IDL.Nat64,
    'plan_union' : IDL.Nat64,
    'schema_transition_rejected_field_contract' : IDL.Nat64,
    'save_calls' : IDL.Nat64,
    'sql_write_error_incompatible_persisted_format' : IDL.Nat64,
    'schema_store_latest_snapshot_bytes' : IDL.Nat64,
    'schema_transition_rejected_snapshot' : IDL.Nat64,
    'accepted_schema_nested_leaf_facts' : IDL.Nat64,
    'schema_store_encoded_bytes' : IDL.Nat64,
    'sql_write_error_insert' : IDL.Nat64,
    'plan_index' : IDL.Nat64,
    'schema_reconcile_store_write_error' : IDL.Nat64,
    'cache_shared_query_plan_miss_cold' : IDL.Nat64,
    'plan_choice_planner_composite_non_index' : IDL.Nat64,
    'delete_calls' : IDL.Nat64,
    'sql_insert_select_calls' : IDL.Nat64,
    'relation_delete_blocks' : IDL.Nat64,
    'unique_violations' : IDL.Nat64,
    'sql_write_returning_rows' : IDL.Nat64,
    'accepted_schema_fields' : IDL.Nat64,
    'plan_key_range' : IDL.Nat64,
    'cache_sql_compiled_command_inserts' : IDL.Nat64,
    'schema_reconcile_latest_snapshot_corrupt' : IDL.Nat64,
    'schema_transition_checks' : IDL.Nat64,
    'plan_range' : IDL.Nat64,
    'plan_by_key' : IDL.Nat64,
    'cache_shared_query_plan_miss_distinct_key' : IDL.Nat64,
    'plan_choice_limit_zero_window' : IDL.Nat64,
    'cache_sql_compiled_command_miss_surface' : IDL.Nat64,
    'plan_grouped_hash_materialized' : IDL.Nat64,
    'schema_reconcile_exact_match' : IDL.Nat64,
    'plan_choice_constant_false_predicate' : IDL.Nat64,
    'exec_error_unsupported' : IDL.Nat64,
    'sql_write_matched_rows' : IDL.Nat64,
    'rows_deleted' : IDL.Nat64,
    'reverse_index_inserts' : IDL.Nat64,
    'plan_choice_intent_key_access_override' : IDL.Nat64,
    'write_relation_checks' : IDL.Nat64,
    'exec_error_invariant_violation' : IDL.Nat64,
    'sql_write_error_invariant_violation' : IDL.Nat64,
    'exec_error_corruption' : IDL.Nat64,
    'exec_error_not_found' : IDL.Nat64,
    'schema_transition_append_only_nullable_fields' : IDL.Nat64,
    'cache_shared_query_plan_inserts' : IDL.Nat64,
    'sql_compile_rejects' : IDL.Nat64,
    'exec_error_conflict' : IDL.Nat64,
    'plan_explicit_full_scan' : IDL.Nat64,
    'index_inserts' : IDL.Nat64,
    'exec_error_internal' : IDL.Nat64,
    'exec_error_incompatible_persisted_format' : IDL.Nat64,
    'sql_delete_calls' : IDL.Nat64,
    'sql_write_mutated_rows' : IDL.Nat64,
    'save_update_calls' : IDL.Nat64,
    'sql_compile_reject_cache_key' : IDL.Nat64,
    'schema_transition_rejected_schema_version' : IDL.Nat64,
    'plan_choice_planner_primary_key_range' : IDL.Nat64,
    'exec_success' : IDL.Nat64,
    'rows_aggregated' : IDL.Nat64,
    'sql_compile_reject_semantic' : IDL.Nat64,
    'load_calls' : IDL.Nat64,
    'plan_choice_full_scan_access' : IDL.Nat64,
    'sql_insert_calls' : IDL.Nat64,
    'schema_reconcile_rejected_schema_version' : IDL.Nat64,
    'write_index_entries_changed' : IDL.Nat64,
    'plan_choice_non_index_access' : IDL.Nat64,
    'schema_transition_rejected_field_slot' : IDL.Nat64,
    'cache_sql_compiled_command_miss_cold' : IDL.Nat64,
    'rows_saved' : IDL.Nat64,
    'plan_choice_conflicting_primary_key_children_access_preferred' : IDL.Nat64,
    'prepared_shape_already_finalized' : IDL.Nat64,
    'plan_intersection' : IDL.Nat64,
    'schema_transition_rejected_entity_identity' : IDL.Nat64,
    'non_atomic_partial_commits' : IDL.Nat64,
    'non_atomic_partial_rows_committed' : IDL.Nat64,
    'sql_write_error_conflict' : IDL.Nat64,
    'plan_choice_planner_primary_key_lookup' : IDL.Nat64,
    'rows_inserted' : IDL.Nat64,
    'sql_write_error_internal' : IDL.Nat64,
    'save_replace_calls' : IDL.Nat64,
    'plan_index_prefix' : IDL.Nat64,
    'rows_scanned' : IDL.Nat64,
    'write_reverse_index_entries_changed' : IDL.Nat64,
    'plan_choice_singleton_primary_key_child_access_preferred' : IDL.Nat64,
    'cache_shared_query_plan_miss_schema_fingerprint' : IDL.Nat64,
    'sql_write_error_unsupported' : IDL.Nat64,
    'plan_index_range' : IDL.Nat64,
    'schema_reconcile_first_create' : IDL.Nat64,
    'schema_reconcile_checks' : IDL.Nat64,
    'sql_write_error_corruption' : IDL.Nat64,
    'plan_choice_planner_key_set_access' : IDL.Nat64,
    'plan_choice_empty_child_access_preferred' : IDL.Nat64,
  });
  const EventOps = IDL.Record({
    'schema_reconcile_rejected_field_slot' : IDL.Nat64,
    'plan_index_multi_lookup' : IDL.Nat64,
    'sql_compile_reject_parse' : IDL.Nat64,
    'schema_transition_rejected_row_layout' : IDL.Nat64,
    'reverse_index_removes' : IDL.Nat64,
    'cache_shared_query_plan_misses' : IDL.Nat64,
    'schema_reconcile_rejected_other' : IDL.Nat64,
    'cache_sql_compiled_command_miss_method_version' : IDL.Nat64,
    'load_candidate_rows_scanned' : IDL.Nat64,
    'schema_store_snapshots' : IDL.Nat64,
    'relation_reverse_lookups' : IDL.Nat64,
    'prepared_shape_generated_fallback' : IDL.Nat64,
    'load_result_rows_emitted' : IDL.Nat64,
    'index_removes' : IDL.Nat64,
    'schema_transition_exact_match' : IDL.Nat64,
    'plan_grouped_ordered_materialized' : IDL.Nat64,
    'plan_by_keys' : IDL.Nat64,
    'plan_keys' : IDL.Nat64,
    'write_rows_touched' : IDL.Nat64,
    'cache_shared_query_plan_hits' : IDL.Nat64,
    'rows_updated' : IDL.Nat64,
    'plan_full_scan' : IDL.Nat64,
    'save_insert_calls' : IDL.Nat64,
    'exec_aborted' : IDL.Nat64,
    'cache_sql_compiled_command_miss_distinct_key' : IDL.Nat64,
    'cache_shared_query_plan_miss_visibility' : IDL.Nat64,
    'cache_sql_compiled_command_misses' : IDL.Nat64,
    'sql_write_error_delete' : IDL.Nat64,
    'plan_choice_planner_full_scan_fallback' : IDL.Nat64,
    'rows_filtered' : IDL.Nat64,
    'cache_sql_compiled_command_miss_schema_fingerprint' : IDL.Nat64,
    'load_candidate_rows_filtered' : IDL.Nat64,
    'rows_replaced' : IDL.Nat64,
    'schema_reconcile_rejected_row_layout' : IDL.Nat64,
    'plan_choice_required_order_primary_key_range_preferred' : IDL.Nat64,
    'sql_write_error_not_found' : IDL.Nat64,
    'cache_sql_compiled_command_hits' : IDL.Nat64,
    'cache_shared_query_plan_miss_method_version' : IDL.Nat64,
    'rows_emitted' : IDL.Nat64,
    'sql_update_calls' : IDL.Nat64,
    'sql_write_error_update' : IDL.Nat64,
    'rows_loaded' : IDL.Nat64,
    'sql_write_error_insert_select' : IDL.Nat64,
    'plan_union' : IDL.Nat64,
    'schema_transition_rejected_field_contract' : IDL.Nat64,
    'save_calls' : IDL.Nat64,
    'sql_write_error_incompatible_persisted_format' : IDL.Nat64,
    'schema_store_latest_snapshot_bytes' : IDL.Nat64,
    'schema_transition_rejected_snapshot' : IDL.Nat64,
    'accepted_schema_nested_leaf_facts' : IDL.Nat64,
    'schema_store_encoded_bytes' : IDL.Nat64,
    'sql_write_error_insert' : IDL.Nat64,
    'plan_index' : IDL.Nat64,
    'schema_reconcile_store_write_error' : IDL.Nat64,
    'cache_shared_query_plan_miss_cold' : IDL.Nat64,
    'plan_choice_planner_composite_non_index' : IDL.Nat64,
    'delete_calls' : IDL.Nat64,
    'sql_insert_select_calls' : IDL.Nat64,
    'relation_delete_blocks' : IDL.Nat64,
    'unique_violations' : IDL.Nat64,
    'cache_sql_compiled_command_entries' : IDL.Nat64,
    'sql_write_returning_rows' : IDL.Nat64,
    'accepted_schema_fields' : IDL.Nat64,
    'plan_key_range' : IDL.Nat64,
    'cache_sql_compiled_command_inserts' : IDL.Nat64,
    'schema_reconcile_latest_snapshot_corrupt' : IDL.Nat64,
    'schema_transition_checks' : IDL.Nat64,
    'plan_range' : IDL.Nat64,
    'plan_by_key' : IDL.Nat64,
    'cache_shared_query_plan_miss_distinct_key' : IDL.Nat64,
    'plan_choice_limit_zero_window' : IDL.Nat64,
    'cache_sql_compiled_command_miss_surface' : IDL.Nat64,
    'plan_grouped_hash_materialized' : IDL.Nat64,
    'schema_reconcile_exact_match' : IDL.Nat64,
    'plan_choice_constant_false_predicate' : IDL.Nat64,
    'exec_error_unsupported' : IDL.Nat64,
    'sql_write_matched_rows' : IDL.Nat64,
    'rows_deleted' : IDL.Nat64,
    'reverse_index_inserts' : IDL.Nat64,
    'cache_shared_query_plan_entries' : IDL.Nat64,
    'plan_choice_intent_key_access_override' : IDL.Nat64,
    'write_relation_checks' : IDL.Nat64,
    'exec_error_invariant_violation' : IDL.Nat64,
    'sql_write_error_invariant_violation' : IDL.Nat64,
    'exec_error_corruption' : IDL.Nat64,
    'exec_error_not_found' : IDL.Nat64,
    'schema_transition_append_only_nullable_fields' : IDL.Nat64,
    'cache_shared_query_plan_inserts' : IDL.Nat64,
    'sql_compile_rejects' : IDL.Nat64,
    'exec_error_conflict' : IDL.Nat64,
    'plan_explicit_full_scan' : IDL.Nat64,
    'index_inserts' : IDL.Nat64,
    'exec_error_internal' : IDL.Nat64,
    'exec_error_incompatible_persisted_format' : IDL.Nat64,
    'sql_delete_calls' : IDL.Nat64,
    'sql_write_mutated_rows' : IDL.Nat64,
    'save_update_calls' : IDL.Nat64,
    'sql_compile_reject_cache_key' : IDL.Nat64,
    'schema_transition_rejected_schema_version' : IDL.Nat64,
    'plan_choice_planner_primary_key_range' : IDL.Nat64,
    'exec_success' : IDL.Nat64,
    'rows_aggregated' : IDL.Nat64,
    'sql_compile_reject_semantic' : IDL.Nat64,
    'load_calls' : IDL.Nat64,
    'plan_choice_full_scan_access' : IDL.Nat64,
    'sql_insert_calls' : IDL.Nat64,
    'schema_reconcile_rejected_schema_version' : IDL.Nat64,
    'write_index_entries_changed' : IDL.Nat64,
    'plan_choice_non_index_access' : IDL.Nat64,
    'schema_transition_rejected_field_slot' : IDL.Nat64,
    'cache_sql_compiled_command_miss_cold' : IDL.Nat64,
    'rows_saved' : IDL.Nat64,
    'plan_choice_conflicting_primary_key_children_access_preferred' : IDL.Nat64,
    'prepared_shape_already_finalized' : IDL.Nat64,
    'plan_intersection' : IDL.Nat64,
    'schema_transition_rejected_entity_identity' : IDL.Nat64,
    'non_atomic_partial_commits' : IDL.Nat64,
    'non_atomic_partial_rows_committed' : IDL.Nat64,
    'sql_write_error_conflict' : IDL.Nat64,
    'plan_choice_planner_primary_key_lookup' : IDL.Nat64,
    'rows_inserted' : IDL.Nat64,
    'sql_write_error_internal' : IDL.Nat64,
    'save_replace_calls' : IDL.Nat64,
    'plan_index_prefix' : IDL.Nat64,
    'rows_scanned' : IDL.Nat64,
    'write_reverse_index_entries_changed' : IDL.Nat64,
    'plan_choice_singleton_primary_key_child_access_preferred' : IDL.Nat64,
    'cache_shared_query_plan_miss_schema_fingerprint' : IDL.Nat64,
    'sql_write_error_unsupported' : IDL.Nat64,
    'plan_index_range' : IDL.Nat64,
    'schema_reconcile_first_create' : IDL.Nat64,
    'schema_reconcile_checks' : IDL.Nat64,
    'sql_write_error_corruption' : IDL.Nat64,
    'plan_choice_planner_key_set_access' : IDL.Nat64,
    'plan_choice_empty_child_access_preferred' : IDL.Nat64,
  });
  const EventPerf = IDL.Record({
    'save_inst_max' : IDL.Nat64,
    'delete_inst_max' : IDL.Nat64,
    'load_inst_total' : IDL.Nat,
    'load_inst_max' : IDL.Nat64,
    'save_inst_total' : IDL.Nat,
    'delete_inst_total' : IDL.Nat,
  });
  const EventCounters = IDL.Record({
    'ops' : EventOps,
    'window_duration_ms' : IDL.Nat64,
    'perf' : EventPerf,
    'window_end_ms' : IDL.Nat64,
    'window_start_ms' : IDL.Nat64,
  });
  const EventReport = IDL.Record({
    'entity_counters' : IDL.Vec(EntitySummary),
    'active_window_start_ms' : IDL.Nat64,
    'requested_window_start_ms' : IDL.Opt(IDL.Nat64),
    'counters' : IDL.Opt(EventCounters),
    'window_filter_matched' : IDL.Bool,
  });
  const RuntimeErrorKind = IDL.Variant({
    'Internal' : IDL.Null,
    'IncompatiblePersistedFormat' : IDL.Null,
    'InvariantViolation' : IDL.Null,
    'Corruption' : IDL.Null,
    'NotFound' : IDL.Null,
    'Unsupported' : IDL.Null,
    'Conflict' : IDL.Null,
  });
  const QueryErrorKind = IDL.Variant({
    'Plan' : IDL.Null,
    'NotFound' : IDL.Null,
    'NotUnique' : IDL.Null,
    'UnorderedPagination' : IDL.Null,
    'InvalidContinuationCursor' : IDL.Null,
    'Intent' : IDL.Null,
    'Validate' : IDL.Null,
  });
  const ErrorKind = IDL.Variant({
    'Runtime' : RuntimeErrorKind,
    'Query' : QueryErrorKind,
  });
  const ErrorOrigin = IDL.Variant({
    'Store' : IDL.Null,
    'Planner' : IDL.Null,
    'Index' : IDL.Null,
    'Cursor' : IDL.Null,
    'Response' : IDL.Null,
    'Recovery' : IDL.Null,
    'Identity' : IDL.Null,
    'Serialize' : IDL.Null,
    'Executor' : IDL.Null,
    'Interface' : IDL.Null,
    'Query' : IDL.Null,
  });
  const Error = IDL.Record({
    'kind' : ErrorKind,
    'origin' : ErrorOrigin,
    'message' : IDL.Text,
  });
  const Result_31 = IDL.Variant({ 'Ok' : EventReport, 'Err' : Error });
  const Result_32 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : Error });
  const IndexState = IDL.Variant({
    'Building' : IDL.Null,
    'Ready' : IDL.Null,
    'Dropping' : IDL.Null,
  });
  const IndexStoreSnapshot = IDL.Record({
    'memory_bytes' : IDL.Nat64,
    'path' : IDL.Text,
    'user_entries' : IDL.Nat64,
    'entries' : IDL.Nat64,
    'state' : IndexState,
    'system_entries' : IDL.Nat64,
  });
  const EntitySnapshot = IDL.Record({
    'memory_bytes' : IDL.Nat64,
    'path' : IDL.Text,
    'entries' : IDL.Nat64,
    'store' : IDL.Text,
  });
  const DataStoreSnapshot = IDL.Record({
    'memory_bytes' : IDL.Nat64,
    'path' : IDL.Text,
    'entries' : IDL.Nat64,
  });
  const StorageReport = IDL.Record({
    'corrupted_entries' : IDL.Nat64,
    'corrupted_keys' : IDL.Nat64,
    'storage_index' : IDL.Vec(IndexStoreSnapshot),
    'entity_storage' : IDL.Vec(EntitySnapshot),
    'storage_data' : IDL.Vec(DataStoreSnapshot),
  });
  const Result_33 = IDL.Variant({ 'Ok' : StorageReport, 'Err' : Error });
  const Result_34 = IDL.Variant({ 'Ok' : BuildPreview, 'Err' : ApiError });
  const ChampionSkillChoiceView = IDL.Record({
    'name' : IDL.Text,
    'rank' : IDL.Nat8,
    'skill_key' : IDL.Text,
    'description' : IDL.Text,
    'enabled' : IDL.Bool,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const ChampionProgressionView = IDL.Record({
    'mana_turn' : IDL.Nat32,
    'mana_max' : IDL.Nat16,
    'mana' : IDL.Nat16,
    'level' : IDL.Nat16,
    'experience' : IDL.Nat64,
    'learned_spell_slugs' : IDL.Vec(IDL.Text),
    'champion_id' : IDL.Text,
    'skill_keys' : IDL.Vec(IDL.Text),
    'skill_points' : IDL.Nat16,
    'level_up_choices' : IDL.Vec(ChampionSkillChoiceView),
  });
  const Result_35 = IDL.Variant({
    'Ok' : ChampionProgressionView,
    'Err' : ApiError,
  });
  const DwellingRecruitPreview = IDL.Record({
    'target_champion_id' : IDL.Text,
    'object_id' : IDL.Text,
    'total_cost' : ResourceBalances,
    'allowed' : IDL.Bool,
    'unit_slug' : IDL.Text,
    'available' : IDL.Nat32,
    'quantity' : IDL.Nat32,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const Result_36 = IDL.Variant({
    'Ok' : DwellingRecruitPreview,
    'Err' : ApiError,
  });
  const ChampionHirePreview = IDL.Record({
    'town_id' : IDL.Text,
    'offer_key' : IDL.Text,
    'cost' : ResourceBalances,
    'allowed' : IDL.Bool,
    'candidate_name' : IDL.Text,
    'disabled_reason' : IDL.Opt(IDL.Text),
    'champion_class_slug' : IDL.Text,
  });
  const Result_37 = IDL.Variant({
    'Ok' : ChampionHirePreview,
    'Err' : ApiError,
  });
  const MarketTradePreview = IDL.Record({
    'rate_key' : IDL.Text,
    'to_resource' : IDL.Text,
    'from_resource' : IDL.Text,
    'allowed' : IDL.Bool,
    'amount_out' : IDL.Nat64,
    'amount_in' : IDL.Nat64,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const Result_38 = IDL.Variant({
    'Ok' : MarketTradePreview,
    'Err' : ApiError,
  });
  const Result_39 = IDL.Variant({ 'Ok' : MovementPreview, 'Err' : ApiError });
  const QuestProgressView = IDL.Record({
    'status' : IDL.Text,
    'title' : IDL.Text,
    'accepted_turn' : IDL.Nat32,
    'reward_claimed' : IDL.Bool,
    'reward_gold' : IDL.Opt(IDL.Nat32),
    'quest_key' : IDL.Text,
    'objective_key' : IDL.Text,
    'participant_id' : IDL.Text,
    'claimed_turn' : IDL.Nat32,
    'required_value' : IDL.Nat32,
    'redacted' : IDL.Bool,
    'progress_value' : IDL.Nat32,
  });
  const QuestPreview = IDL.Record({
    'can_accept' : IDL.Bool,
    'quest' : QuestProgressView,
    'can_claim' : IDL.Bool,
    'disabled_reason' : IDL.Opt(IDL.Text),
  });
  const Result_40 = IDL.Variant({ 'Ok' : QuestPreview, 'Err' : ApiError });
  const RecruitTarget = IDL.Variant({
    'TownGarrison' : IDL.Record({ 'slot_index' : IDL.Opt(IDL.Nat8) }),
    'Champion' : IDL.Record({
      'champion_id' : IDL.Text,
      'slot_index' : IDL.Opt(IDL.Nat8),
    }),
  });
  const Result_41 = IDL.Variant({ 'Ok' : RecruitPreview, 'Err' : ApiError });
  const Result_42 = IDL.Variant({
    'Ok' : DiagnosticProjectionFlushView,
    'Err' : ApiError,
  });
  const Result_43 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : ApiError });
  const Result_44 = IDL.Variant({ 'Ok' : IDL.Nat32, 'Err' : ApiError });
  const BattleActionInput = IDL.Record({
    'destination' : IDL.Opt(BattleCoord),
    'action' : IDL.Text,
    'battle_id' : IDL.Text,
    'battle_stack_id' : IDL.Text,
    'ability_key' : IDL.Opt(IDL.Text),
    'target_stack_id' : IDL.Opt(IDL.Text),
  });
  return IDL.Service({
    'accept_quest' : IDL.Func([IDL.Text, IDL.Text, IDL.Text], [Result], []),
    'cast_adventure_spell' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'claim_quest_reward' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'create_session' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Nat64, IDL.Text],
        [Result_1],
        [],
      ),
    'end_battle_turn' : IDL.Func([IDL.Text, IDL.Text, IDL.Text], [Result], []),
    'end_turn' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'force_diagnostic_system_job_running' : IDL.Func(
        [IDL.Text, IDL.Nat64],
        [Result_2],
        [],
      ),
    'get_battle_state' : IDL.Func([IDL.Text, IDL.Text], [Result_3], ['query']),
    'get_canister_endpoint_inventory' : IDL.Func(
        [],
        [IDL.Vec(CanisterEndpointView)],
        ['query'],
      ),
    'get_champion_view' : IDL.Func([IDL.Text, IDL.Text], [Result_4], ['query']),
    'get_command_status' : IDL.Func(
        [IDL.Text, IDL.Text],
        [Result_5],
        ['query'],
      ),
    'get_command_status_by_nonce' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_5],
        ['query'],
      ),
    'get_content_manifest' : IDL.Func(
        [IDL.Text, IDL.Nat32],
        [Result_6],
        ['query'],
      ),
    'get_diagnostic_projection_snapshot' : IDL.Func([], [Result_7], ['query']),
    'get_diagnostic_storage_snapshot' : IDL.Func(
        [IDL.Vec(IDL.Text)],
        [Result_8],
        ['query'],
      ),
    'get_diagnostic_system_jobs' : IDL.Func(
        [IDL.Opt(IDL.Text), IDL.Opt(IDL.Text), IDL.Nat32, IDL.Opt(IDL.Text)],
        [Result_9],
        ['query'],
      ),
    'get_dwelling_pool' : IDL.Func(
        [IDL.Text, IDL.Text],
        [Result_10],
        ['query'],
      ),
    'get_events_after' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Nat64, IDL.Nat32],
        [Result_11],
        ['query'],
      ),
    'get_game_view' : IDL.Func(
        [IDL.Text, GameViewRequest],
        [Result_12],
        ['query'],
      ),
    'get_match_history' : IDL.Func(
        [IDL.Nat32, IDL.Nat32],
        [Result_13],
        ['query'],
      ),
    'get_my_champions' : IDL.Func([IDL.Text], [Result_14], ['query']),
    'get_my_participant' : IDL.Func([IDL.Text], [Result_15], ['query']),
    'get_my_player' : IDL.Func([], [Result_16], ['query']),
    'get_naval_routes' : IDL.Func([IDL.Text], [Result_17], ['query']),
    'get_object_view' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_18],
        ['query'],
      ),
    'get_objective_progress' : IDL.Func([IDL.Text], [Result_19], ['query']),
    'get_procedural_map_state' : IDL.Func([IDL.Text], [Result_20], ['query']),
    'get_scenario_rules' : IDL.Func([IDL.Text], [Result_21], ['query']),
    'get_session' : IDL.Func([IDL.Text], [Result_22], ['query']),
    'get_setup_progress' : IDL.Func([IDL.Text], [Result_23], ['query']),
    'get_siege_rules' : IDL.Func([IDL.Text], [Result_24], ['query']),
    'get_skirmish_settings' : IDL.Func([IDL.Text], [Result_25], ['query']),
    'get_tavern_offers' : IDL.Func(
        [IDL.Text, IDL.Text],
        [Result_26],
        ['query'],
      ),
    'get_town_view' : IDL.Func([IDL.Text, IDL.Text], [Result_27], ['query']),
    'get_visible_map_chunks' : IDL.Func(
        [IDL.Text, Viewport, IDL.Opt(IDL.Nat32), IDL.Nat32],
        [Result_28],
        ['query'],
      ),
    'get_visible_objects' : IDL.Func(
        [IDL.Text, Viewport, IDL.Opt(IDL.Nat32), IDL.Nat32],
        [Result_29],
        ['query'],
      ),
    'get_world_events' : IDL.Func([IDL.Text], [Result_30], ['query']),
    'hire_tavern_champion' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'icydb_metrics' : IDL.Func([IDL.Opt(IDL.Nat64)], [Result_31], ['query']),
    'icydb_metrics_reset' : IDL.Func([], [Result_32], []),
    'icydb_snapshot' : IDL.Func([], [Result_33], ['query']),
    'join_session' : IDL.Func([IDL.Text, IDL.Text, IDL.Text], [Result_1], []),
    'learn_champion_spell' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'mark_ready' : IDL.Func([IDL.Text, IDL.Text], [Result_1], []),
    'preview_build_town_structure' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_34],
        ['query'],
      ),
    'preview_champion_progression' : IDL.Func(
        [IDL.Text, IDL.Text],
        [Result_35],
        ['query'],
      ),
    'preview_dwelling_recruit' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat32, IDL.Text],
        [Result_36],
        ['query'],
      ),
    'preview_hire_champion' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text],
        [Result_37],
        ['query'],
      ),
    'preview_market_trade' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat64],
        [Result_38],
        ['query'],
      ),
    'preview_move_path' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Vec(MoveCoord)],
        [Result_39],
        ['query'],
      ),
    'preview_quest' : IDL.Func([IDL.Text, IDL.Text], [Result_40], ['query']),
    'preview_recruit_units' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat32, RecruitTarget],
        [Result_41],
        ['query'],
      ),
    'register_player' : IDL.Func(
        [IDL.Opt(IDL.Text), IDL.Opt(IDL.Text), IDL.Text],
        [Result_1],
        [],
      ),
    'run_diagnostic_battle_projection_flush' : IDL.Func([], [Result_42], []),
    'run_diagnostic_flush_barrier' : IDL.Func([IDL.Text], [Result_43], []),
    'run_diagnostic_projection_flush' : IDL.Func([], [Result_42], []),
    'run_diagnostic_system_job' : IDL.Func([IDL.Text], [Result_44], []),
    'run_diagnostic_system_jobs' : IDL.Func([IDL.Nat32], [Result_44], []),
    'select_champion_level_up' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'start_session' : IDL.Func([IDL.Text, IDL.Text], [Result_1], []),
    'submit_battle_action' : IDL.Func(
        [IDL.Text, BattleActionInput, IDL.Text],
        [Result],
        [],
      ),
    'submit_build_town_structure' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'submit_dwelling_recruit' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat32, IDL.Text, IDL.Text],
        [Result],
        [],
      ),
    'submit_market_trade' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat64, IDL.Text],
        [Result],
        [],
      ),
    'submit_move_intent' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Vec(MoveCoord), IDL.Text],
        [Result],
        [],
      ),
    'submit_recruit_units' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Text, IDL.Nat32, RecruitTarget, IDL.Text],
        [Result],
        [],
      ),
    'sync_advanced_victory' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'sync_battle' : IDL.Func([IDL.Text, IDL.Text, IDL.Text], [Result], []),
    'sync_objectives' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'sync_session_turn' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'sync_world_events' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'sync_world_generation' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
  });
};
export const init = ({ IDL }) => { return []; };
