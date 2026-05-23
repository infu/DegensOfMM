# DoMM Client Spec

Status: starting client architecture, 2026-05-22.

Primary backend source of truth: `docs/client-ui-integration.md`,
`docs/canister-endpoints.md`, exported Candid, and Gate M.

This spec defines the first real web client architecture for Degens of Misery
and Mayhem. The goal is a canvas-first game client with a small, testable UI
kernel that can grow into larger maps, multiple battles, richer city screens,
and gameover/history without becoming a pile of direct canister calls and
imperative canvas state.

## Core Decisions

Use this stack for the first client:

```text
Language: TypeScript
App shell: React
Build tool: Vite
Client kernel: Redux Toolkit
Network/cache: RTK Query with custom actor query functions
Renderer: PixiJS
Panels/forms: React DOM
Canister calls: generated Candid actor behind a typed port
Tests: Vitest + Playwright
```

Do not use raw Canvas 2D as the main rendering API. Raw canvas is fine for tiny
experiments, but this game needs layers, sprites, hit testing, camera movement,
fog overlays, battle grids, interaction affordances, and future animation.

Do not use D3 as the main renderer. D3 is useful for data transforms, scales,
debug charts, timelines, and maybe a minimap utility, but it should not own the
map or battle scene. The game renderer needs a sprite/container scene graph and
a render loop; PixiJS is the better fit.

Use Redux Toolkit, not hand-written Redux boilerplate. The store is the client
kernel. React components and Pixi renderers read selectors and dispatch semantic
actions; they do not call the canister directly.

RTK Query is used, but not as the whole game model. It owns request lifecycle,
in-flight dedupe, polling, cache metadata, and endpoint invalidation. The
canonical client projection lives in normalized domain slices fed by fulfilled
endpoint responses and canister command/event results.

## Player Experience Contract

The kernel exists to make the game feel immediate without lying about
authoritative state.

Required UX properties:

- Hovering, selecting, panning, zooming, and opening already-loaded panels must
  feel instant.
- Path drafts, target highlights, and disabled reasons should appear before the
  player waits on a canister update.
- Every command button must have a visible lifecycle: available, checking,
  submitting, applying, applied, failed, or needs sync.
- The client should explain why an action is unavailable whenever the canister
  or local rules know the reason.
- Network latency must not freeze the canvas or block camera/input.
- The player should never see stale hidden information as fact.
- The player should never lose the context of what they clicked while the
  kernel refreshes affected views.
- Gameover should feel like a state transition, not an error page.

The client may predict, preview, and animate. It must not forge truth. Any
predicted result is labeled visually as a preview until canister responses,
events, or refreshed views confirm it.

## Visual Design References

Mock design images referenced by the spec, design notes, or future checked-in
client assets are style and layout direction for the first UI. Use them for
overall screen structure, information hierarchy, panel density, canvas
treatment, control grouping, and mood. They are not pixel-perfect contracts.

When the mock references conflict with the actual game flow, the actual game
wins. The client must fit the real canister workflow: lobby, setup progress,
active map, movement intent planning, town panels, battle board, events,
command status, sync states, and gameover/history. Adjust spacing, panel
placement, copy, and interaction details when the backend contracts or
playability require it.

Design interpretation rules:

- Treat the strategic map and battle board as the primary visual surfaces.
- Keep the canvas large and immediately playable; do not turn the first screen
  into a marketing/landing page.
- Use DOM side panels, bottom bars, overlays, and modals for dense text and
  controls, but do not bury the map in nested cards.
- Preserve a strategy-game cockpit feel: quick scanning, compact controls,
  clear turn/command state, and visible ownership/selection/status cues.
- Reuse the mock images' broad style language, but adapt colors, scale,
  density, and layout to the actual DoMM map, towns, champions, battles, and
  event feed.
- Do not copy or ship third-party art from reference images unless its license
  is explicit. The content manifest owns real game asset keys.
- If no mock image path is committed yet, implementation should create a small
  `docs/client-ui-visuals.md` or `apps/web/design/README.md` index before
  building visual assets so later agents know which references were used.

## Hard Boundaries

The client has four layers:

```text
app/        React shell, routes, layout, panels
kernel/     Redux slices, selectors, command/event orchestration
ports/      generated Candid actor wrappers, storage, clock, telemetry
render/     PixiJS scene, camera, layers, hit testing, animation
```

Rules:

- Components never import generated Candid actor methods directly.
- Pixi render code never imports generated Candid actor methods directly.
- Reducers are pure and deterministic.
- Canister DTOs are adapted at the boundary before entering domain slices.
- Server state and UI planning overlays are separate.
- The canister remains authoritative for world state.
- Optimistic world mutation is forbidden except local intent previews.
- The render loop must not dispatch Redux actions every frame.

## Reevaluated Kernel Shape

The kernel should be three subsystems inside one Redux Toolkit store:

```text
transport cache:
  RTK Query API slice using custom queryFn wrappers around the generated actor

domain projection:
  normalized Redux slices that represent the current client-readable game state

workflow engine:
  listener middleware that reacts to semantic intents, endpoint fulfillment,
  command responses, timers, and visibility changes
```

This is stricter than "put everything in Redux" and safer than "let components
call endpoints." The client needs a deterministic projection that the renderer
can consume cheaply, but it also needs robust request lifecycle behavior.
RTK Query is good at endpoint caching and invalidation; the game kernel is good
at composing many endpoint responses into one consistent play view.

Gameplay components read only domain selectors. They do not read raw RTK Query
cache entries and do not use generated endpoint hooks directly. A non-gameplay
admin/debug screen may read raw endpoint status if it is explicitly labeled as
diagnostic.

Kernel data flow:

```text
UI/Pixi intent
  -> semantic Redux action
  -> listener workflow
  -> RTK Query endpoint initiate or command port call
  -> raw endpoint response
  -> adapter emits kernel/domain actions
  -> normalized domain slices update
  -> selectors produce React panel view models and Pixi RenderFrame
```

Do not let endpoint response shapes leak upward. The generated Candid client is
a transport detail. The renderer sees `RenderFrame`; React panels see feature
view models; reducers see adapted domain payloads.

## Kernel Invariants

These invariants are more important than the exact folder layout:

- Only adapters may translate generated Candid DTOs.
- Only listeners may perform async gameplay workflows.
- Only reducers may update domain projection state.
- Only selectors may compose render/panel view models.
- Only the renderer may own Pixi containers, sprites, textures, and animation.
- Only the canister may decide legal world-state changes.
- Local previews may draw overlays, but they must not mutate authoritative
  champion, town, battle, resource, or event state.
- Every network response carries a request token, viewport key, command local
  id, or endpoint args sufficient to ignore stale responses.
- Every long-running workflow has a finite status enum and a cancellation path.
- Every public canister command has a command record until it is applied,
  failed, superseded, or intentionally abandoned.

## Rules-Aware UX Kernel

The client needs a read-only rules layer for responsiveness. This layer is not
a second game engine. It is a planning and affordance engine built from content
manifest data, current visible state, canister previews, and battle legal-action
metadata.

Add a `rules/` module:

```text
rules/
  contentRules.ts
  movementPlanner.ts
  battlePlanner.ts
  townPlanner.ts
  affordability.ts
  affordanceModel.ts
  rulesWorker.ts
```

Inputs:

```text
ContentManifest
SessionSummary / ParticipantSummary
visible MapChunkView data
visible ObjectView data
ChampionView / ApiTownView / BattleView
preview_* responses
BattleView.legal_actions_for_caller
current command/pending-sync state
```

Outputs:

```text
ActionAffordanceModel
PathDraftModel
TargetHighlightModel
PanelControlModel
DisabledReasonModel
RenderOverlayModel
```

