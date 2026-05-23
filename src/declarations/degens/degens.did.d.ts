import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

export interface ActionAffordance {
  'action' : string,
  'target_id' : [] | [string],
  'enabled' : boolean,
  'disabled_reason' : [] | [string],
}
export interface AdvancedScenarioReceipt {
  'event_key' : [] | [string],
  'rule_key' : [] | [string],
  'action' : string,
  'command_id' : string,
  'resources_after' : [] | [ResourceBalances],
  'reward_gold' : number,
  'state' : string,
  'quest_key' : [] | [string],
  'current_turn' : number,
  'objective_key' : [] | [string],
}
export interface ApiError {
  'code' : string,
  'details_json' : [] | [string],
  'message' : string,
  'retryable' : boolean,
}
export interface ApiEventPage {
  'page_info' : EventPageInfo,
  'events' : Array<ApiEventView>,
}
export interface ApiEventView {
  'event_key' : string,
  'event_seq' : bigint,
  'session_id' : string,
  'subject_kind' : [] | [string],
  'turn_number' : number,
  'audience_key' : string,
  'subject_id_text' : [] | [string],
  'event_type' : string,
  'payload' : [] | [string],
  'redacted' : boolean,
}
export interface ApiTownView {
  'town' : TownRecord,
  'garrison_stacks' : Array<ArmyStackRecord>,
  'recruit_pools' : Array<TownRecruitPoolRecord>,
  'buildings' : Array<TownBuildingRecord>,
}
export interface ArmyStackRecord {
  'status' : string,
  'session_id' : string,
  'front_hp' : number,
  'unit_slug' : string,
  'owner_id' : string,
  'stack_id' : string,
  'quantity' : number,
  'slot_index' : number,
  'last_command_id' : [] | [string],
  'owner_kind' : string,
}
export interface ArtifactContent {
  'id' : string,
  'ruleset_id' : string,
  'name' : string,
  'slot' : string,
  'slug' : string,
  'description' : [] | [string],
  'icon_key' : [] | [string],
  'rarity' : string,
  'effect_key' : string,
}
export interface ArtifactView {
  'artifact_id' : string,
  'artifact_def_id' : string,
  'slot' : string,
  'state' : string,
}
export interface BattleActionInput {
  'destination' : [] | [BattleCoord],
  'action' : string,
  'battle_id' : string,
  'battle_stack_id' : string,
  'ability_key' : [] | [string],
  'target_stack_id' : [] | [string],
}
export interface BattleActionReceipt {
  'event_seq' : [] | [bigint],
  'status' : string,
  'command_id' : string,
  'active_stack_id' : [] | [string],
  'current_round' : number,
}
export interface BattleCoord { 'x' : number, 'y' : number }
export interface BattleEventView {
  'event_key' : string,
  'event_seq' : bigint,
  'subject_id_text' : string,
  'event_type' : string,
  'payload' : string,
}
export interface BattleGridView { 'height' : number, 'width' : number }
export interface BattleMoraleLuckPolicy {
  'morale_disabled_reason' : [] | [string],
  'morale_enabled' : boolean,
  'luck_enabled' : boolean,
  'luck_disabled_reason' : [] | [string],
}
export interface BattleObstacleView {
  'hp' : number,
  'height' : number,
  'battle_x' : number,
  'battle_y' : number,
  'obstacle_type' : string,
  'battle_obstacle_id' : string,
  'state' : string,
  'width' : number,
}
export interface BattleStackView {
  'status' : string,
  'shots_remaining' : number,
  'owner_participant_id' : [] | [string],
  'battle_x' : number,
  'battle_y' : number,
  'flying' : boolean,
  'side' : string,
  'battle_stack_id' : string,
  'champion_guard' : number,
  'front_hp' : number,
  'defended_round' : number,
  'acted_round' : number,
  'speed' : number,
  'waited_round' : number,
  'defense' : number,
  'quantity' : number,
  'ranged' : boolean,
  'damage_max' : number,
  'damage_min' : number,
  'champion_might' : number,
  'max_hp' : number,
  'unit_id' : string,
  'status_keys' : Array<string>,
  'attack' : number,
  'initiative' : number,
}
export interface BattleStartDraft {
  'x' : number,
  'y' : number,
  'attacker_champion_id' : string,
  'defender_id_text' : string,
  'defender_kind' : string,
  'battle_type' : string,
  'battle_key' : string,
}
export interface BattleSummary {
  'battle_id' : string,
  'active_participant_id' : [] | [string],
  'active_stack_id' : [] | [string],
  'state' : string,
  'current_round' : number,
  'battle_type' : string,
}
export interface BattleSyncOutcome {
  'battle_id' : string,
  'battle_sync_incomplete' : boolean,
  'active_stack_id' : [] | [string],
  'timeout_actions_applied' : number,
  'recovered_commands' : number,
}
export interface BattleView {
  'stacks' : Array<BattleStackView>,
  'legal_actions_for_caller' : Array<LegalBattleAction>,
  'battle_id' : string,
  'grid' : BattleGridView,
  'active_participant_id' : [] | [string],
  'action_deadline_at' : [] | [bigint],
  'active_stack_id' : [] | [string],
  'state' : string,
  'obstacles' : Array<BattleObstacleView>,
  'initiative_order' : Array<string>,
  'events' : Array<BattleEventView>,
  'current_round' : number,
  'battle_type' : string,
  'next_event_seq' : bigint,
  'remaining_ms' : [] | [bigint],
  'morale_luck_policy' : BattleMoraleLuckPolicy,
}
export interface BuildPreview {
  'town_id' : string,
  'cost' : ResourceBalances,
  'allowed' : boolean,
  'building_slug' : string,
  'disabled_reason' : [] | [string],
}
export interface BuildingContent {
  'id' : string,
  'ruleset_id' : string,
  'cost' : ResourceCost,
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'faction_slug' : [] | [string],
  'icon_key' : [] | [string],
  'requires_building_slugs' : Array<string>,
  'building_type' : string,
  'effect_key' : [] | [string],
  'unlocks_unit_slug' : [] | [string],
}
export interface CanisterEndpointView {
  'kind' : EndpointKind,
  'name' : string,
  'group' : string,
  'fixture_mapping' : string,
}
export interface ChampionArmyStackRecord {
  'status' : string,
  'session_id' : string,
  'front_hp' : number,
  'unit_slug' : string,
  'stack_id' : string,
  'champion_id' : string,
  'quantity' : number,
  'slot_index' : number,
  'last_command_id' : [] | [string],
}
export interface ChampionClassContent {
  'id' : string,
  'ruleset_id' : string,
  'portrait_key' : [] | [string],
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'faction_slug' : [] | [string],
  'base_movement' : number,
  'base_vision' : number,
}
export interface ChampionHirePreview {
  'town_id' : string,
  'offer_key' : string,
  'cost' : ResourceBalances,
  'allowed' : boolean,
  'candidate_name' : string,
  'disabled_reason' : [] | [string],
  'champion_class_slug' : string,
}
export interface ChampionMagicReceipt {
  'action' : string,
  'mana_after' : number,
  'command_id' : string,
  'skill_key' : [] | [string],
  'champion_id' : string,
  'status_keys' : Array<string>,
  'movement_remaining_after' : number,
  'spell_slug' : [] | [string],
}
export interface ChampionProgressionView {
  'mana_turn' : number,
  'mana_max' : number,
  'mana' : number,
  'level' : number,
  'experience' : bigint,
  'learned_spell_slugs' : Array<string>,
  'champion_id' : string,
  'skill_keys' : Array<string>,
  'skill_points' : number,
  'level_up_choices' : Array<ChampionSkillChoiceView>,
}
export interface ChampionSkillChoiceView {
  'name' : string,
  'rank' : number,
  'skill_key' : string,
  'description' : string,
  'enabled' : boolean,
  'disabled_reason' : [] | [string],
}
export interface ChampionView {
  'x' : number,
  'y' : number,
  'status' : string,
  'vision_radius' : number,
  'owner_participant_id' : string,
  'mana_max' : number,
  'artifacts' : Array<ArtifactView>,
  'mana' : number,
  'name' : [] | [string],
  'spell_slugs' : Array<string>,
  'movement_max' : number,
  'champion_id' : string,
  'skill_keys' : Array<string>,
  'army_stacks' : Array<ChampionArmyStackRecord>,
  'skill_points' : number,
  'class_key' : string,
  'strength_label' : string,
  'class_def_id' : string,
  'redacted' : boolean,
  'effective_movement' : number,
}
export interface ChangedSubject {
  'subject_kind' : string,
  'operation' : string,
  'subject_id_text' : string,
}
export type CommandPhase = { 'EffectsApplied' : null } |
  { 'Failed' : null } |
  { 'Complete' : null } |
  { 'Recovered' : null } |
  { 'Created' : null } |
  { 'Validated' : null } |
  { 'EventsApplied' : null } |
  { 'Applying' : null };
