# The audit against the rings checkpoint

**2026-09-15** — with the client moved into the kernel and the heap work on
`wip/kernel-heap` stopped, a read-only audit went over the Rust against the
last checkpoint, `checkpoint/20260915-rings`: every pair of a checkpoint
TypeScript file under `apps/web/src` with its `crates/pxl/core` file, the
xpute and shared crates' deltas, and the pre-existing core crate's diffs.
Static reading only; nothing was run. Line numbers are as of `eeae374`, the
Rust side first and the checkpoint's TypeScript second. An item is struck
when its fix lands.

## P. Broken

- ~~**P1**~~ Placed objects are never saved. `object/store.rs:917-939`: `save`
  only sets `save_due_ms`; nothing calls `flush_save_if_due` or
  `flush_save`, and nothing flushes when the page hides. The checkpoint's
  `object/store.ts:843-867` saved 300 ms after a write and flushed on
  `visibilitychange` hidden and `pagehide`.

- ~~**P2**~~ Found on the running client (2026-09-16), not in the reading:
  a pan stopped drawing new ground while the camera still moved. The ledger
  report named it: terrain pages refused with "the page section is full"
  while 14 pages were resident. A response body was a block of the page
  section (`fetch.rs`'s `fetch_body`), 4.2 MB rounded to an 8 MB buddy block,
  up to six in flight, and `RESIDENT_PAGES` counts only a page's lanes; with
  the lanes' 1 MB blocks spread over the section no 8 MB piece was left, the
  page was refused, and its chunks had nothing to build from. In the
  checkpoint a body was the shell's `ArrayBuffer`. Bodies are blocks of the
  mesh section now, a POI body excepted, which a POI page keeps as itself.
  A simulated pan (sixty steps north-east from Seoul) refused 8 pages before
  and none during the pan after.
  Then the jump: a search result leaves every resident page at once, and
  `resident.rs` held all of them for its one-second quarantine while the
  next pages arrived, so a jump from Seoul to Busan refused 21 of them. A
  released page's blocks now go back as soon as the scheduler has no chunk
  of that page mounted (`Scheduler::page_mounted`), the second only an upper
  bound; the same jump refused none. A page refused anyway was asked for
  again only when the view next moved (the wishlist is rebuilt on an anchor
  change, and the scheduler drops a failed demand), so a jump followed by a
  still view left a hole; a failed terrain fetch now rebuilds the wishlist
  once the quarantine has passed.
  Exercised afterwards on the running client, every network path but POI's
  (which stays disconnected, BACKLOG) let out: a pan of eighty steps (terrain
  182 fetched, 0 failed; geocode 15, 0 failed; the module heap's live bytes
  flat at 9.8 MB), the jump from Seoul to Busan (terrain 122, 0 failed), the
  companion's model and skin and the decor kinds', and a search (answered in
  1.8 s; a query superseded or closed sooner is aborted, as designed). No
  trap and no refusal in any of them. The page ran hidden, so what is drawn
  on screen was not seen.

## H. Residue of the heap work

- **H1** `wasm/src/lib.rs:1903-1904` installs `ALLOC_PROBE` and
  `set_step_timing(true)` unconditionally; xpute `sched/tick.rs:1787-1791`
  then records allocation counts into `StepStats.ms`, which `log.rs`
  prints as "ms per frame". `tick.ts:292-304` recorded wall milliseconds,
  in DEV only.
- **H2** `probe.rs` and its `probe::mark` / `add` calls in `world.rs`
  (110-156, 199-202), `core_tick.rs:170-175` and about twenty sites in
  `world/sched.rs`.
- **H3** `pub static mut ALLOC_PROBE` in xpute_core `sched/tick.rs:1646`.
- **H4** ~~`world/decor.rs` `DecorScratch`: past `DECOR_INSTANCES` (65536) an
  instance is dropped silently; past `DECOR_GROUPS` (64) kinds get no
  group.~~ `DECOR_INSTANCES` is the length of two lanes taken once from the
  mesh section and never given back (`decor.rs:164-180`), so raising it is
  spending that section, not editing a number — what was wrong was the
  silence, not the ceiling. `DecorScratch.dropped` counts the instances the
  lanes had no room for, `Scheduler::decor_dropped` carries it into the
  window's counters, and the ledger's `build` line says `DECOR DROPPED n`.
  What happens past `DECOR_GROUPS` (64, the renderer's `GROUPS`) is H11's.
- **H5** ~~`world/sched.rs` asserts `MAX_CHUNKS` (4096, `build.rs:22`) and
  `MAX_PAGES` (1024).~~ How many chunks a range holds is the viewport's and
  the anchor's, not a number the kernel hands itself, so the mount pass stops
  at `MAX_CHUNKS` and leaves the key unrecorded — the shape the frame's mount
  budget already had — and the next pass tries again. The `add` assert stays,
  where reaching it means the caller has a bug rather than that the view
  grew. `MAX_PAGES` (1024) is kept as it is, for a different reason than
  first written: the kernel's own teller mirrors `tile_pages` and tells a
  page gone as well as present (`core_tick.rs:255-260`), so that path stays
  inside `RESIDENT_PAGES` (32 at the cap) — but the shell can also say so
  itself, through `PAGE_CREATE` / `PAGE_DELETE` (`wasm/src/lib.rs:1277`), and
  a thousand pages created and never deleted is that caller being wrong, not
  the view growing. The assert says so, as `Scheduler::add`'s does.
- **H6** ~~`fetch_scheduler.rs` `HELD` (128, `take` truncates), `NAMES` (256)
  and `CATEGORIES` (16) asserted; `demand.rs` `PHASES_MAX` (6) drops
  silently; `asset_store` `VARIANTS` (16) asserted.~~ **Kept as it is**: a
  category reaches the scheduler only as a `&'static str` the code names —
  terrain, ocean, poi, geocode, search — so `CATEGORIES` counts code, and
  `NAMES` is `HELD * 2` because the queue holds at most one name per slot and
  one aborted. `VARIANTS` is asserted against the registry's own bundles,
  whose largest kind has nine. `HELD`'s truncation is the refusal the
  constant already documents: the fetcher's next wishlist asks again, which
  is what terrain's `retry_at` now makes sure of. `PHASES_MAX` leaves out a
  seventh sub-timing, and a run's phases are literals in the code, so it
  costs a line of the report and nothing the world runs on.
- **H7** ~~`ledger.rs:199-209` keeps a one-minute window capped at
  2048 / 4096 / 1024; `ledger.ts:196-198` kept the session
  (200k / 500k / 50k).~~ **Kept as it is**: the shell's rings were the
  browser's heap and could grow with the tab; these are lanes of the fixed
  section, and a session's worth at those counts is the memory this same
  audit spent its turns getting back. What a report answers for is the
  minute before it was taken, and a lane that fills inside that minute says
  so through `truncated` rather than dropping the overflow unannounced.
- **H8** ~~`wasm/src/lib.rs:192-200` names `VOLUME_POOLS`, a command no
  longer in `spec/cmd.rs`.~~ The volumes are cut in `boot`, and the three
  places say so.
- **H9** ~~`frame/range.rs:111-115` `PAGES_IN_RADIUS` is unused, and `Pages`
  is documented as inline while it is a `Vec`.~~ The constant is gone and the
  alias says only what it is.
- **H10** ~~`render/gpu/pipeline.rs:32-33,78,114` `LAYOUTS` (32) and `BUILT`
  (48) asserted; `pipeline.ts` kept maps.~~ **Kept as it is**: a key is a
  pipeline, a format and a sample count, and none of the three is grown by
  what the user does. The twenty-nine `build` calls name fifteen
  `PIPELINE_*`, each always with the one format it draws into — `CANVAS`,
  except `PIPELINE_MIPS`, which `layers.rs:43` builds into `RGBA8_SRGB` at
  one sample — and a sample count that is `4` or `1` as the MSAA setting
  stands (`renderer.rs:923`). That is twenty-nine keys the code can ask for,
  against `BUILT`'s 48, and the layouts are derived from those same records,
  so `LAYOUTS` is fixed by the code in the same way. An entry is never
  evicted, so the tables hold every key the code can ask for at once — and
  only a change to the shader records themselves adds one, which is what an
  assert is for.
- **H11** ~~`badges.rs:38,291` `SETS` (32) stops drawing sets past it;
  `render/gpu/decor.rs:29,93` `GROUPS` (64) gives groups past it no model;
  `skinned.rs:23,77-78` `SKINS` (16) leaves a seventeenth model's actor
  undrawn; `renderer.rs:149,187-193,271` `Blocks` asserts below 16384
  blocks over seven fixed-section lanes never given back, and
  `Scratch::take` (`renderer.rs:132`) expects room in the mesh pool.~~ Three
  of the four are the code's: a billboard set is made in two places
  (`chat_bubble.rs:121`, `poi_layer.rs:246`) and each makes one, `GROUPS` is
  the decor registry's four kinds by their variants, and the blocks the draw
  list names are recorded by one caller (`renderer.rs:943`, the list's upload
  rows) and given back by `remove`. How many there are is the tiers and decor
  steps the live nodes hold — a node keeps one of each per `LodStep` and lets
  a step go on eviction — which is not a number to be derived from
  `MAX_CHUNKS` but the one the ledger's retain line reports; the assert is
  what watches it. `SKINS` was the one that
  could be pressed: nothing tells the pass a model went away, so the table
  never freed a slot and the character preview walks more shipped variants
  (twelve people, twenty-four pets) than it has slots — a seventeenth drew
  untextured for good. A slot now carries when it was last asked for, and a
  new model takes the one asked for longest ago.

## B. Behaviour that differs

- **B1** ~~`fetch_scheduler.rs:160-177` `fetch_settle` records nothing in the
  ledger when the row is gone; `fetch_scheduler.ts:158-177` recorded every
  settle.~~ **Not reachable**: read against xpute's `sched/io.rs`, the row is
  gone in one case only. `sync` removes a row that is QUEUED, and a queued
  demand has never been handed to the shell (`pump` is what starts one), so
  no settle can arrive for it; a RUNNING row it no longer wants keeps its row
  and only takes `abort_sent`, so the shell's settle still finds it and is
  recorded. `IoQueue::reset` is the one call that drops rows in flight, and
  its only caller is `fetch_scheduler_reset`, which says it is the tests'.
- **B2** ~~The fetch pump runs at the end of a turn (`world.rs:164,203`); a
  settle between turns waits for the next. The checkpoint pumped on every
  settle.~~ **Kept as it is**: xpute's `sched/io.rs` states the rule and what
  it was measured against — with a pump per submission the category that
  synced first filled every slot before another had submitted anything, and a
  page at priority 1 sat 358 ms behind pages at priority 2 (the same
  measurement the checkpoint's own `schedule_pump` records, which coalesced
  the pumps of a tick for it). The wait is one frame: the settle arrives as a
  command, and a command asks for the frame that answers it.
- **B3** ~~xpute `run_phase` steps back on a self-unregister;
  `tick.ts:289` spliced and skipped the next step.~~ The Rust is right.
- **B4** ~~The quantum is observed after the turn and its grant applies to
  the next; `tick.ts:329-332` observed before sizing.~~ **Kept as it is**:
  the kernel does not size a quantum — xpute's `Quantum` has no caller here.
  The grant is three fields of the `WORLD_UPDATE` packet
  (`wasm/src/lib.rs:1079`), and a packet is read before the turn it starts,
  so what it carries is the turn before. In the shell both sat in one
  function and could be ordered either way.
- **B5** ~~The frame is drawn after every phase (`world.rs:204-205`);
  `core_tick.tsx:187` drew inside the visible phase.~~ **Kept as it is**: the
  draw list is whole only once every phase has added to it, so a draw inside
  one shows a list the phases after it are still writing.
- **B6** ~~The report step (`core_tick.rs`) and the renderer's census and
  triangle count (`renderer.rs:821-860`) run in every build, every frame;
  `core_tick.tsx:198` and `renderer.ts:406` ran them in DEV.~~ **Kept as it
  is**: in the shell the report was a development tool beside React's own;
  in the kernel it is the only account of a frame anyone gets, and a build
  that leaves it out cannot be asked what it did. What it costs is one pass
  over a draw list already in hand, into a fixed census, with nothing
  allocated.
- **B7** ~~A frame's `task_ms` is the previous turn's (`core_tick.rs:204`,
  `world.rs:206`); `core_tick.tsx:263-267` corrected it in place.~~
  `ledger_last_frame` was written for exactly this and had no caller, so
  `set_host_task_ms` now writes the number into the frame it was measured
  for.
- **B8** ~~The world wakers are revision polling in `world_turn`.~~ Outcome
  equal as read.
- **B9** ~~`log.rs:238-245` sets the baseline at first use; the first report
  prints window counters as 0. `log.ts:124` set it at load.~~ **Kept as it
  is**: the log is touched long before anything asks it for a report —
  `set_host_report_stat` lands every frame the shell draws and
  `ledger_record` on every demand that settles — so the baseline is the
  session's start, and the window is zero only for a report asked for before
  the first frame.
- **B10** ~~`log.rs`'s retain line drops `decor_instances` (`log.ts:250`).~~
  The blocks say how many instances they hold in their own header, so the
  line counts them without the hand-over carrying a number.
- **B11** ~~`category.rs`'s terrain note drops its mobile bottleneck
  sentence.~~ **Kept as it is**: the sentence says bottleneck A hangs off
  `set_tile` inside React's commit and shows up as `chunk_stage`'s
  `commit_ms`. The kernel has no commit and no `chunk_stage`, so the note
  keeps what a reader can still act on.
- **B12** ~~`error_reason`: `_` is a word boundary, and 80 characters are
  counted as chars rather than UTF-16 units.~~ The boundary counts an
  underscore as a word character, and the eighty are counted in the units the
  shell measures a string in, a character never cut in half.
- **B13** ~~`camera/input.rs:116,330-333` pairs the two lowest pointer ids;
  `input.tsx:209,355-358` the two earliest pressed.~~ **Kept as it is**: both
  readers of the pair (`begin_pinch`, `update_pinch`) take it in the same
  order, and the map is keyed by id, so the order cannot flip mid-gesture and
  turn the twist angle around. With two fingers down the two orders name the
  same pair; they differ only about which two of three or more to follow,
  where either is a choice, and the baseline is retaken when a third lifts.
- **B14** ~~`decor/asset_store.rs:95-119` retries a variant only when
  `load_revision` moves, so a model the pool refused waits for another
  load.~~ A refusal is the mesh pool having no room, which nothing else
  reports, so the turn that met one leaves the revision unconsumed and the
  next turn asks again.
- ~~**B15**~~ `use_follow_mode.rs:62-71` sets the grace deadline once;
  `use_follow_mode.ts:67-84` restarted 12 s on each status change.
- ~~**B16**~~ `search_modal.rs:69-119` leaves the last query in flight while
  the next is typed, so its results can show; a failed parse clears the
  results (`:145-147`, the TypeScript kept them); `{:.1}` rounds a tie to
  even (1250 m "1.2km", `toFixed` "1.3km"). `search_modal.tsx:192-195`.
- ~~**B17**~~ `use_reverse_geocode.rs:67-88` submits during a continuous pan;
  `use_reverse_geocode.ts:55-100` restarted 500 ms on every centre change.
- **B18** ~~`map/environment.rs:60-64` `tail_ambient`, `mid` and a `Vec`
  nothing reads.~~ Gone; the writes beside them already put the ambient
  intensity in the lane.
- ~~**B19**~~ `character_preview.rs:125` stands the case's actor at
  `[0, 0, -CASE_DEPTH_U]`; `character_preview.tsx:70-71,95` at the map
  origin's X and Y, its Z less `CASE_DEPTH_U`.
- ~~**B20**~~ `companion.rs:207` reads `TickState.elapsed_s`, a sum of turn
  deltas clamped to 0.1 s (`world.rs:198`); `companion.tsx:235` read wall
  seconds. The settle delay, the walk plan and the linger stretch over
  idle gaps.
- ~~**B21**~~ `chunk_node.rs:171,202,240-245` knows a page's tile by
  `*const Tile as usize`, and `page_table.rs:25,34,74-98` holds tiles by
  value in a `Vec`: a push that reallocates rebuilds every chunk, a
  `swap_remove` rebuilds the moved page's, and a set in place is missed.
  `chunk_node.ts:295-298` compared the object.
- ~~**B22**~~ `poi_index.rs:60-66` caps the page radius at 4; `:301` sorts
  unstably; `:366-372` a record refused at capacity is not seen, so its
  badge is evicted (`poi_index.ts:85-87,320,382-384`) — those three fixed.
  The fourth, `:93,469`, a record naming its page by index and looking it up
  at use where the checkpoint held the page itself, so a page evicted mid-job
  is skipped: left to the rewrite that gives every page its own index
  (BACKLOG), which ends the question instead of patching it.
- ~~**B23**~~ `poi_layer.rs:547-564` asks for placement on every camera store
  revision; `poi_layer.tsx:632-635` on the view state.
- **B24** ~~A channel's values are read as `key_cnt * width` floats by
  pointer (`model_asset.rs:195`, `world/model.rs:283-290`) from a `Vec`
  `truncate` never grows (`gltf.rs:401`); a CUBICSPLINE slice panics on a
  short output (`gltf.rs:396`). The TypeScript's subarrays clamped.
  Malformed assets only.~~ The spline reads its keys through `get` rather
  than by slicing, and the key count is now the lesser of what the input and
  the output carry, with both cut to it — so the pointer read is short by
  construction instead of clamped at the far end.
- **B25** ~~`gltf.rs:193,260-274` caches accessors as `Vec<f64>`, cloned and
  converted per read~~ (each accessor is read once, straight into its
  lane's type); integer casts saturate where typed arrays wrapped; a JSON
  chunk that is not UTF-8 reads as missing (`:133,139`).
- **B26** ~~`renderer.rs:867-889` resizes a surface's targets after
  `SETTLE_TURNS` turns at the new size; `renderer.ts:242-268` in the frame.~~
  **Kept as it is**: a drag on a window edge changes the size every frame,
  and each change would otherwise rebuild the depth and multisampled colour.
  The wait is not a delay — `request_world_tick` keeps the turns coming, and
  the buffer already made is drawn scaled until the size holds still.
- **B27** ~~`render/gpu/decor.rs:98-104,116` uploads a texture per group;
  `decor.ts:117-128` one per image.~~ `skin_group` uploads a texture, not
  just a bind group over one, and `asset_store`'s settle says outright that
  two kinds may name the same skin. The pass now holds its skins by image,
  counts the groups on each, and gives a row back when the last leaves it.
- **B28** ~~An MSAA change keeps the device and rebuilds the pipelines
  (`renderer.rs:573-585,927-929`); `stage.tsx:139-165` made a new
  renderer.~~ **Kept as it is**: a sample count is a pipeline's, and the
  targets are remade where `targets` sees it changed. Making a renderer
  again would throw away the device, its buffers and every block listed on
  it to change one number in a pipeline.
- **B29** ~~`world.rs:90` shows the debug gizmos where `APP_ENV()` is not
  production; `world.tsx:97` where `__IS_DEV__`.~~ **Kept as it is**: the
  kernel has no bundler flag to read — `APP_ENV` is what the shell's build
  info hands it — and the gizmos also wait on `show_debug_objects`, which is
  off until someone turns it on.

## X. xpute has it

- **X1** ~~`setting/store.rs:189-243` parses the overrides by hand beside
  xpute `read_json`; `\b \f \r` read as letters and a surrogate `\u` stops
  the parse.~~ The overrides are read through `read_json`'s fields, and an
  escape is decoded by the one reader that knows them all.
- **X2** `json_string` in `setting/store.rs`, `ui_store.rs` and
  `sched/trace.rs`; xpute has `Quoted`.

## The heap all of it runs in

Found after the audit, the first time the dev client ran on it: loading the
models trapped over and over on an allocation failure, in `Reader::read`
under `gltf_arrived`.

The module heap is xpute's `FixedHeap` over `__heap_base` to `MODULE_BYTES`:
32 MB less the 1 MB stack and the data, about 31 MB, in power-of-two size
classes, a freed block reused only by a request of its own class. In the
checkpoint it held the chunk builds' scratch and the build tables (the
sizing comment in `pxl-wasm`'s `lib.rs` is against those). The glTF parse
and the documents it keeps, the ledger, and the rest of what moved in with
the client now share it.

What is reserved at its worst case the first time it is touched, sizes from
`-Zprint-type-sizes` on wasm32:

| Where | Lane | Elements × bytes | Class taken |
|---|---|---|---|
| `ledger.rs:318-330` | frame ring | 4096 × 528 | 4 MB |
| | task ring | 2048 × 192 | 512 KB |
| | long frame ring | 1024 × 48 | 64 KB |
| | worst frames | 12 × 1056 | 16 KB |
| `world/decor.rs:158-159` | instances | 65536 × 32 | 2 MB |
| | groups | 65536 × 2 | 128 KB |
| `world/sched.rs:433` | slots | 4096 × 160 | 1 MB |
| ~~`world/sched.rs:374-380`~~ | ~~tick scratch, seven lists~~ | ~~4096 × 4…32~~ | ~~224 KB~~ — and again every tick: the tick's `mem::take` made a fresh reserved one and dropped it (`sched.rs:868,947`). Empty by default now; the one put back keeps what it grew to. |
| `world/sched.rs:443,446` | pages, gone | 1024 × 8, 4096 × 24 | 136 KB |

About 8 MB. The parse read each accessor into `f64`s, kept a copy in its
cache, cloned that copy for the read and converted the clone: 28 bytes a
float element at the read, and 8 kept for every accessor until the parse
ended. It reads 4 (2 for joints) now.

### Measured

The dev client with only the terrain fetch let out, `FixedHeap` counting the
bytes it has handed out and, per size class, the blocks cut and held; the
shell printing them each turn.

- After boot and `WORLD_CREATE` the top is at 5.2 MB: 3.5 MB of the heap.
- After the first turn, before any page has arrived: 26.6 MB live of 26.8 MB
  cut, 0.2 MB idle on the free lists. What fills the heap is use, not
  fragmentation.
- 25 MB of it is 57 blocks: 30 of 256 KB, 25 of 512 KB, one of 1 MB, one of
  4 MB. By their sizes (not traced to their owners one by one):
  - the POI index, about 20 MB: two `Lanes` at `CAPACITY` 65536
    (`poi_index.rs:73,119-136`), each eleven lanes reserved at that capacity
    and a `SpatialHash` of nine capacity lanes over 131071 buckets, and `seen`
    at the same capacity — the 256 KB and 512 KB blocks;
  - the ledger's frame ring, 4096 × 528 B: the 4 MB block;
  - the scheduler's slots, 4096 × 160 B: the 1 MB block.
- As pages arrive, live grows about 20 KB a turn, the tick's small blocks
  (128 B to 1 KB), with one more 512 KB and one more 1 MB block; idle stays
  under 1 MB. After 33 turns: 28.6 MB live, the top at 31.2 MB of 32, and the
  next large request traps.

## Read and found equal

frame_budget, priority, summary, trace, the ledger's worst frames and peak,
the io queue's order, the pages set, blend_hash, the decor block layout and
raycast groups, the shared crate, ui_store, search's go-to, layer_table,
layers, the glTF parse, model_asset's lanes, joints, height and scale, the
loader's cache, the puck, the shadow, the stage's values, the badges', decor's
and skinned's uniforms and groups, the renderer's pass order, mesh offsets
and upload / rewrite / free order.

## Order

One small change a turn, `cargo check` green:

1. P1.
2. H1–H3.
3. B21.
4. B19, B20.
5. B15–B17.
6. B22, B23.
7. B1, B2.
8. H4–H6, H10, H11.
9. H7.
10. X1, X2.
11. B6, B9–B12, B29, H8, H9, B18.
12. B13, B14, B24–B28.