The rules layer may:

- Compute local path candidates over visible/surveyed terrain.
- Estimate movement cost for display before `preview_move_path` returns.
- Mark controls as clearly impossible from local visible state.
- Group battle legal actions into player-friendly controls.
- Highlight legal battle targets and movement destinations from
  `legal_actions_for_caller`.
- Explain visible affordability and prerequisite failures from preview
  responses.
- Build render overlays for paths, threat, selection, and expected command
  targets.

The rules layer must not:

- Spend resources.
- Move champions authoritatively.
- Resolve simultaneous movement.
- Resolve battle damage or RNG.
- Reveal hidden blockers, armies, towns, or events.
- Mark a command as legally confirmed without canister preview/legal-action
  evidence.
- Override a canister error or command response.

Control states should be explicit:

```text
hidden
disabled_local
needs_preview
previewing
enabled_confirmed
submitting
applying
failed
needs_sync
```

For example, a movement path can be drawn as a draft from local terrain
immediately, but the submit button should become `enabled_confirmed` only after
`preview_move_path` accepts the path for the current champion/turn snapshot.

Rules snapshots are versioned. A planned path, town preview, or battle target
set records:

```text
session_id
current_turn
content_manifest_hash
champion_id / town_id / battle_id
battle_round if applicable
source_view_revision
```

If any version key changes, the kernel invalidates the preview and returns the
control to `needs_preview` or `needs_sync`.

Large planning work must not block the UI thread. `rulesWorker.ts` should be
introduced when local pathing, battle overlays, or future larger maps exceed a
small synchronous budget. Worker messages must be plain serializable snapshots,
not Redux state objects, Pixi objects, or generated Candid classes.

### Rules Worker Protocol

Rules workers receive plain snapshots: typed arrays, ids, content hashes,
source view revisions, and request ids. They never receive Redux state, DTO
classes, or Pixi objects.

Worker responses are applied only if `request_id` and `snapshot_key` still
match the active planning state. Superseded path/range jobs are canceled when
possible and ignored otherwise. Large terrain, cost, and visibility blobs
should use transferable `ArrayBuffer`s.

## Why Not Saga Or XState First

Do not add Redux Saga for the first client. Listener middleware is already part
of Redux Toolkit, supports reacting to actions/state, and is enough for the
first set of game workflows.

Do not add XState for the first client. Use explicit finite status fields in
slices for setup, command submission, battle aftermath, and viewport loading.
If the UI later grows flows that are hard to reason about with listener
middleware, a state-machine library can be introduced for those isolated flows,
not as the global app model.

Required finite-state domains:

```text
identity: anonymous | loading | registered | failed
lobby: idle | creating | joining | ready | starting | failed
setup: inactive | polling | active | failed
viewport: idle | loading | refreshing | ready | stale | failed
command: queued | submitting | submitted | applying | applied | failed | needs_sync | abandoned
battle aftermath: idle | syncing | no_op | applied | failed
gameover: active | finalizing | finished
```

## Library Rationale

PixiJS is the main renderer because it provides a canvas-backed 2D renderer,
containers, sprites, graphics, textures, events, and a render loop suited to
game scenes. It lets us render a large tile map and battle scene without
building our own scene graph.

Redux Toolkit is the kernel because the game has many independent but related
state domains: session, setup, events, commands, map chunks, visible objects,
champions, towns, battles, selections, previews, and pending sync. Normalized
slices, entity adapters, selectors, listener middleware, and serializable
actions give us a debuggable model.

RTK Query is the endpoint/cache subsystem because canister reads and command
status polling are still server-state fetching problems. Use a custom
`baseQuery` or per-endpoint `queryFn` that calls the generated actor instead of
HTTP `fetch`. Use tags for coarse invalidation, but use the domain refresh graph
below for gameplay-specific refresh decisions.

React owns HTML UI: lobby, panels, modals, town screens, champion sheets,
toolbars, logs, settings, and result/history. It should not own the Pixi object
tree node by node. React can mount the canvas and subscribe to selectors, while
the Pixi bridge performs efficient scene reconciliation.

D3 may be used only as small utility packages where useful:

```text
d3-scale      minimap or chart scales
d3-array      summaries/debug tables
d3-quadtree   optional hit-test/spatial utility if Pixi hit testing is not enough
```

Do not add the whole D3 package for core gameplay rendering.

## Proposed File Layout

```text
apps/web/
  index.html
  package.json
  vite.config.ts
  tsconfig.json
  src/
    app/
      App.tsx
      routes.tsx
      GameScreen.tsx
      LobbyScreen.tsx
      ResultScreen.tsx
    kernel/
      api.ts
      store.ts
      rootReducer.ts
      listeners.ts
      workflows/
        lobbyWorkflow.ts
        viewportWorkflow.ts
        commandWorkflow.ts
        battleWorkflow.ts
        syncWorkflow.ts
      refreshGraph.ts
      selectors.ts
      identitySlice.ts
      sessionSlice.ts
      setupSlice.ts
      manifestSlice.ts
      mapSlice.ts
      objectSlice.ts
      championSlice.ts
      townSlice.ts
      battleSlice.ts
      eventSlice.ts
      commandSlice.ts
      previewSlice.ts
      uiSlice.ts
    rules/
      contentRules.ts
      movementPlanner.ts
      battlePlanner.ts
      townPlanner.ts
      affordability.ts
      affordanceModel.ts
      rulesWorker.ts
    ports/
      canister/
        actor.ts
        client.ts
        dtoAdapters.ts
        endpointTypes.ts
      clock.ts
      storage.ts
      telemetry.ts
    render/
      PixiRoot.tsx
      RendererBridge.ts
      RenderFrame.ts
      layers/
        TerrainLayer.ts
        FogLayer.ts
        ObjectLayer.ts
        ChampionLayer.ts
        PathLayer.ts
        BattleLayer.ts
        OverlayLayer.ts
      input/
        hitTest.ts
        pointerActions.ts
      assets/
        assetCatalog.ts
    features/
      lobby/
      map/
      town/
      champion/
      battle/
      events/
      result/
    test/
      fakeCanister.ts
      fixtures/
```

This layout is a starting point. Keep ownership boundaries more important than
exact folder names.

## Kernel State Model

The Redux store is the client kernel. It stores normalized domain state plus
small UI state.

Core slices:

```text
identity
  principal
  player_id
  username/display_name
  auth_state

session
  session_id
  state
  participant_ids
  participant_id
  current_turn
  render_time
  content_manifest_hash

result
  finished
  winner_participant_id: from victory_finalized event payload or match history summary_json
  finish_reason: from victory_finalized event payload or match history summary_json
  my_result: win | loss | unknown from get_match_history

setup
  setup_complete
  completed_effect_count
  total_effect_count
  setup_command_status
  setup_job_status

manifest
  content_hash
  factions
  terrain
  units
  buildings
  spells
  artifacts
  map_objects
  asset_keys

rules
  content_rules_revision
  local_planner_status
  active_affordance_revision

map
  chunks_by_id
  loaded_viewports
  requested_viewports
  fog/discovery metadata

objects
  objects_by_subject_key
  visibility
  redaction_level
  last_seen_turn

champions
  champions_by_id
  my_champion_ids
  selected_champion_id

towns
  towns_by_id
  buildings_by_town
  recruit_pools_by_town
  garrisons_by_town

battles
  battles_by_id
  active_battle_id
  stacks_by_battle_id
  legal_actions_by_battle_id
  battle_events_by_battle_id

events
  public_cursor
  private_cursor
  events_by_seq
  unread_count

commands
  commands_by_local_id
  command_id_index
  nonce_index
  pending_intents

previews
  movement_path
  build_preview
  recruit_preview
  battle_action_preview
  preview_revision

ui
  screen
  selected_tile
  selected_subject
  open_panel
  tool_mode
  camera_bookmark
  modal_stack
```