export interface CommandResponse {
  'status' : CommandStatus,
  'result' : CommandResult,
  'changed_subjects' : Array<ChangedSubject>,
  'command_id' : string,
  'error' : [] | [ApiError],
  'command_type' : string,
  'events' : Array<ApiEventView>,
  'effective_turn' : number,
  'actor_participant_id' : [] | [string],
  'actor_principal' : Principal,
  'phase' : CommandPhase,
  'retryable' : boolean,
  'client_nonce' : string,
  'durable_turn' : number,
  'payload_hash' : string,
}
export type CommandResult = { 'ChampionMagic' : ChampionMagicReceipt } |
  { 'RecruitPreview' : RecruitPreview } |
  { 'BuildPreview' : BuildPreview } |
  { 'None' : null } |
  { 'MovementSync' : MovementSyncOutcome } |
  { 'BattleAction' : BattleActionReceipt } |
  { 'WorldGeneration' : WorldGenerationReceipt } |
  { 'MovementPreview' : MovementPreview } |
  { 'BattleSync' : BattleSyncOutcome } |
  { 'StrategicReceipt' : StrategicCommandReceipt } |
  { 'ExpandedEconomy' : ExpandedEconomyReceipt } |
  { 'AdvancedScenario' : AdvancedScenarioReceipt };
export type CommandStatus = { 'Applied' : null } |
  { 'Failed' : null } |
  { 'Superseded' : null } |
  { 'AppliedNoop' : null } |
  { 'Cancelled' : null } |
  { 'Pending' : null } |
  { 'Applying' : null };
export interface CommandStatusView {
  'status' : CommandStatus,
  'result_json' : [] | [string],
  'command_id' : string,
  'error_message' : [] | [string],
  'phase' : CommandPhase,
  'error_code' : [] | [string],
  'retryable' : boolean,
}
export interface ContentManifest {
  'asset_keys' : Array<string>,
  'terrain' : Array<TerrainContent>,
  'factions' : Array<FactionContent>,
  'artifacts' : Array<ArtifactContent>,
  'champion_classes' : Array<ChampionClassContent>,
  'units' : Array<UnitContent>,
  'spells' : Array<SpellContent>,
  'ruleset' : RulesetContent,
  'map_objects' : Array<MapObjectContent>,
  'buildings' : Array<BuildingContent>,
}
export interface ContentManifestResponse { 'manifest' : ContentManifest }
export interface DamagePreview {
  'max_damage' : number,
  'min_damage' : number,
  'estimated_kills_max' : number,
  'estimated_kills_min' : number,
  'target_stack_id' : string,
}
export interface DataStoreSnapshot {
  'memory_bytes' : bigint,
  'path' : string,
  'entries' : bigint,
}
export interface DiagnosticProjectionFlushView {
  'queue_len_after' : bigint,
  'entries_processed' : bigint,
  'stable_pages_delta' : bigint,
  'queue_len_before' : bigint,
  'flushed_at_ms' : bigint,
  'flush_truncated' : boolean,
  'flush_instructions' : bigint,
  'rows_flushed' : bigint,
}
export interface DiagnosticProjectionKernelView {
  'dirty_queue_len' : bigint,
  'session_id' : string,
  'oldest_dirty_age_ms' : [] | [bigint],
  'lag_ms' : bigint,
  'lag_generations' : bigint,
  'flushed_at_ms' : bigint,
  'kernel_id' : string,
  'pending_entries' : bigint,
  'kernel_generation' : bigint,
  'turn_number' : number,
  'flushed_generation' : bigint,
}
export interface DiagnosticProjectionSnapshot {
  'oldest_dirty_age_ms' : [] | [bigint],
  'total_dirty_queue_len' : bigint,
  'last_flush' : [] | [DiagnosticProjectionFlushView],
  'kernels' : Array<DiagnosticProjectionKernelView>,
}
export interface DiagnosticRowCount { 'entity' : string, 'count' : number }
export interface DiagnosticStorageSnapshot {
  'stable_memory_pages' : bigint,
  'total_rows' : number,
  'row_counts' : Array<DiagnosticRowCount>,
}
export interface DiagnosticSystemJobPage {
  'jobs' : Array<DiagnosticSystemJobView>,
  'limit' : number,
  'next_cursor' : [] | [string],
}
export interface DiagnosticSystemJobView {
  'last_error' : [] | [string],
  'status' : string,
  'due_at_ms' : bigint,
  'battle_id' : [] | [string],
  'session_id' : string,
  'command_id' : [] | [string],
  'job_key' : string,
  'lease_expires_at_ms' : [] | [bigint],
  'attempt_count' : number,
  'lease_owner' : [] | [string],
  'turn_number' : [] | [number],
  'job_kind' : string,
  'cursor_json' : [] | [string],
}
export interface DwellingPoolView {
  'owner_participant_id' : [] | [string],
  'direct_recruit' : boolean,
  'growth_per_week' : number,
  'object_id' : string,
  'unit_slug' : string,
  'available' : number,
  'last_growth_week' : number,
}
export interface DwellingRecruitPreview {
  'target_champion_id' : string,
  'object_id' : string,
  'total_cost' : ResourceBalances,
  'allowed' : boolean,
  'unit_slug' : string,
  'available' : number,
  'quantity' : number,
  'disabled_reason' : [] | [string],
}
export type EndpointKind = { 'Update' : null } |
  { 'Query' : null };
export interface EntitySnapshot {
  'memory_bytes' : bigint,
  'path' : string,
  'entries' : bigint,
  'store' : string,
}
export interface EntitySummary {
  'schema_reconcile_rejected_field_slot' : bigint,
  'plan_index_multi_lookup' : bigint,
  'sql_compile_reject_parse' : bigint,
  'schema_transition_rejected_row_layout' : bigint,
  'reverse_index_removes' : bigint,
  'cache_shared_query_plan_misses' : bigint,
  'schema_reconcile_rejected_other' : bigint,
  'cache_sql_compiled_command_miss_method_version' : bigint,
  'load_candidate_rows_scanned' : bigint,
  'schema_store_snapshots' : bigint,
  'relation_reverse_lookups' : bigint,
  'prepared_shape_generated_fallback' : bigint,
  'load_result_rows_emitted' : bigint,
  'index_removes' : bigint,
  'schema_transition_exact_match' : bigint,
  'plan_grouped_ordered_materialized' : bigint,
  'plan_by_keys' : bigint,
  'plan_keys' : bigint,
  'write_rows_touched' : bigint,
  'cache_shared_query_plan_hits' : bigint,
  'rows_updated' : bigint,
  'plan_full_scan' : bigint,
  'save_insert_calls' : bigint,
  'exec_aborted' : bigint,
  'cache_sql_compiled_command_miss_distinct_key' : bigint,
  'cache_shared_query_plan_miss_visibility' : bigint,
  'cache_sql_compiled_command_misses' : bigint,
  'sql_write_error_delete' : bigint,
  'plan_choice_planner_full_scan_fallback' : bigint,
  'rows_filtered' : bigint,
  'cache_sql_compiled_command_miss_schema_fingerprint' : bigint,
  'load_candidate_rows_filtered' : bigint,
  'rows_replaced' : bigint,
  'schema_reconcile_rejected_row_layout' : bigint,
  'plan_choice_required_order_primary_key_range_preferred' : bigint,
  'sql_write_error_not_found' : bigint,
  'cache_sql_compiled_command_hits' : bigint,
  'cache_shared_query_plan_miss_method_version' : bigint,
  'rows_emitted' : bigint,
  'sql_update_calls' : bigint,
  'sql_write_error_update' : bigint,
  'rows_loaded' : bigint,
  'path' : string,
  'sql_write_error_insert_select' : bigint,
  'plan_union' : bigint,
  'schema_transition_rejected_field_contract' : bigint,
  'save_calls' : bigint,
  'sql_write_error_incompatible_persisted_format' : bigint,
  'schema_store_latest_snapshot_bytes' : bigint,
  'schema_transition_rejected_snapshot' : bigint,
  'accepted_schema_nested_leaf_facts' : bigint,
  'schema_store_encoded_bytes' : bigint,
  'sql_write_error_insert' : bigint,
  'plan_index' : bigint,
  'schema_reconcile_store_write_error' : bigint,
  'cache_shared_query_plan_miss_cold' : bigint,
  'plan_choice_planner_composite_non_index' : bigint,
  'delete_calls' : bigint,
  'sql_insert_select_calls' : bigint,
  'relation_delete_blocks' : bigint,
  'unique_violations' : bigint,
  'sql_write_returning_rows' : bigint,
  'accepted_schema_fields' : bigint,
  'plan_key_range' : bigint,
  'cache_sql_compiled_command_inserts' : bigint,
  'schema_reconcile_latest_snapshot_corrupt' : bigint,
  'schema_transition_checks' : bigint,
  'plan_range' : bigint,
  'plan_by_key' : bigint,
  'cache_shared_query_plan_miss_distinct_key' : bigint,
  'plan_choice_limit_zero_window' : bigint,
  'cache_sql_compiled_command_miss_surface' : bigint,
  'plan_grouped_hash_materialized' : bigint,
  'schema_reconcile_exact_match' : bigint,
  'plan_choice_constant_false_predicate' : bigint,
  'exec_error_unsupported' : bigint,
  'sql_write_matched_rows' : bigint,
  'rows_deleted' : bigint,
  'reverse_index_inserts' : bigint,
  'plan_choice_intent_key_access_override' : bigint,
  'write_relation_checks' : bigint,
  'exec_error_invariant_violation' : bigint,
  'sql_write_error_invariant_violation' : bigint,
  'exec_error_corruption' : bigint,
  'exec_error_not_found' : bigint,
  'schema_transition_append_only_nullable_fields' : bigint,
  'cache_shared_query_plan_inserts' : bigint,
  'sql_compile_rejects' : bigint,
  'exec_error_conflict' : bigint,
  'plan_explicit_full_scan' : bigint,
  'index_inserts' : bigint,
  'exec_error_internal' : bigint,
  'exec_error_incompatible_persisted_format' : bigint,
  'sql_delete_calls' : bigint,
  'sql_write_mutated_rows' : bigint,
  'save_update_calls' : bigint,
  'sql_compile_reject_cache_key' : bigint,
  'schema_transition_rejected_schema_version' : bigint,
  'plan_choice_planner_primary_key_range' : bigint,
  'exec_success' : bigint,
  'rows_aggregated' : bigint,
  'sql_compile_reject_semantic' : bigint,
  'load_calls' : bigint,
  'plan_choice_full_scan_access' : bigint,
  'sql_insert_calls' : bigint,
  'schema_reconcile_rejected_schema_version' : bigint,
  'write_index_entries_changed' : bigint,
  'plan_choice_non_index_access' : bigint,
  'schema_transition_rejected_field_slot' : bigint,
  'cache_sql_compiled_command_miss_cold' : bigint,
  'rows_saved' : bigint,
  'plan_choice_conflicting_primary_key_children_access_preferred' : bigint,
  'prepared_shape_already_finalized' : bigint,
  'plan_intersection' : bigint,
  'schema_transition_rejected_entity_identity' : bigint,
  'non_atomic_partial_commits' : bigint,
  'non_atomic_partial_rows_committed' : bigint,
  'sql_write_error_conflict' : bigint,
  'plan_choice_planner_primary_key_lookup' : bigint,
  'rows_inserted' : bigint,
  'sql_write_error_internal' : bigint,
  'save_replace_calls' : bigint,
  'plan_index_prefix' : bigint,
  'rows_scanned' : bigint,
  'write_reverse_index_entries_changed' : bigint,
  'plan_choice_singleton_primary_key_child_access_preferred' : bigint,
  'cache_shared_query_plan_miss_schema_fingerprint' : bigint,
  'sql_write_error_unsupported' : bigint,
  'plan_index_range' : bigint,
  'schema_reconcile_first_create' : bigint,
  'schema_reconcile_checks' : bigint,
  'sql_write_error_corruption' : bigint,
  'plan_choice_planner_key_set_access' : bigint,
  'plan_choice_empty_child_access_preferred' : bigint,
}
export interface Error {
  'kind' : ErrorKind,
  'origin' : ErrorOrigin,
  'message' : string,
}
export type ErrorKind = {
    /**
     * Runtime failure.
     */
    'Runtime' : RuntimeErrorKind
  } |
  { 'Query' : QueryErrorKind };
export type ErrorOrigin = { 'Store' : null } |
  { 'Planner' : null } |
  { 'Index' : null } |
  { 'Cursor' : null } |
  { 'Response' : null } |
  { 'Recovery' : null } |
  { 'Identity' : null } |
  { 'Serialize' : null } |
  { 'Executor' : null } |
  { 'Interface' : null } |
  { 'Query' : null };