Use `createEntityAdapter` for collections with stable ids. Use plain keyed maps
when the key is a compound string such as `subject_kind:subject_id_text` or
`session_id:audience_key`.

Never store raw generated Candid DTOs in render selectors. Convert them through
adapters first so field names, redaction rules, and missing optional values are
handled once.

## Store Configuration

The store should be configured as one root store:

```ts
const listenerMiddleware = createListenerMiddleware();

export const store = configureStore({
  reducer: {
    [api.reducerPath]: api.reducer,
    identity,
    session,
    result,
    setup,
    manifest,
    rules,
    map,
    objects,
    champions,
    towns,
    battles,
    events,
    commands,
    previews,
    ui,
  },
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware({
      serializableCheck: {
        // Keep this strict. Fix adapters before disabling checks broadly.
      },
    })
      .prepend(listenerMiddleware.middleware)
      .concat(api.middleware),
});
```

Keep Redux serializability checks enabled. If generated Candid values fail the
check, that is a DTO adapter bug, not a reason to disable checks globally.

Candid adaptation rules:

```text
Principal -> text
Nat64/Int64/bigint -> string unless safely bounded by the UI domain
Candid opt -> T | null
Candid variant -> discriminated union with `kind`
Id/Text -> string
timestamp millis -> number only if within JS safe integer, otherwise string
Vec<T> -> readonly-friendly arrays copied at the boundary
```

Reducers should never receive actor instances, functions, class instances,
`Principal` objects, raw `bigint`, `Map`, `Set`, `Date`, or Pixi objects.

## Data Ownership Policy

There are four categories of data:

```text
transport response cache:
  owned by RTK Query
  examples: latest raw endpoint result, loading/error timestamps, polling status

domain projection:
  owned by normalized Redux slices
  examples: ChampionModel, TownModel, BattleModel, ObjectModel, EventModel

view models:
  owned by selectors
  examples: RenderFrame, TownPanelViewModel, BattleActionBarViewModel

ephemeral interaction:
  owned by renderer or ui slice depending on persistence needs
  examples: pointer drag delta, camera easing, hover tile, modal stack
```

The domain projection is the only source that gameplay React components and the
Pixi bridge read for current game state. RTK Query's cache is a transport
helper, not the canonical object graph.

Recommended policy:

```text
raw response needed only for request status -> keep in RTK Query
raw response changes gameplay view -> adapt into domain slice
state derived from several slices -> selector only
state changes every frame -> renderer local state
state must survive route switch/reload -> ui slice or local storage
```

Avoid double-authority bugs. If a value exists in both RTK Query cache and a
domain slice, the domain slice is what the game renders. The raw cache may be
evicted without changing the current play view.

## Server State Versus Client Overlays

Separate authoritative state from local planning:

```text
authoritative:
  session state from get_session/get_game_view
  map chunks and visible objects
  champion/town/battle DTOs
  events and command responses

planning overlays:
  hovered tile
  selected path candidate
  preview_move_path response
  pending command spinner
  local camera animation
  drag rectangle
  tooltip state
```

The renderer may show a path arrow, target marker, predicted damage, or pending
badge before the command is applied. It must not move a champion, spend
resources, capture a town, or remove a stack until the canister response,
events, or refreshed views make that state authoritative.

## Canister Port

All canister traffic goes through one typed port, and the RTK Query API slice
uses that port internally.

The port must expose one typed method per public gameplay Candid endpoint. Do
not hide gameplay updates behind a generic `submitCommand`; `command_type` must
remain explicit for nonce replay and `get_command_status_by_nonce`. Endpoint
wrapper names may be camelCase in TypeScript, but their canister method name
and command type string must be preserved.

```ts
export interface DommCanisterPort {
  registerPlayer(...): Promise<LobbyCommandResponse>;
  getMyPlayer(): Promise<PlayerView>;
  createSession(...): Promise<LobbyCommandResponse>;
  joinSession(...): Promise<LobbyCommandResponse>;
  markReady(...): Promise<LobbyCommandResponse>;
  startSession(...): Promise<LobbyCommandResponse>;
  getSession(sessionId: string): Promise<SessionView>;
  getSetupProgress(sessionId: string): Promise<SetupProgressView>;
  getMyParticipant(sessionId: string): Promise<ParticipantView>;
  getMatchHistory(...): Promise<MatchHistoryPage>;
  getGameView(sessionId: string, request: GameViewRequest): Promise<GameView>;
  getVisibleMapChunks(...): Promise<MapChunkPage>;
  getVisibleObjects(...): Promise<ObjectViewPage>;
  getObjectView(...): Promise<ObjectView>;
  getMyChampions(sessionId: string): Promise<ChampionView[]>;
  getChampionView(sessionId: string, championId: string): Promise<ChampionView>;
  previewChampionProgression(...): Promise<ChampionProgressionView>;
  selectChampionLevelUp(...): Promise<CommandResponse>;
  learnChampionSpell(...): Promise<CommandResponse>;
  castAdventureSpell(...): Promise<CommandResponse>;
  getTavernOffers(...): Promise<TavernOfferPage>;
  previewHireChampion(...): Promise<ChampionHirePreview>;
  hireTavernChampion(...): Promise<CommandResponse>;
  previewMarketTrade(...): Promise<MarketTradePreview>;
  submitMarketTrade(...): Promise<CommandResponse>;
  getDwellingPool(...): Promise<DwellingPoolView>;
  previewDwellingRecruit(...): Promise<DwellingRecruitPreview>;
  submitDwellingRecruit(...): Promise<CommandResponse>;
  getObjectiveProgress(...): Promise<ObjectiveProgressView>;
  getScenarioRules(...): Promise<ScenarioRulesView>;
  getWorldEvents(...): Promise<WorldEventsView>;
  previewQuest(...): Promise<QuestPreview>;
  acceptQuest(...): Promise<CommandResponse>;
  claimQuestReward(...): Promise<CommandResponse>;
  syncObjectives(...): Promise<CommandResponse>;
  syncWorldEvents(...): Promise<CommandResponse>;
  syncAdvancedVictory(...): Promise<CommandResponse>;
  getSkirmishSettings(...): Promise<SkirmishSettingsView>;
  getProceduralMapState(...): Promise<ProceduralMapStateView>;
  getNavalRoutes(...): Promise<NavalRoutesView>;
  getSiegeRules(...): Promise<SiegeRulesView>;
  syncWorldGeneration(...): Promise<CommandResponse>;
  getTownView(sessionId: string, townId: string): Promise<ApiTownView>;
  getBattleState(sessionId: string, battleId: string): Promise<BattleView>;
  getContentManifest(args): Promise<ContentManifest>;
  getEventsAfter(...): Promise<ApiEventPage>;
  getCommandStatus(sessionId: string, commandId: string): Promise<CommandStatusView>;
  getCommandStatusByNonce(...): Promise<CommandStatusView>;
  previewMovePath(...): Promise<MovementPreview>;
  previewBuildTownStructure(...): Promise<BuildPreview>;
  previewRecruitUnits(...): Promise<RecruitPreview>;
  submitMoveIntent(...): Promise<CommandResponse>;
  endTurn(...): Promise<CommandResponse>;
  syncSessionTurn(...): Promise<CommandResponse>;
  submitBuildTownStructure(...): Promise<CommandResponse>;
  submitRecruitUnits(...): Promise<CommandResponse>;
  syncBattle(...): Promise<CommandResponse>;
  endBattleTurn(...): Promise<CommandResponse>;
  submitBattleAction(...): Promise<CommandResponse>;
}
```

The method names and endpoint coverage above are normative. The exact
TypeScript DTO type names come from the generated Candid declarations; semantic
names in this spec are placeholders when generated names differ.

Required generated actor to port mapping:

| Canister method | Port method |
| --- | --- |
| `register_player` | `registerPlayer` |
| `get_my_player` | `getMyPlayer` |
| `create_session` | `createSession` |
| `join_session` | `joinSession` |
| `mark_ready` | `markReady` |
| `start_session` | `startSession` |
| `get_session` | `getSession` |
| `get_setup_progress` | `getSetupProgress` |
| `get_my_participant` | `getMyParticipant` |
| `get_match_history` | `getMatchHistory` |
| `get_game_view` | `getGameView` |
| `get_visible_map_chunks` | `getVisibleMapChunks` |
| `get_visible_objects` | `getVisibleObjects` |
| `get_object_view` | `getObjectView` |
| `get_my_champions` | `getMyChampions` |
| `get_champion_view` | `getChampionView` |
| `preview_champion_progression` | `previewChampionProgression` |
| `select_champion_level_up` | `selectChampionLevelUp` |
| `learn_champion_spell` | `learnChampionSpell` |
| `cast_adventure_spell` | `castAdventureSpell` |
| `get_tavern_offers` | `getTavernOffers` |
| `preview_hire_champion` | `previewHireChampion` |
| `hire_tavern_champion` | `hireTavernChampion` |
| `preview_market_trade` | `previewMarketTrade` |
| `submit_market_trade` | `submitMarketTrade` |
| `get_dwelling_pool` | `getDwellingPool` |
| `preview_dwelling_recruit` | `previewDwellingRecruit` |
| `submit_dwelling_recruit` | `submitDwellingRecruit` |
| `get_objective_progress` | `getObjectiveProgress` |
| `get_scenario_rules` | `getScenarioRules` |
| `get_world_events` | `getWorldEvents` |
| `preview_quest` | `previewQuest` |
| `accept_quest` | `acceptQuest` |
| `claim_quest_reward` | `claimQuestReward` |
| `sync_objectives` | `syncObjectives` |
| `sync_world_events` | `syncWorldEvents` |
| `sync_advanced_victory` | `syncAdvancedVictory` |
| `get_skirmish_settings` | `getSkirmishSettings` |
| `get_procedural_map_state` | `getProceduralMapState` |
| `get_naval_routes` | `getNavalRoutes` |
| `get_siege_rules` | `getSiegeRules` |
| `sync_world_generation` | `syncWorldGeneration` |
| `get_town_view` | `getTownView` |
| `get_battle_state` | `getBattleState` |
| `get_content_manifest` | `getContentManifest` |
| `get_events_after` | `getEventsAfter` |
| `get_command_status` | `getCommandStatus` |
| `get_command_status_by_nonce` | `getCommandStatusByNonce` |
| `preview_move_path` | `previewMovePath` |
| `preview_build_town_structure` | `previewBuildTownStructure` |
| `preview_recruit_units` | `previewRecruitUnits` |
| `submit_move_intent` | `submitMoveIntent` |
| `end_turn` | `endTurn` |
| `sync_session_turn` | `syncSessionTurn` |
| `submit_build_town_structure` | `submitBuildTownStructure` |
| `submit_recruit_units` | `submitRecruitUnits` |
| `sync_battle` | `syncBattle` |
| `end_battle_turn` | `endBattleTurn` |
| `submit_battle_action` | `submitBattleAction` |

This interface is normative at the method-boundary level: every required
gameplay endpoint from `docs/canister-endpoints.md` must be represented, even
if a Phase 1 UI does not call it yet. Deferred endpoints remain absent or are
represented only as disabled affordances.

Implementation details:

- `ports/canister/actor.ts` creates the generated Candid actor.
- `ports/canister/client.ts` wraps every endpoint in typed functions.
- `ports/canister/dtoAdapters.ts` converts DTOs into kernel actions.
- `kernel/api.ts` defines RTK Query endpoints with custom actor `queryFn`s.
- `kernel/listeners.ts` starts endpoint calls by dispatching
  `api.endpoints.someEndpoint.initiate(args)`, not by importing the actor.
- Tests use `test/fakeCanister.ts`, not the real actor.

RTK Query endpoint rules:

- One API slice for the canister keeps tag invalidation coherent.
- Endpoint names match canister method names unless there is a strong reason to
  adapt them.
- Query args include `session_id` and viewport/cursor/audience values needed to
  make cache keys unambiguous.
- Mutations wrap update calls but do not perform optimistic domain mutation.
- `keepUnusedDataFor` should be short for gameplay pages; domain slices retain
  the playable projection.
- Use polling only for setup progress, command status, battle deadline windows,
  and event feeds. Do not poll every endpoint.
- Listener workflows adapt fulfilled endpoint data into domain slices.
- Components do not use RTK Query hooks for gameplay screens; they dispatch
  semantic intents and read domain selectors.

### RTK Query Cache And Cancellation Contract

Gameplay listeners must not rely on RTK Query's raw cache as freshness proof.
Every gameplay refresh affected by a command, sync, turn change, visibility
change, or viewport change must either pass `forceRefetch: true` or include a
freshness key in the endpoint args.

Canonical query keys must be explicit:

```text
viewport pages:
  session_id, participant_id, viewport rect, cursor, limit,
  content_manifest_hash, current_turn or visibility revision

previews:
  session_id, subject id, input hash, current_turn, content_manifest_hash,
  source_view_revision

command status:
  session_id, command_id or { command_type, client_nonce }

events:
  session_id, audience_key, events_after_seq, limit
```

Listener-dispatched one-shot `api.endpoints.x.initiate(args)` calls must use
`subscribe: false` or call `unsubscribe()` in `finally`. Long-lived polling
subscriptions must be owned by a workflow scope and cleaned up when the
session, battle, viewport, or command leaves that scope.

Abort is best-effort. If the generated actor cannot cancel an in-flight
canister call, cancellation still means the response is ignored unless its
request token matches the latest token for that workflow scope.

The port must preserve these backend contracts:

- Public ids are strings.
- Movement paths use `MoveCoord { x, y }`.
- `get_game_view` is shell metadata, not a full render state.
- Event audiences are `public` or `participant:{participant_id}`.
- `get_command_status_by_nonce` requires explicit `command_type`.
- `sync_session_turn` is called when `render_time.sync_required` is true.
- `sync_battle` handles battle timeout, recovery, and resolved-battle aftermath.

## Command Kernel

Every user intent creates one local command record before the canister call:

```text
local_id
command_type
client_nonce
payload_summary
created_at_ms
status: idle | submitting | submitted | applying | applied | failed | needs_sync
command_id: optional
retryable
error
changed_subjects
event_seq_range
```

Nonce rules:

- Generate a fresh nonce for each new user intent.
- Reuse the same nonce only for retrying the exact same typed payload.
- If the canister returns `duplicate_nonce_payload_mismatch`, discard the local
  intent and require a new user action.
- If the command response includes `command_id`, poll by command id.
- If only `{ command_type, client_nonce }` is known, poll
  `get_command_status_by_nonce(session_id, command_type, client_nonce)`.
- Do not depend on nonce text inference.

Update flow:

```text
intent action
  -> commandQueued
  -> canister call through listener middleware
  -> commandResponseReceived
  -> apply returned events/changed_subjects
  -> refresh affected query domains
  -> poll status only if response is retryable/applying/pending
```

Some errors happen before command creation. For example, late map-turn commands
can return `backend_work_pending` before a durable status row exists. The kernel
must not wait forever for command status in that case. It should sync/refresh
and mark the local command as `needs_sync` or `failed_precommand`.