export interface EventCounters {
  'ops' : EventOps,
  'window_duration_ms' : bigint,
  'perf' : EventPerf,
  'window_end_ms' : bigint,
  'window_start_ms' : bigint,
}
export interface EventOps {
  'schema_reconcile_rejected_field_slot' : bigint,
  'plan_index_multi_lookup' : bigint,
  'sql_compile_reject_parse' : bigint,
  'schema_transition_rejected_row_layout' : bigint,
  'reverse_index_removes' : bigint,
  'cache_shared_query_plan_misses' : bigint,
  'schema_reconcile_rejected_other' : bigint,
  'cache_sql_compiled_command_miss_method_version' : bigint,
  'load_candidate_rows_scanned' : bigint,
  'schema_store_snapshots' : bigint,
  'relation_reverse_lookups' : bigint,
  'prepared_shape_generated_fallback' : bigint,
  'load_result_rows_emitted' : bigint,
  'index_removes' : bigint,
  'schema_transition_exact_match' : bigint,
  'plan_grouped_ordered_materialized' : bigint,
  'plan_by_keys' : bigint,
  'plan_keys' : bigint,
  'write_rows_touched' : bigint,
  'cache_shared_query_plan_hits' : bigint,
  'rows_updated' : bigint,
  'plan_full_scan' : bigint,
  'save_insert_calls' : bigint,
  'exec_aborted' : bigint,
  'cache_sql_compiled_command_miss_distinct_key' : bigint,
  'cache_shared_query_plan_miss_visibility' : bigint,
  'cache_sql_compiled_command_misses' : bigint,
  'sql_write_error_delete' : bigint,
  'plan_choice_planner_full_scan_fallback' : bigint,
  'rows_filtered' : bigint,
  'cache_sql_compiled_command_miss_schema_fingerprint' : bigint,
  'load_candidate_rows_filtered' : bigint,
  'rows_replaced' : bigint,
  'schema_reconcile_rejected_row_layout' : bigint,
  'plan_choice_required_order_primary_key_range_preferred' : bigint,
  'sql_write_error_not_found' : bigint,
  'cache_sql_compiled_command_hits' : bigint,
  'cache_shared_query_plan_miss_method_version' : bigint,
  'rows_emitted' : bigint,
  'sql_update_calls' : bigint,
  'sql_write_error_update' : bigint,
  'rows_loaded' : bigint,
  'sql_write_error_insert_select' : bigint,
  'plan_union' : bigint,
  'schema_transition_rejected_field_contract' : bigint,
  'save_calls' : bigint,
  'sql_write_error_incompatible_persisted_format' : bigint,
  'schema_store_latest_snapshot_bytes' : bigint,
  'schema_transition_rejected_snapshot' : bigint,
  'accepted_schema_nested_leaf_facts' : bigint,
  'schema_store_encoded_bytes' : bigint,
  'sql_write_error_insert' : bigint,
  'plan_index' : bigint,
  'schema_reconcile_store_write_error' : bigint,
  'cache_shared_query_plan_miss_cold' : bigint,
  'plan_choice_planner_composite_non_index' : bigint,
  'delete_calls' : bigint,
  'sql_insert_select_calls' : bigint,
  'relation_delete_blocks' : bigint,
  'unique_violations' : bigint,
  'cache_sql_compiled_command_entries' : bigint,
  'sql_write_returning_rows' : bigint,
  'accepted_schema_fields' : bigint,
  'plan_key_range' : bigint,
  'cache_sql_compiled_command_inserts' : bigint,
  'schema_reconcile_latest_snapshot_corrupt' : bigint,
  'schema_transition_checks' : bigint,
  'plan_range' : bigint,
  'plan_by_key' : bigint,
  'cache_shared_query_plan_miss_distinct_key' : bigint,
  'plan_choice_limit_zero_window' : bigint,
  'cache_sql_compiled_command_miss_surface' : bigint,
  'plan_grouped_hash_materialized' : bigint,
  'schema_reconcile_exact_match' : bigint,
  'plan_choice_constant_false_predicate' : bigint,
  'exec_error_unsupported' : bigint,
  'sql_write_matched_rows' : bigint,
  'rows_deleted' : bigint,
  'reverse_index_inserts' : bigint,
  'cache_shared_query_plan_entries' : bigint,
  'plan_choice_intent_key_access_override' : bigint,
  'write_relation_checks' : bigint,
  'exec_error_invariant_violation' : bigint,
  'sql_write_error_invariant_violation' : bigint,
  'exec_error_corruption' : bigint,
  'exec_error_not_found' : bigint,
  'schema_transition_append_only_nullable_fields' : bigint,
  'cache_shared_query_plan_inserts' : bigint,
  'sql_compile_rejects' : bigint,
  'exec_error_conflict' : bigint,
  'plan_explicit_full_scan' : bigint,
  'index_inserts' : bigint,
  'exec_error_internal' : bigint,
  'exec_error_incompatible_persisted_format' : bigint,
  'sql_delete_calls' : bigint,
  'sql_write_mutated_rows' : bigint,
  'save_update_calls' : bigint,
  'sql_compile_reject_cache_key' : bigint,
  'schema_transition_rejected_schema_version' : bigint,
  'plan_choice_planner_primary_key_range' : bigint,
  'exec_success' : bigint,
  'rows_aggregated' : bigint,
  'sql_compile_reject_semantic' : bigint,
  'load_calls' : bigint,
  'plan_choice_full_scan_access' : bigint,
  'sql_insert_calls' : bigint,
  'schema_reconcile_rejected_schema_version' : bigint,
  'write_index_entries_changed' : bigint,
  'plan_choice_non_index_access' : bigint,
  'schema_transition_rejected_field_slot' : bigint,
  'cache_sql_compiled_command_miss_cold' : bigint,
  'rows_saved' : bigint,
  'plan_choice_conflicting_primary_key_children_access_preferred' : bigint,
  'prepared_shape_already_finalized' : bigint,
  'plan_intersection' : bigint,
  'schema_transition_rejected_entity_identity' : bigint,
  'non_atomic_partial_commits' : bigint,
  'non_atomic_partial_rows_committed' : bigint,
  'sql_write_error_conflict' : bigint,
  'plan_choice_planner_primary_key_lookup' : bigint,
  'rows_inserted' : bigint,
  'sql_write_error_internal' : bigint,
  'save_replace_calls' : bigint,
  'plan_index_prefix' : bigint,
  'rows_scanned' : bigint,
  'write_reverse_index_entries_changed' : bigint,
  'plan_choice_singleton_primary_key_child_access_preferred' : bigint,
  'cache_shared_query_plan_miss_schema_fingerprint' : bigint,
  'sql_write_error_unsupported' : bigint,
  'plan_index_range' : bigint,
  'schema_reconcile_first_create' : bigint,
  'schema_reconcile_checks' : bigint,
  'sql_write_error_corruption' : bigint,
  'plan_choice_planner_key_set_access' : bigint,
  'plan_choice_empty_child_access_preferred' : bigint,
}
export interface EventPageInfo {
  'limit' : number,
  'next_event_seq' : [] | [bigint],
  'has_more' : boolean,
}
export interface EventPerf {
  'save_inst_max' : bigint,
  'delete_inst_max' : bigint,
  'load_inst_total' : bigint,
  'load_inst_max' : bigint,
  'save_inst_total' : bigint,
  'delete_inst_total' : bigint,
}
export interface EventReport {
  'entity_counters' : Array<EntitySummary>,
  'active_window_start_ms' : bigint,
  'requested_window_start_ms' : [] | [bigint],
  'counters' : [] | [EventCounters],
  'window_filter_matched' : boolean,
}
export interface ExpandedEconomyReceipt {
  'town_id' : [] | [string],
  'action' : string,
  'offer_key' : [] | [string],
  'to_resource' : [] | [string],
  'from_resource' : [] | [string],
  'object_id' : [] | [string],
  'command_id' : string,
  'unit_slug' : [] | [string],
  'resources_after' : ResourceBalances,
  'amount_out' : bigint,
  'champion_id' : [] | [string],
  'quantity' : number,
  'amount_in' : bigint,
}
export interface FactionContent {
  'id' : string,
  'ruleset_id' : string,
  'theme' : [] | [string],
  'banner_key' : [] | [string],
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'icon_key' : [] | [string],
  'trait_key' : string,
  'native_terrain' : [] | [string],
}
export interface GameView {
  'map_chunks' : Array<MapChunkView>,
  'towns' : Array<ApiTownView>,
  'event_page_info' : EventPageInfo,
  'omitted_fields' : Array<string>,
  'battle_summary' : [] | [BattleSummary],
  'render_time' : RenderTimeMeta,
  'battle' : [] | [BattleView],
  'map_page_info' : PageInfo,
  'participant' : ParticipantSummary,
  'objects' : Array<ObjectView>,
  'session' : SessionSummary,
  'events' : Array<ApiEventView>,
  'content_manifest_hash' : string,
  'viewport' : Viewport,
  'action_affordances' : Array<ActionAffordance>,
  'object_page_info' : PageInfo,
  'champions' : Array<ChampionView>,
}
export interface GameViewRequest {
  'object_limit' : number,
  'include_battle' : boolean,
  'event_limit' : number,
  'viewport' : Viewport,
  'chunk_limit' : number,
  'object_cursor' : [] | [number],
  'chunk_cursor' : [] | [number],
  'events_after_seq' : bigint,
}
export type IndexState = { 'Building' : null } |
  { 'Ready' : null } |
  { 'Dropping' : null };
export interface IndexStoreSnapshot {
  'memory_bytes' : bigint,
  'path' : string,
  'user_entries' : bigint,
  'entries' : bigint,
  'state' : IndexState,
  'system_entries' : bigint,
}
export interface LegalBattleAction {
  'action' : string,
  'path' : Array<BattleCoord>,
  'damage_preview' : [] | [DamagePreview],
  'ability_key' : [] | [string],
  'enabled' : boolean,
  'targets' : Array<string>,
  'disabled_reason' : [] | [string],
}
export interface LobbyCommandResponse {
  'status' : CommandStatus,
  'result' : LobbyCommandResult,
  'changed_subjects' : Array<ChangedSubject>,
  'command_id' : string,
  'error' : [] | [ApiError],
  'command_type' : string,
  'events' : Array<ApiEventView>,
  'effective_turn' : number,
  'actor_principal' : Principal,
  'phase' : CommandPhase,
  'retryable' : boolean,
  'client_nonce' : string,
  'durable_turn' : number,
  'payload_hash' : string,
}
export type LobbyCommandResult = { 'None' : null } |
  { 'Session' : SessionView } |
  { 'Player' : PlayerView };
export interface MapChunkPage {
  'next_cursor' : [] | [number],
  'chunks' : Array<MapChunkView>,
  'has_more' : boolean,
}
export interface MapChunkView {
  'height' : number,
  'movement_blob' : Uint8Array | number[],
  'chunk_x' : number,
  'chunk_y' : number,
  'discovered_blob' : Uint8Array | number[],
  'chunk_id' : string,
  'width' : number,
  'visible_blob' : Uint8Array | number[],
  'terrain_blob' : Uint8Array | number[],
  'flags_blob' : Uint8Array | number[],
}
export interface MapObjectContent {
  'id' : string,
  'ruleset_id' : string,
  'interaction_key' : string,
  'blocking' : boolean,
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'refresh_rule' : string,
  'icon_key' : [] | [string],
  'footprint_h' : number,
  'footprint_w' : number,
  'object_type' : string,
  'sprite_key' : [] | [string],
}
export interface MarketTradePreview {
  'rate_key' : string,
  'to_resource' : string,
  'from_resource' : string,
  'allowed' : boolean,
  'amount_out' : bigint,
  'amount_in' : bigint,
  'disabled_reason' : [] | [string],
}
export interface MatchHistoryEntry {
  'result' : string,
  'session_id' : string,
  'opponent_name' : [] | [string],
  'turns_played' : number,
  'summary_json' : [] | [string],
}
export interface MatchHistoryPage {
  'page_info' : PageInfo,
  'entries' : Array<MatchHistoryEntry>,
}
export interface MoveCoord { 'x' : number, 'y' : number }
export interface MovementPathStop {
  'x' : number,
  'y' : number,
  'subject_kind' : string,
  'subject_id_text' : string,
  'reason' : string,
}
export interface MovementPreview {
  'available_movement' : number,
  'chunks_touched' : number,
  'path' : Array<MoveCoord>,
  'stop' : [] | [MovementPathStop],
  'total_cost' : number,
  'champion_id' : string,
  'participant_id' : string,
  'turn_number' : number,
}
export interface MovementResolutionCursor {
  'gameplay_rows_written' : number,
  'session_id' : string,
  'command_id' : string,
  'next_step_index' : number,
  'turn_number' : number,
}
export interface MovementSnapshotRecord {
  'movement_cost' : number,
  'session_id' : string,
  'remaining_after' : number,
  'command_id' : string,
  'to_x' : number,
  'to_y' : number,
  'step_index' : number,
  'created_at_ms' : bigint,
  'champion_id' : string,
  'from_x' : number,
  'from_y' : number,
  'participant_id' : string,
  'turn_number' : number,
  'outcome' : string,
  'interaction_id_text' : [] | [string],
  'snapshot_id' : string,
  'interaction_kind' : [] | [string],
  'intent_id' : string,
}
export interface MovementSyncOutcome {
  'recovered_commands_advanced' : number,
  'gameplay_rows_written' : number,
  'superseded_intent_ids' : Array<string>,
  'from_turn' : number,
  'object_stops' : Array<ObjectStopDraft>,
  'battle_starts' : Array<BattleStartDraft>,
  'session_id' : string,
  'resolved_intent_ids' : Array<string>,
  'cursor' : [] | [MovementResolutionCursor],
  'command_id' : string,
  'snapshots' : Array<MovementSnapshotRecord>,
  'budget_exhausted' : boolean,
  'advanced_turn' : boolean,
  'recovered_commands_inspected' : number,
  'current_turn' : number,
  'recovery_checked' : boolean,
}
export interface NavalRouteRecord {
  'status' : string,
  'boat_required' : boolean,
  'to_x' : number,
  'to_y' : number,
  'actionable' : boolean,
  'route_key' : string,
  'from_x' : number,
  'from_y' : number,
  'disabled_reason' : [] | [string],
  'water_crossings' : number,
}
export interface NavalRoutesView {
  'session_id' : string,
  'current_turn' : number,
  'routes' : Array<NavalRouteRecord>,
}
export interface ObjectStopDraft {
  'x' : number,
  'y' : number,
  'interaction_key' : string,
  'object_id' : string,
  'champion_id' : string,
}
export interface ObjectView {
  'x' : number,
  'y' : number,
  'owner_participant_id' : [] | [string],
  'subject_kind' : string,
  'display_name' : [] | [string],
  'details_json' : string,
  'last_seen_turn' : [] | [number],
  'asset_key' : [] | [string],
  'visibility' : string,
  'subject_id_text' : string,
  'redaction_level' : string,
}
export interface ObjectViewPage {
  'objects' : Array<ObjectView>,
  'next_cursor' : [] | [number],
  'has_more' : boolean,
}
export interface ObjectiveProgressRecord {
  'status' : string,
  'owner_participant_id' : [] | [string],
  'objective_type' : string,
  'object_id' : [] | [string],
  'last_scored_turn' : number,
  'visible_to' : string,
  'objective_key' : string,
  'required_value' : number,
  'redacted' : boolean,
  'progress_value' : number,
}
export interface ObjectiveProgressView {
  'session_id' : string,
  'objectives' : Array<ObjectiveProgressRecord>,
}
export interface PageInfo {
  'limit' : number,
  'next_cursor' : [] | [number],
  'has_more' : boolean,
}
export interface ParticipantSummary {
  'player_id' : string,
  'status' : string,
  'resources' : ResourceBalances,
  'faction_slug' : string,
  'slot_index' : number,
  'participant_id' : string,
  'ready' : boolean,
}
export interface ParticipantView {
  'player_id' : string,
  'status' : string,
  'session_id' : string,
  'resources' : ResourceCost,
  'faction_slug' : string,
  'slot_index' : number,
  'participant_id' : string,
  'ready' : boolean,
}
/**
 * Public player DTO used by the headless driver contract.
 */