Command records are append-only until terminal state. If the UI needs to show a
compact command list, derive it through selectors. Do not delete pending command
records just because the user navigated away; command status and sync recovery
need them.

Command local ids are separate from canister `command_id`:

```text
local_id: generated by client for UI tracking
client_nonce: sent to canister for idempotency
command_id: returned by canister after command creation
payload_hash: returned by canister and used to prove exact replay
```

The command kernel should expose these semantic intents:

```text
registerPlayerRequested
createSessionRequested
joinSessionRequested
markReadyRequested
startSessionRequested
moveIntentRequested
endTurnRequested
buildTownStructureRequested
recruitUnitsRequested
hireChampionRequested
marketTradeRequested
dwellingRecruitRequested
championProgressionRequested
battleActionRequested
endBattleTurnRequested
syncSessionTurnRequested
syncBattleRequested
```

Each intent maps to one workflow. Workflows may call many queries after the
update, but they must produce one visible command record for the user's intent.

### Command Status Adaptation

Backend `CommandStatus` values:

```text
Pending
Applying
Applied
Failed
Cancelled
Superseded
AppliedNoop
```

Backend `CommandPhase` values:

```text
Created
Validated
Applying
EffectsApplied
EventsApplied
Recovered
Complete
Failed
```

Client polling continues only for backend `Pending` or `Applying`. Terminal
backend statuses map into UI `applied`, `failed`, `abandoned`, or
`applied_noop`.

A canister update can return `Ok(CommandResponse)` with `status = Failed` and
`error != null`; treat that as an authoritative command failure, not a
transport failure.

## Query And Refresh Kernel

Do not bind every component to ad hoc queries. Use domain refresh tasks:

```text
refreshSessionShell(session_id)
refreshViewport(session_id, viewport)
refreshSelectedChampion(session_id, champion_id)
refreshTown(session_id, town_id)
refreshBattle(session_id, battle_id)
refreshEvents(session_id)
refreshAfterCommand(command_response)
```

### Active Render Load Sequence

After `get_session.state == "active"`:

1. Call `get_my_participant(session_id)` and store `participant_id`.
2. Call `get_game_view(session_id, request)` only for session/participant
   summary, render time, content hash, opening events, and omitted-field hints.
3. Independently page `get_visible_map_chunks(session_id, viewport, cursor,
   limit)` until `has_more == false`.
4. Independently page `get_visible_objects(session_id, viewport, cursor, limit)`
   until `has_more == false`.
5. Call `get_my_champions`; call detail endpoints only for selected or visible
   actionable subjects.
6. Load `get_events_after` separately for `public` and
   `participant:{participant_id}`.

Do not treat `GameView.map_chunks`, `GameView.objects`, or their `page_info`
fields as the source of map pagination on the current canister. The current
canister `get_game_view` is a shell.

`refreshAfterCommand` uses `changed_subjects` and event types to schedule
bounded follow-up reads. Examples:

```text
champion changed -> get_champion_view, get_my_champions, visible objects
town changed -> get_town_view, visible objects
battle changed -> get_battle_state
session changed -> get_session, get_game_view
object changed -> get_object_view or visible object page refresh
events returned -> advance event cursor and render feed
```

This mapping lives in `kernel/refreshGraph.ts`, not inside React components.

Refresh graph inputs:

```text
changed_subjects
ApiEventView.event_type
CommandResult variant
current screen
selected subject
active viewport
active battle id
```

Refresh graph outputs:

```text
endpoint calls to run
domain slices expected to change
whether to preserve selection
whether to recenter camera
whether to show result/history
```

Use coarse invalidation first, then narrow reads:

```text
session changed -> get_session + get_game_view
participant/resources changed -> get_my_participant + get_game_view
champion changed -> get_champion_view selected ids + get_my_champions
object changed -> refresh visible object page for active viewport
town changed -> get_town_view if selected/visible
battle changed -> get_battle_state active battle
history changed -> get_match_history
```

Viewport reads are paged. The kernel must keep per-viewport request tokens so a
slow response for an old viewport cannot overwrite newer visible data.

### Viewport Page Reconciliation

Viewport loads are staged under a `viewportRequestId`. Chunks and objects from
paged responses are first collected into a staging record keyed by:

```text
session_id:participant_id:viewportKey:requestId
```

Only the latest request id for a viewport may commit to domain slices. Object
pages commit as a complete viewport membership set when `has_more == false`.
Committing replaces `visibleObjectKeysByViewport[viewportKey]`.

Objects no longer present in any committed visible viewport must be marked
not-visible, last-known, or evicted according to their DTO visibility/redaction
model. A `not_visible` detail response clears forbidden detail fields and
preserves only allowed last-known/redacted data. Hidden information must never
survive because an older page or detail response completed late.

Event reads:

```text
public feed:
  get_events_after(session_id, "public", public_cursor, limit)

player feed:
  get_events_after(session_id, `participant:${participant_id}`, private_cursor, limit)
```

The private player feed includes public plus private rows, so event de-duplication
uses `(audience_key, event_key)` or stable event identity, not only `event_seq`.

### Pagination Rules

Map/object cursors are live numeric offsets, not snapshot tokens. Keep page
state keyed by:

```text
session_id, participant_id, viewport, turn/render revision, endpoint
```

Drop old page responses when the viewport or revision changes.

Events keep separate cursors per audience:

```text
public_cursor
participant_cursor for participant:{my_participant_id}
```

The participant feed includes public plus private rows. De-duplicate by
`audience_key:event_key` or another stable event identity, not by `event_seq`
alone. Unauthorized participant audience reads are client bugs and return
`audience_not_allowed`.

## Workflow Engine

Use Redux Toolkit listener middleware for orchestration. Listeners react to
semantic actions and selected state, then dispatch RTK Query endpoint initiates
or domain actions.

Workflow rules:

- Every workflow starts from a semantic action, not a component lifecycle hook.
- Every workflow reads current state through selectors.
- Every workflow is idempotent or guarded by a request token.
- Workflows may cancel older work for the same viewport, selected battle, or
  command local id.
- Workflows may fork short polling loops, but every loop has a stop condition.
- Workflows convert endpoint errors into typed kernel errors.

Core workflows:

```text
lobbyWorkflow:
  register, create, join, ready, start once, setup polling

viewportWorkflow:
  load shell, chunks, objects, champions, public/private events, prefetch nearby viewports

commandWorkflow:
  queue command, submit, process response, poll status, refresh graph

syncWorkflow:
  run sync_session_turn when render_time.sync_required or backend-work state says due

battleWorkflow:
  load battle, submit legal action, handle battle_processing, sync resolved aftermath

resultWorkflow:
  detect finished session, refresh history/final views, block gameplay intents

rulesWorkflow:
  rebuild local affordance snapshots after manifest, viewport, champion, town,
  battle, preview, or command-state changes
```

The workflow engine is also where telemetry belongs. Record endpoint duration,
failed code, retry count, and stale-response drops, but keep telemetry writes
out of reducers and Pixi code.

### Setup Workflow

Call `start_session(session_id, client_nonce)` once for the user intent. A
successful response may contain a `SessionView` with `state = "starting"` or
`state = "active"`.

While not active:

- Poll `get_session(session_id)`.
- Poll `get_setup_progress(session_id)`.
- Show `completed_effect_count / total_effect_count`, `next_effect_key`,
  `setup_command_status`, and `setup_job_status`.
- Accept setup job/status values such as `scheduled`, `running`, `completed`,
  and `runtime_timer`.

Only retry `start_session` with the same nonce for the same start intent. Do
not issue fresh start nonces to advance setup.

### Sync Loops

`sync_session_turn`:

- Call with a fresh nonce when `render_time.sync_required`, the displayed turn
  deadline has passed, or `backend_work_pending` says turn closure is in
  progress.