export interface PlayerView {
  'player_id' : string,
  'principal' : Principal,
  'display_name' : string,
}
export interface ProceduralMapRecord {
  'status' : string,
  'scenario_hash' : string,
  'road_tile_count' : number,
  'map_height' : number,
  'town_count' : number,
  'water_tile_count' : number,
  'generation_key' : string,
  'map_width' : number,
  'land_tile_count' : number,
  'chunk_count' : number,
  'mine_count' : number,
  'generated_turn' : number,
  'chunk_size' : number,
  'map_seed' : bigint,
}
export interface ProceduralMapView {
  'session_id' : string,
  'maps' : Array<ProceduralMapRecord>,
  'current_turn' : number,
}
export type QueryErrorKind = {
    /**
     * Planning failed.
     */
    'Plan' : null
  } |
  {
    /**
     * No rows matched.
     */
    'NotFound' : null
  } |
  {
    /**
     * More than one row matched.
     */
    'NotUnique' : null
  } |
  {
    /**
     * Pagination lacked ordering.
     */
    'UnorderedPagination' : null
  } |
  {
    /**
     * Continuation cursor was invalid.
     */
    'InvalidContinuationCursor' : null
  } |
  {
    /**
     * Intent validation failed.
     */
    'Intent' : null
  } |
  {
    /**
     * Validation failed.
     */
    'Validate' : null
  };
export interface QuestPreview {
  'can_accept' : boolean,
  'quest' : QuestProgressView,
  'can_claim' : boolean,
  'disabled_reason' : [] | [string],
}
export interface QuestProgressView {
  'status' : string,
  'title' : string,
  'accepted_turn' : number,
  'reward_claimed' : boolean,
  'reward_gold' : [] | [number],
  'quest_key' : string,
  'objective_key' : string,
  'participant_id' : string,
  'claimed_turn' : number,
  'required_value' : number,
  'redacted' : boolean,
  'progress_value' : number,
}
export interface RecruitPreview {
  'town_id' : string,
  'target_slot_index' : [] | [number],
  'total_cost' : ResourceBalances,
  'allowed' : boolean,
  'unit_slug' : string,
  'available' : number,
  'quantity' : number,
  'disabled_reason' : [] | [string],
}
export type RecruitTarget = {
    'TownGarrison' : { 'slot_index' : [] | [number] }
  } |
  { 'Champion' : { 'champion_id' : string, 'slot_index' : [] | [number] } };
export interface RenderTimeMeta {
  'sync_required' : boolean,
  'turn_started_at_ms' : bigint,
  'turn_duration_ms' : bigint,
  'server_now_ms' : bigint,
}
export interface ResourceBalances {
  'aether' : number,
  'gold' : bigint,
  'iron' : number,
  'wood' : number,
  'ember' : number,
  'stone' : number,
  'crystal' : number,
}
export interface ResourceCost {
  'aether' : number,
  'gold' : number,
  'iron' : number,
  'wood' : number,
  'ember' : number,
  'stone' : number,
  'crystal' : number,
}
export type Result = { 'Ok' : CommandResponse } |
  { 'Err' : ApiError };
export type Result_1 = { 'Ok' : LobbyCommandResponse } |
  { 'Err' : ApiError };
export type Result_10 = { 'Ok' : DwellingPoolView } |
  { 'Err' : ApiError };
export type Result_11 = { 'Ok' : ApiEventPage } |
  { 'Err' : ApiError };
export type Result_12 = { 'Ok' : GameView } |
  { 'Err' : ApiError };
export type Result_13 = { 'Ok' : MatchHistoryPage } |
  { 'Err' : ApiError };
export type Result_14 = { 'Ok' : Array<ChampionView> } |
  { 'Err' : ApiError };
export type Result_15 = { 'Ok' : ParticipantView } |
  { 'Err' : ApiError };
export type Result_16 = { 'Ok' : PlayerView } |
  { 'Err' : ApiError };
export type Result_17 = { 'Ok' : NavalRoutesView } |
  { 'Err' : ApiError };
export type Result_18 = { 'Ok' : ObjectView } |
  { 'Err' : ApiError };
export type Result_19 = { 'Ok' : ObjectiveProgressView } |
  { 'Err' : ApiError };
export type Result_2 = { 'Ok' : DiagnosticSystemJobView } |
  { 'Err' : ApiError };
export type Result_20 = { 'Ok' : ProceduralMapView } |
  { 'Err' : ApiError };
export type Result_21 = { 'Ok' : ScenarioRulesView } |
  { 'Err' : ApiError };
export type Result_22 = { 'Ok' : SessionView } |
  { 'Err' : ApiError };
export type Result_23 = { 'Ok' : SetupProgressView } |
  { 'Err' : ApiError };
export type Result_24 = { 'Ok' : SiegeRulesView } |
  { 'Err' : ApiError };
export type Result_25 = { 'Ok' : SkirmishSettingsView } |
  { 'Err' : ApiError };
export type Result_26 = { 'Ok' : TavernOffersView } |
  { 'Err' : ApiError };
export type Result_27 = { 'Ok' : ApiTownView } |
  { 'Err' : ApiError };
export type Result_28 = { 'Ok' : MapChunkPage } |
  { 'Err' : ApiError };
export type Result_29 = { 'Ok' : ObjectViewPage } |
  { 'Err' : ApiError };
export type Result_3 = { 'Ok' : BattleView } |
  { 'Err' : ApiError };
export type Result_30 = { 'Ok' : WorldEventsView } |
  { 'Err' : ApiError };
export type Result_31 = { 'Ok' : EventReport } |
  { 'Err' : Error };
export type Result_32 = { 'Ok' : null } |
  { 'Err' : Error };
export type Result_33 = { 'Ok' : StorageReport } |
  { 'Err' : Error };