- Apply returned events and changed subjects, then refresh session, game shell,
  viewport, champions, objects, events, and command status.
- Repeat with fresh sync nonces until `get_session.state`,
  `get_game_view.session.current_turn`, and
  `get_game_view.render_time.sync_required` show a stable turn.
- If events include `neutral_encounter_pending`, `champion_encounter_pending`,
  or `town_encounter_pending`, stop strategic movement assumptions and
  open/load the battle.

`sync_battle`:

- Call after `battle_processing`, after battle deadline expiry, when
  `BattleSyncOutcome.battle_sync_incomplete == true`, or when
  `get_battle_state.state == "resolved"`.
- Retry the same nonce only for transport retry of the same sync call; use a
  fresh nonce for each new sync attempt.
- Stop when `battle_sync_incomplete == false`, no retryable battle error
  remains, and refreshed battle/session/history state is stable.

## Render Architecture

React mounts one Pixi canvas:

```text
<GameScreen>
  <PixiRoot />
  <HudOverlay />
  <SidePanels />
  <ModalStack />
</GameScreen>
```

`PixiRoot` creates the Pixi application and a `RendererBridge`.

The bridge subscribes to a memoized `selectRenderFrame` selector:

```ts
type RenderFrame = {
  camera: CameraView;
  terrainChunks: TerrainChunkRenderModel[];
  fogChunks: FogChunkRenderModel[];
  objects: ObjectRenderModel[];
  champions: ChampionRenderModel[];
  movementPreview: MovementPreviewRenderModel | null;
  battle: BattleRenderModel | null;
  overlays: OverlayRenderModel[];
};
```

The bridge diffs `RenderFrame` into Pixi containers. It does not own game
truth. It owns textures, sprites, containers, local animation state, and pointer
hit testing.

Layer order:

```text
terrain
roads/decals
fog/discovery
world objects
towns/mines/resources
champions
movement path preview
selection/hover
range/vision overlays
battle scene if active
debug overlays
```

### Fog And Redaction Rendering

Object rendering must be visibility-gated from `ObjectView`:

```text
visible -> live marker and allowed live details
last_known -> ghost/redacted marker with last_seen_turn
discovered terrain -> static terrain/movement/flags only
hidden -> no marker and no detail panel
```

Static terrain bytes may render for surveyed map chunks, but they must not
imply current occupants, owners, battles, or event payloads. Detail panels open
only after the relevant detail endpoint succeeds. A `not_visible` response
clears forbidden detail without teaching the player whether the hidden subject
exists.

Rules:

- Use Pixi for map/battle visuals, not DOM nodes.
- Use DOM for text-heavy panels and form controls.
- Text labels inside Pixi must be sparse and performance-budgeted.
- Keep asset keys from the content manifest; do not hard-code sprite filenames
  in domain reducers.
- Renderer may interpolate movement locally for animation, but final positions
  come from kernel state.
- Pointer movement and drag velocity stay local to the renderer. Selection,
  target choice, command intent, and panel state go through Redux actions.

### Selector And RenderFrame Identity Contract

Do not build one monolithic selector that reallocates the full render frame on
every store action. Use selector factories scoped by session, viewport, battle,
and selected subject.

`selectRenderFrame` must return the same object reference for unrelated state
changes. Render models are partitioned by layer and carry stable ids plus
revision/hash fields. The renderer subscribes to the store, compares previous
and next frame/layer revisions, and coalesces multiple store updates into one
`requestAnimationFrame` reconciliation.

### Pixi Reconciliation Rules

Every render item has a stable `render_id`, `asset_key`, position, z/layer, and
revision. Each layer owns `Map<render_id, DisplayObject>`.

For each committed frame:

- Create missing display objects from pools.
- Update only objects whose revision or transform changed.
- Remove or pool objects absent from the new layer model.
- Update terrain by chunk key/blob hash, not by full-map redraw.
- Avoid clearing and rebuilding whole `Graphics` layers for hover, selection,
  or command-status changes.
- Keep camera pan/zoom as container transforms unless tile membership changes.

## Camera And Coordinates

Use explicit coordinate transforms:

```text
tile:   { x: u16, y: u16 }
world:  pixel coordinate in map space
screen: CSS pixel coordinate in viewport
```

One module owns conversions:

```text
tileToWorld(tile)
worldToTile(world)
worldToScreen(world, camera)
screenToWorld(screen, camera)
screenToTile(screen, camera)
```

Store persistent camera intent in Redux:

```text
center_tile
zoom_level
mode: map | battle
follow_champion_id
```

Keep frame-by-frame easing and pointer drag state out of Redux.

## Map Interaction

Map interactions dispatch semantic actions:

```text
tileHovered(tile)
tileClicked(tile)
subjectClicked(subject_key)
championSelected(champion_id)
movementPathEdited(champion_id, path)
movementPreviewRequested(champion_id, path)
movementSubmitRequested(champion_id, path)
endTurnRequested()
```

The path planner can compute a local candidate path for responsiveness, but the
UI must call `preview_move_path` before showing the path as legal. If preview
fails or returns a disabled reason, keep the local path as a visual draft only.

Map UX rules:

- Selecting an owned champion focuses reachable local terrain immediately.
- Dragging or clicking a destination draws a draft path without waiting for the
  network.
- While `preview_move_path` is pending, the path is visually marked as checking.
- Once preview succeeds, show cost, remaining movement, blockers/stops, and an
  enabled submit affordance.
- If preview fails, keep the path visible with the canister reason and a clear
  edit/cancel action.
- If turn state changes while the player edits a path, invalidate the preview
  and keep the draft in `needs_preview` rather than submitting stale data.
- Camera movement should prefetch the adjacent viewport band after the current
  viewport settles.

## Movement Intent UX

Submitting movement creates or replaces the champion's movement intent for the
current turn. It does not move the authoritative champion position. The UI
stores `planned_movement_intents_by_champion` with:

```text
champion_id
turn_number
path
preview
client_nonce
command_local_id
command_id if known
status
replaced_intent_id if known
source_view_revision
```

After submit succeeds, render the planned path as submitted and keep the
champion on its last confirmed tile. The player may replace the intent while
the same turn remains open, including after calling `end_turn`.

When turn closure or sync resolves movement, animate only from refreshed
authoritative movement events/views. If the refreshed champion position differs
from the planned path because simultaneous movement, blockers, battle contact,
or object interaction changed the result, show the resolved path/event outcome
instead of trying to force the draft path.

## Battle Interaction

Battle render uses the same Pixi app with a different scene root:

```text
battle grid
obstacles
stacks
initiative strip marker
legal target highlights
movement path
damage preview
deadline timer
event marks
```

Controls come from `BattleView.legal_actions_for_caller`.

Allowed action strings currently include:

```text
Move
MeleeAttack
RangedAttack
CastAbility
Defend
Wait
```

Retreat and surrender may appear only as disabled/deferred affordances. Do not
show them as enabled commands.

Battle UX rules:

- The active stack should be visually obvious within one frame of battle load.
- Legal actions should be grouped into command buttons with target/path
  highlights from `legal_actions_for_caller`.
- Hovering a legal target should show the server-provided damage preview when
  `damage_preview.target_stack_id` matches the hovered target; otherwise show
  the legal highlight without invented damage.
- Disabled battle actions should show disabled reasons from legal-action
  metadata, not guessed copy.
- When the battle deadline is near, show the timer and any pending
  `sync_battle`/auto-defend state without freezing input.
- On resolved battle state, transition to aftermath syncing with a clear
  "resolving battle" status instead of leaving the player on a dead tactical
  board.