export type Result_34 = { 'Ok' : BuildPreview } |
  { 'Err' : ApiError };
export type Result_35 = { 'Ok' : ChampionProgressionView } |
  { 'Err' : ApiError };
export type Result_36 = { 'Ok' : DwellingRecruitPreview } |
  { 'Err' : ApiError };
export type Result_37 = { 'Ok' : ChampionHirePreview } |
  { 'Err' : ApiError };
export type Result_38 = { 'Ok' : MarketTradePreview } |
  { 'Err' : ApiError };
export type Result_39 = { 'Ok' : MovementPreview } |
  { 'Err' : ApiError };
export type Result_4 = { 'Ok' : ChampionView } |
  { 'Err' : ApiError };
export type Result_40 = { 'Ok' : QuestPreview } |
  { 'Err' : ApiError };
export type Result_41 = { 'Ok' : RecruitPreview } |
  { 'Err' : ApiError };
export type Result_42 = { 'Ok' : DiagnosticProjectionFlushView } |
  { 'Err' : ApiError };
export type Result_43 = { 'Ok' : bigint } |
  { 'Err' : ApiError };
export type Result_44 = { 'Ok' : number } |
  { 'Err' : ApiError };
export type Result_5 = { 'Ok' : CommandStatusView } |
  { 'Err' : ApiError };
export type Result_6 = { 'Ok' : ContentManifestResponse } |
  { 'Err' : ApiError };
export type Result_7 = { 'Ok' : DiagnosticProjectionSnapshot } |
  { 'Err' : ApiError };
export type Result_8 = { 'Ok' : DiagnosticStorageSnapshot } |
  { 'Err' : ApiError };
export type Result_9 = { 'Ok' : DiagnosticSystemJobPage } |
  { 'Err' : ApiError };
export interface RulesetContent {
  'id' : string,
  'player_count' : number,
  'max_turns' : number,
  'turn_duration_ms' : number,
  'map_height' : number,
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'map_width' : number,
  'version' : number,
  'content_manifest_hash' : string,
  'chunk_size' : number,
}
export type RuntimeErrorKind = { 'Internal' : null } |
  { 'IncompatiblePersistedFormat' : null } |
  { 'InvariantViolation' : null } |
  { 'Corruption' : null } |
  { 'NotFound' : null } |
  { 'Unsupported' : null } |
  { 'Conflict' : null };
export interface ScenarioRuleView {
  'rule_type' : string,
  'status' : string,
  'rule_key' : string,
  'owner_participant_id' : [] | [string],
  'victory_state' : string,
  'last_checked_turn' : number,
  'winner_participant_id' : [] | [string],
  'disabled_reason' : [] | [string],
  'current_value' : number,
  'required_value' : number,
}
export interface ScenarioRulesView {
  'session_id' : string,
  'current_turn' : number,
  'rules' : Array<ScenarioRuleView>,
}
export interface SessionSummary {
  'session_id' : string,
  'participant_ids' : Array<string>,
  'state' : string,
  'current_turn' : number,
}
/**
 * Public session DTO used by lobby and session smoke paths.
 */