Battle ids are discovered from command responses, changed subjects, and
encounter/battle events. The battle workflow records pending battle ids,
notifies the player, auto-opens the battle when it is the caller's only
actionable battle, and supports switching between active battles.

Legal actions are rendered directly from `legal_actions_for_caller`. Do not
synthesize legal targets. `damage_preview` may be absent or may describe only
one target.

When `get_battle_state.state == "resolved"`, the battle driver calls
`sync_battle(session_id, battle_id, fresh_nonce)` until aftermath is applied or
the response is a stable no-op, then refreshes session, map/object/champion,
town, events, and match history.

## Town And Champion Panels

Town panels are React DOM. They use:

```text
get_town_view
preview_build_town_structure
submit_build_town_structure
preview_recruit_units
submit_recruit_units
get_tavern_offers
preview_hire_champion
hire_tavern_champion
preview_market_trade
submit_market_trade
```

Champion panels are React DOM. They use:

```text
get_champion_view
preview_champion_progression
select_champion_level_up
learn_champion_spell
cast_adventure_spell
```

Panel controls are enabled from preview responses or explicit legal-action
metadata. Do not infer affordability, unit compatibility, or legal target slots
only from local content definitions.

Town panels use a preview lifecycle per selected building, unit, quantity, and
target. Required controls:

- Building grid with preview state per selected building.
- Recruit quantity stepper bounded by preview/pool feedback.
- Recruitment target selector for `TownGarrison` and eligible same-tile
  `Champion` targets.
- Optional `slot_index` selector when the preview says slot choice matters.
- External dwelling recruitment controls driven by `get_dwelling_pool`,
  `preview_dwelling_recruit`, and `submit_dwelling_recruit`.

Building controls must not enforce a local one-building-per-turn rule unless
the preview response returns that disabled reason. The current canister generic
build path prevents duplicate building slugs and missing prerequisites, but
does not enforce a hard generic one-building-per-town-per-turn cooldown.

Champion panels should show learned spells and progression choices from
`get_champion_view` and `preview_champion_progression`; they must not assume a
spell can be cast until the relevant adventure or battle endpoint confirms it.

## Result And Gameover Workflow

Finished sessions are detected from `get_session`/`get_game_view` session
state, `victory_finalized` events, and `get_match_history` summaries. Because
`SessionView` is a shell, winner and finish reason live in the result slice,
derived from event payloads or match-history `summary_json`.

Once finished:

- Block new gameplay commands in the UI.
- Allow read-only refresh, history, and final detail queries.
- Treat `session_not_active` from gameplay updates as a closed-match state, not
  a fatal application error.
- Keep final map, town, champion, battle, event, and history panels available
  for inspection.

## Time Model

The client never sends `now_ms`.

Store server time metadata from `GameView.render_time`:

```text
server_now_ms
turn_started_at_ms
turn_duration_ms
sync_required
```

Compute a local clock offset for display only:

```text
clock_offset = server_now_ms - local_now_ms_at_response
display_now = local_now_ms + clock_offset
```

Countdown widgets may tick locally. They must not mutate game state. When
`sync_required` is true or the displayed deadline is crossed, schedule
`sync_session_turn` and refresh.

Battle timers use `action_deadline_at` and `remaining_ms` from `BattleView`.
When the deadline is due, schedule `sync_battle` or refresh battle state.

## Error Model

Represent endpoint failures with a typed UI error:

```text
kind: auth | validation | retryable | visibility | not_found | backend_work | unknown
code
message
retryable
details_json
command_local_id
```

Common handling:

```text
not_authenticated -> login/identity state
not_participant -> leave session view or show access error
session_not_active -> switch to result/history refresh if session is finished
turn_expired -> refresh session/game shell and require a new intent
not_visible -> clear forbidden detail panel, keep map redacted
backend_work_pending -> sync/refresh, no status wait unless command_id exists
recovery_budget_exhausted -> show syncing state and retry bounded recovery
battle_processing -> sync battle and refresh
battle_sync_incomplete -> keep battle aftermath/timeout sync visible and retry with fresh nonce
battle_not_visible -> close tactical detail and keep only redacted public state
not_active_stack -> refresh battle; disable stale stack controls
stale_battle_round -> refresh battle; rebuild legal actions
turn_not_due -> show countdown, no retry loop
duplicate_nonce_payload_mismatch -> abandon local intent
payload_too_large -> fail local action and keep user state
command_status_not_found -> stop polling after bounded attempts
path_blocked/path_too_long -> keep draft path and show editable movement error
insufficient_resources -> refresh participant/town and show affordability error
recruit_target_full/unit_stack_incompatible -> keep recruit panel open with target error
already_built/building_not_found/unit_not_found -> refresh town/content and show preview error
value_cap_exceeded -> clamp input and show cap explanation
```

Errors from hidden objects should not teach the client that the object does not
exist. Preserve the backend redaction semantics in UI copy and state.

Domain-specific error mappers should live beside feature workflows:

```text
movementErrors.ts
townErrors.ts
battleErrors.ts
visibilityErrors.ts
syncErrors.ts
```

Copy should name the player action and recovery path, not backend internals. For
example, `backend_work_pending` becomes "Turn is resolving, refreshing..." and
`battle_processing` becomes "Battle is catching up, resolving actions...".

## Persistence

Local persistence is convenience only:

```text
identity/session bookmarks
last selected ruleset
graphics/audio settings
camera preference
last joined session id
```

Do not persist authoritative game state as truth. On reload, rehydrate from the
canister through the normal startup/render composition flow.

## Accessibility And Input

Canvas content needs DOM affordances:

- Keyboard shortcuts for selection, end turn, wait/defend, cancel, confirm.
- DOM panels for selected tile, selected champion, selected town, and selected
  battle stack.
- Event feed as real DOM text.
- Buttons with icons and labels/tooltips for commands.
- Reduced-motion option for movement/battle animations.
- Color-blind-safe ownership and path colors.

The first canvas implementation must support mouse. Keyboard navigation and
touch should be designed into the state model, not bolted onto Pixi internals.

## Performance Rules

Initial targets:

```text
desktop map pan/zoom: 60 fps on normal route
mobile/tablet map pan: no blank frames
hover/selection response: under 16 ms from cached state
local path draft update: under 32 ms for normal first-playable paths
panel open from cached entity: under 50 ms
command response UI: visible pending state within 100 ms
viewport refresh: cancel/ignore stale page responses
canvas startup: visible loading/progress before large asset work
```

Rules:

- No Redux dispatch from the animation ticker.
- No full map redraw from React render.
- Diff render models into Pixi containers.
- Pool sprites for common object/champion markers.
- Batch terrain/chunk rendering where practical.
- Throttle viewport fetches during camera drag.
- Request details only for selected or visible actionable subjects.
- Keep large debug overlays off by default.
- Use memoized selectors with stable array/object identities for render models.
- Recompute rules affordances only for changed domains, not for every action.
- Move pathing/range overlay work to a worker when it exceeds the synchronous
  budget.
- Preload textures referenced by the current content manifest before showing
  the first active map.
- Prefetch one viewport band around the current camera after pan settles, but
  never let prefetch block visible viewport requests.
- Use request tokens to drop old viewport, preview, and battle responses.
- Keep command/status polling bounded and back off after terminal or
  pre-command failures.
- Keep event feed paging incremental; do not rebuild the whole event log on
  every refresh.

Performance anti-patterns:

```text
component calls endpoint -> set local state -> renderer reads local state
Pixi pointermove dispatches Redux actions every frame
selectors allocate a brand-new full RenderFrame for unrelated command status
RTK Query raw cache is treated as the render model
pathfinding runs on the main thread for every pointer movement
preview failure clears the player's draft path without explanation
camera pan waits for canister page responses before moving
```