export interface SessionView {
  'session_id' : string,
  'participant_ids' : Array<string>,
  'state' : string,
}
export interface SetupProgressView {
  'session_id' : string,
  'setup_job_attempt_count' : number,
  'total_effect_count' : number,
  'setup_command_id' : [] | [string],
  'session_state' : string,
  'last_effect_key' : [] | [string],
  'setup_command_status' : [] | [string],
  'setup_job_status' : [] | [string],
  'completed_effect_count' : number,
  'setup_complete' : boolean,
  'next_effect_key' : [] | [string],
}
export interface SiegeRuleRecord {
  'status' : string,
  'rule_key' : string,
  'battle_obstacle_cap' : number,
  'tower_count' : number,
  'actionable' : boolean,
  'wall_segments' : number,
  'disabled_reason' : [] | [string],
  'fortification_level' : string,
  'siege_engine_slots' : number,
  'gate_count' : number,
}
export interface SiegeRulesView {
  'session_id' : string,
  'current_turn' : number,
  'rules' : Array<SiegeRuleRecord>,
}
export interface SkirmishSettingsRecord {
  'larger_map_enabled' : boolean,
  'status' : string,
  'player_count' : number,
  'neutral_difficulty' : string,
  'map_height' : number,
  'fog_enabled' : boolean,
  'generation_key' : string,
  'siege_enabled' : boolean,
  'map_width' : number,
  'victory_condition' : string,
  'chunk_size' : number,
  'naval_enabled' : boolean,
  'map_seed' : bigint,
  'profile_key' : string,
}
export interface SkirmishSettingsView {
  'session_id' : string,
  'settings' : SkirmishSettingsRecord,
  'current_turn' : number,
}
export interface SpellContent {
  'id' : string,
  'ruleset_id' : string,
  'mana_cost' : number,
  'school' : string,
  'name' : string,
  'slug' : string,
  'description' : [] | [string],
  'level' : number,
  'icon_key' : [] | [string],
  'duration_rounds' : number,
  'effect_key' : string,
  'target_type' : string,
}
export interface StorageReport {
  'corrupted_entries' : bigint,
  'corrupted_keys' : bigint,
  'storage_index' : Array<IndexStoreSnapshot>,
  'entity_storage' : Array<EntitySnapshot>,
  'storage_data' : Array<DataStoreSnapshot>,
}
export interface StrategicCommandReceipt {
  'command_count' : number,
  'command_id' : string,
  'event_count' : number,
  'command_kind' : string,
  'current_turn' : number,
}
export interface TavernOfferView {
  'status' : string,
  'town_id' : string,
  'offer_key' : string,
  'offer_slot' : number,
  'hired_champion_id' : [] | [string],
  'cost_gold' : number,
  'candidate_name' : string,
  'champion_class_slug' : string,
  'week_number' : number,
}
export interface TavernOffersView {
  'town_id' : string,
  'offers' : Array<TavernOfferView>,
  'week_number' : number,
}
export interface TerrainContent {
  'id' : string,
  'ruleset_id' : string,
  'movement_cost' : number,
  'passable' : boolean,
  'name' : string,
  'terrain_code' : number,
  'sprite_key' : [] | [string],
  'terrain_key' : string,
}
export interface TownBuildingRecord {
  'town_id' : string,
  'session_id' : string,
  'built_turn' : number,
  'building_slug' : string,
  'building_id' : string,
}
export interface TownRecord {
  'x' : number,
  'y' : number,
  'status' : string,
  'town_id' : string,
  'owner_participant_id' : string,
  'income_started_turn' : number,
  'hall_level' : number,
  'session_id' : string,
  'unrest_until_turn' : number,
  'name' : string,
  'faction_slug' : string,
  'captured_turn' : number,
  'fort_level' : number,
  'last_command_id' : [] | [string],
  'last_built_turn' : number,
}
export interface TownRecruitPoolRecord {
  'town_id' : string,
  'session_id' : string,
  'unit_slug' : string,
  'available' : number,
  'pool_id' : string,
  'last_command_id' : [] | [string],
  'last_growth_week' : number,
}
export interface UnitContent {
  'id' : string,
  'ruleset_id' : string,
  'weekly_growth' : number,
  'cost' : ResourceCost,
  'animation_key' : [] | [string],
  'flying' : boolean,
  'name' : string,
  'slug' : string,
  'tier' : number,
  'description' : [] | [string],
  'faction_slug' : [] | [string],
  'shots' : number,
  'icon_key' : [] | [string],
  'speed' : number,
  'defense' : number,
  'ranged' : boolean,
  'damage_max' : number,
  'damage_min' : number,
  'max_hp' : number,
  'ability_keys' : Array<string>,
  'attack' : number,
  'initiative' : number,
  'sprite_key' : [] | [string],
}
export interface Viewport {
  'x' : number,
  'y' : number,
  'height' : number,
  'width' : number,
}
export interface WorldEventView {
  'event_key' : string,
  'status' : string,
  'ends_turn' : number,
  'starts_turn' : number,
  'event_window' : string,
  'event_type' : string,
  'payload' : [] | [string],
  'redacted' : boolean,
}
export interface WorldEventsView {
  'session_id' : string,
  'events' : Array<WorldEventView>,
  'current_turn' : number,
}
export interface WorldGenerationReceipt {
  'scenario_hash' : string,
  'action' : string,
  'command_id' : string,
  'map_height' : number,
  'generation_key' : string,
  'map_width' : number,
  'state' : string,
  'chunk_count' : number,
  'current_turn' : number,
}
export interface _SERVICE {
  'accept_quest' : ActorMethod<[string, string, string], Result>,
  'cast_adventure_spell' : ActorMethod<
    [string, string, string, string],
    Result
  >,
  'claim_quest_reward' : ActorMethod<[string, string, string], Result>,
  'create_session' : ActorMethod<[string, string, bigint, string], Result_1>,
  'end_battle_turn' : ActorMethod<[string, string, string], Result>,
  'end_turn' : ActorMethod<[string, string], Result>,
  'force_diagnostic_system_job_running' : ActorMethod<
    [string, bigint],
    Result_2
  >,
  'get_battle_state' : ActorMethod<[string, string], Result_3>,
  'get_canister_endpoint_inventory' : ActorMethod<
    [],
    Array<CanisterEndpointView>
  >,
  'get_champion_view' : ActorMethod<[string, string], Result_4>,
  'get_command_status' : ActorMethod<[string, string], Result_5>,
  'get_command_status_by_nonce' : ActorMethod<
    [string, string, string],
    Result_5
  >,
  'get_content_manifest' : ActorMethod<[string, number], Result_6>,
  'get_diagnostic_projection_snapshot' : ActorMethod<[], Result_7>,
  'get_diagnostic_storage_snapshot' : ActorMethod<[Array<string>], Result_8>,
  'get_diagnostic_system_jobs' : ActorMethod<
    [[] | [string], [] | [string], number, [] | [string]],
    Result_9
  >,
  'get_dwelling_pool' : ActorMethod<[string, string], Result_10>,
  'get_events_after' : ActorMethod<[string, string, bigint, number], Result_11>,
  'get_game_view' : ActorMethod<[string, GameViewRequest], Result_12>,
  'get_match_history' : ActorMethod<[number, number], Result_13>,
  'get_my_champions' : ActorMethod<[string], Result_14>,
  'get_my_participant' : ActorMethod<[string], Result_15>,
  'get_my_player' : ActorMethod<[], Result_16>,
  'get_naval_routes' : ActorMethod<[string], Result_17>,
  'get_object_view' : ActorMethod<[string, string, string], Result_18>,
  'get_objective_progress' : ActorMethod<[string], Result_19>,
  'get_procedural_map_state' : ActorMethod<[string], Result_20>,
  'get_scenario_rules' : ActorMethod<[string], Result_21>,
  'get_session' : ActorMethod<[string], Result_22>,
  'get_setup_progress' : ActorMethod<[string], Result_23>,
  'get_siege_rules' : ActorMethod<[string], Result_24>,
  'get_skirmish_settings' : ActorMethod<[string], Result_25>,
  'get_tavern_offers' : ActorMethod<[string, string], Result_26>,
  'get_town_view' : ActorMethod<[string, string], Result_27>,
  'get_visible_map_chunks' : ActorMethod<
    [string, Viewport, [] | [number], number],
    Result_28
  >,
  'get_visible_objects' : ActorMethod<
    [string, Viewport, [] | [number], number],
    Result_29
  >,
  'get_world_events' : ActorMethod<[string], Result_30>,
  'hire_tavern_champion' : ActorMethod<
    [string, string, string, string],
    Result
  >,
  'icydb_metrics' : ActorMethod<[[] | [bigint]], Result_31>,
  'icydb_metrics_reset' : ActorMethod<[], Result_32>,
  'icydb_snapshot' : ActorMethod<[], Result_33>,
  'join_session' : ActorMethod<[string, string, string], Result_1>,
  'learn_champion_spell' : ActorMethod<
    [string, string, string, string],
    Result
  >,
  'mark_ready' : ActorMethod<[string, string], Result_1>,
  'preview_build_town_structure' : ActorMethod<
    [string, string, string],
    Result_34
  >,
  'preview_champion_progression' : ActorMethod<[string, string], Result_35>,
  'preview_dwelling_recruit' : ActorMethod<
    [string, string, string, number, string],
    Result_36
  >,
  'preview_hire_champion' : ActorMethod<[string, string, string], Result_37>,
  'preview_market_trade' : ActorMethod<
    [string, string, string, bigint],
    Result_38
  >,
  'preview_move_path' : ActorMethod<
    [string, string, Array<MoveCoord>],
    Result_39
  >,
  'preview_quest' : ActorMethod<[string, string], Result_40>,
  'preview_recruit_units' : ActorMethod<
    [string, string, string, number, RecruitTarget],
    Result_41
  >,
  'register_player' : ActorMethod<
    [[] | [string], [] | [string], string],
    Result_1
  >,
  'run_diagnostic_battle_projection_flush' : ActorMethod<[], Result_42>,
  'run_diagnostic_flush_barrier' : ActorMethod<[string], Result_43>,
  'run_diagnostic_projection_flush' : ActorMethod<[], Result_42>,
  'run_diagnostic_system_job' : ActorMethod<[string], Result_44>,
  'run_diagnostic_system_jobs' : ActorMethod<[number], Result_44>,
  'select_champion_level_up' : ActorMethod<
    [string, string, string, string],
    Result
  >,
  'start_session' : ActorMethod<[string, string], Result_1>,
  'submit_battle_action' : ActorMethod<
    [string, BattleActionInput, string],
    Result
  >,
  'submit_build_town_structure' : ActorMethod<
    [string, string, string, string],
    Result
  >,
  'submit_dwelling_recruit' : ActorMethod<
    [string, string, string, number, string, string],
    Result
  >,
  'submit_market_trade' : ActorMethod<
    [string, string, string, bigint, string],
    Result
  >,
  'submit_move_intent' : ActorMethod<
    [string, string, Array<MoveCoord>, string],
    Result
  >,
  'submit_recruit_units' : ActorMethod<
    [string, string, string, number, RecruitTarget, string],
    Result
  >,
  'sync_advanced_victory' : ActorMethod<[string, string], Result>,
  'sync_battle' : ActorMethod<[string, string, string], Result>,
  'sync_objectives' : ActorMethod<[string, string], Result>,
  'sync_session_turn' : ActorMethod<[string, string], Result>,
  'sync_world_events' : ActorMethod<[string, string], Result>,
  'sync_world_generation' : ActorMethod<[string, string], Result>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