The kernel should degrade gracefully. If the network is slow, the player can
still pan, inspect cached visible state, edit draft paths, read event history,
and understand which commands are waiting on the canister.

## Testing Strategy

Unit tests:

```text
dtoAdapters convert generated Candid DTOs into domain models
dtoAdapters strip Principal/bigint/class instances before reducer actions
reducers preserve invariants and never mutate server truth optimistically
selectors produce stable render frames
rules planners produce affordances from manifest/current visible state/previews
rules planners invalidate previews when turn, content hash, or subject revision changes
command kernel handles replay, nonce mismatch, pre-command failure, and sync
event reducer de-duplicates public/private feeds
viewport reducer ignores stale page responses
refreshGraph maps changed_subjects/events to bounded endpoint calls
listener workflows stop polling on terminal states and cancellation
RTK Query endpoint wrappers call the port, not generated actors directly
listener-dispatched RTK Query calls unsubscribe or use subscribe: false
selector reference identity is stable for unrelated command/status updates
object detail not_visible clears privileged detail state
combined event feed de-duplicates by stable event identity and prefers unredacted rows when available
```

Integration tests with fake canister:

```text
startup reaches active session from one start_session call plus setup polling
map render composition pages chunks/objects and applies event cursors
movement preview -> submit -> end_turn -> sync -> refresh updates champion
movement draft remains editable while preview/network is pending
canceled preview response cannot enable submit for a stale path
delayed fake canister returns old viewport pages after a newer viewport commits
town build/recruit previews control enabled/disabled UI
battle action -> resolved state -> sync_battle aftermath -> result refresh
slow fake canister does not block pan/zoom/selection from cached state
gameover switches to result/history and blocks gameplay commands
```

### Playwright Verification Contract

Playwright is required for every browser-facing UI slice. Unit/component tests
can prove state logic, but they do not prove that the game is actually
playable in a browser.

Every meaningful UI implementation checkpoint must run Playwright against the
real app shell and inspect both the DOM and the canvas:

- Capture screenshots for desktop and mobile viewports and keep them as test
  artifacts when a visual or layout failure occurs.
- Use locators to inspect DOM structure, visible text, enabled/disabled button
  states, ARIA names where present, focus behavior, modal/panel visibility, and
  command lifecycle labels.
- Check that critical controls do not overlap, clip, or render outside their
  containers at mobile and desktop widths.
- Check that panels and HUD elements update from Redux/kernel state after
  canister/fake-canister responses rather than only after local component
  state changes.
- Inspect console errors, failed network requests, and uncaught page errors;
  no UI checkpoint passes with unexplained browser errors.
- For Pixi/canvas surfaces, use screenshot and canvas-pixel checks to prove the
  map/battle scene is nonblank, framed correctly, and changes after camera or
  gameplay interactions.
- Prefer semantic locators for DOM controls and stable `data-testid` values for
  canvas-root, layer/debug counters, and difficult-to-name game surfaces.

Browser tests with Playwright:

```text
desktop map canvas is nonblank and framed
mobile map canvas is nonblank and controls do not overlap
DOM panels expose selected tile/champion/town/battle state through locators
buttons expose checking/submitting/applying/applied/failed/needs-sync states
dialogs, side panels, and bottom bars can be opened/closed and inspected
camera pan changes visible tiles
selecting a champion shows path overlay and DOM panel
movement preview checking/success/failure states are visibly distinct
selecting a champion does not recreate terrain chunk display objects
pan/zoom updates camera transforms without full layer rebuild
battle grid renders stacks, highlights legal actions, and reacts to clicks
resolved battle refreshes map/result state
text in controls fits at mobile and desktop widths
```

Use screenshot and canvas-pixel checks for the first real renderer. A passing
React test that never verifies canvas pixels is not enough. A passing canvas
test that never inspects DOM controls and command-state text is also not
enough.

Contract tests:

```text
generated Candid types compile into the web client
fake canister fixtures cover every DTO the UI consumes
recorded Gate M route can be replayed against the client kernel
optional local/PocketIC smoke can drive the real actor after scaffold exists
```

## Implementation Phases

Phase 1: scaffold.

```text
apps/web Vite + React + TypeScript
Redux Toolkit store
domain projection slices
listener workflow middleware
fake canister port
visual reference index for mock images/design direction
Pixi canvas root with nonblank map placeholder
Playwright DOM inspection plus screenshot/pixel smoke
```

Phase 2: generated client and DTO adapters.

```text
import generated Candid declarations
wrap actor in DommCanisterPort
represent every required gameplay endpoint from docs/canister-endpoints.md
define RTK Query API slice with custom actor query functions
wire listener middleware to endpoint initiates
adapter tests for GameView, MapChunkPage, ObjectViewPage, ChampionView,
ApiTownView, BattleView, ApiEventPage, CommandResponse
```

Phase 3: lobby/setup/session shell.

```text
register/get player
create/join/ready/start
single start_session call
setup progress polling
active session transition
```

Phase 4: map render composition.

```text
get_game_view shell
map chunk pages
visible object pages
my champions
event feeds
camera and selection
rules affordance model for selected champion/tile
viewport staging/reconciliation and stale-response drops
```

Phase 5: map commands and towns.

```text
movement preview/submit
end_turn/sync_session_turn
town build/recruit
tavern/market/dwelling panels
command status polling
slow-network UX states for checking/submitting/applying/needs-sync
first-match suggested move/build/recruit checklist
```

Phase 6: battle.

```text
get_battle_state render
legal action controls
submit_battle_action
end_battle_turn
sync_battle timeout/recovery/aftermath
```

Phase 7: result/history.

```text
gameover detection
match history
final detail views
rematch disabled/deferred affordance
```

Phase 8: hardening.

```text
mobile layout
keyboard shortcuts
asset atlas
error states
offline/retry behavior
rules worker for expensive pathing/overlays if needed
performance budgets
visual regression suite
```

## Acceptance Checklist For First UI Cut

The first usable UI is not done until:

- It uses generated Candid through a port, not raw ad hoc calls.
- It routes gameplay endpoint traffic through the RTK Query API slice and
  listener workflows, not component-level actor calls.
- It has a visual reference index or design README that records the mock
  images/design references used for style and layout direction.
- It calls `start_session` once and polls setup progress.
- It composes map state from dedicated endpoints, not only `get_game_view`.
- It renders a nonblank Pixi map canvas.
- It can select a champion, preview a path, submit movement, end turn, sync,
  and show the champion at the updated position.
- It shows fast local movement drafts and only enables submit after preview
  confirmation for the current turn/champion snapshot.
- It can open a town panel and run at least one preview/submit city action.
- It gives first-match suggested move, build, and recruit actions without
  blocking free play.
- It can open a battle, show legal actions, submit one action, sync aftermath,
  and refresh map/result state.
- It handles command replay and pre-command backend-work errors.
- It keeps generated Candid DTOs, `bigint`, `Principal`, and Pixi objects out
  of reducers.
- It remains responsive for pan/zoom/selection while viewport, preview, command,
  or battle sync requests are in flight.
- It has unit tests for adapters/kernel and Playwright checks that inspect DOM
  controls, capture screenshots, and verify canvas pixels.

## External References

- PixiJS docs: https://pixijs.com/8.x/guides
- Redux Toolkit docs: https://redux-toolkit.js.org/
- RTK Query overview: https://redux-toolkit.js.org/rtk-query/overview
- RTK Query cache behavior: https://redux-toolkit.js.org/rtk-query/usage/cache-behavior
- Redux Toolkit listener middleware: https://redux-toolkit.js.org/api/createListenerMiddleware
- Redux Toolkit entity adapter: https://redux-toolkit.js.org/api/createEntityAdapter
- D3 docs: https://d3js.org/what-is-d3
