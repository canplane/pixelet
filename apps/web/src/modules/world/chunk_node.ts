// @pxl/web/modules/world/chunk_node.ts

/**
 * One chunk, owned imperatively.
 *
 * This was a React component (`Chunk`, plus a `ChunkMesh` per LOD role and a
 * `DecorVariantMesh` per decor variant) and there were thousands of them
 * live. React was not being asked to do anything React is good at here:
 * nothing about a chunk is described by props, no chunk reads another's
 * state, and every one of its "components" existed purely to own a
 * `THREE.Mesh` behind a `<primitive>`. What React did add was a deferral —
 * it decides *later* when a layout actually mounts and unmounts — and that
 * deferral is what made the chunk's own memory unsafe to reason about:
 * between `set_layout` and the mount there is a window in which the layout
 * names meshes that nothing owns, and the LOD scheduler runs inside it. Three
 * separate reference-counting bugs came out of that window before this
 * existed (see the git history around chunk_stage.tsx).
 *
 * So: a plain object with a `THREE.Group`, a `dispose()`, and a `commit()`
 * that builds geometry and attaches it in the same call. Nothing here is
 * deferred, which is the whole point — a mesh has exactly one owner, this
 * node, for a lifetime this file decides.
 *
 * Three.js stays. `Mesh`/`BufferGeometry`/`Material` are the abstraction
 * being kept; going to raw GL calls would harden the code around WebGL
 * semantics, and WebGPU is on the roadmap.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useDebugStore } from "@/app/settings/debug_store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import type { AnchorRegistry } from "@/modules/world/anchor/registry.ts";
import { decor_field, type DecorContribution, type DecorVariantContribution, variant_key } from "@/modules/world/decor/field.ts";
import { build_decor_instances, type ModelInstanceTransformList } from "@/modules/world/decor/decor_layer.ts";
import { decor_bundle_assets, type DecorBundleAssets, subscribe_decor_assets } from "@/modules/world/decor/asset_store.ts";
import { decor_column_revision, object_column_of, object_column_revision, subscribe_decor_column, subscribe_object_column } from "@/modules/world/objects/store.ts";
import { chunk_material, create_geometry, VISIBLE_ATTR } from "@/modules/world/render/chunk_material.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { APPROXIMATE_CHUNK_RADIUS, chunk_distance_to_camera, estimate_initial_chunk_view } from "@/render/chunk_lod.ts";
import {
  bucket_from_baked,
  type BuiltChunkBucket,
  chunk_bounds,
  type ChunkBounds,
  type ChunkLodRole,
  type ChunkOffsets,
  mesh_bounds,
  release_built_mesh,
  retain_built_mesh,
} from "@/render/layer/pipeline/chunk_bucket.ts";
import type { ChunkKey, WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { composite_chunk_layers } from "@/render/layer/pipeline/compositor.ts";
import { abort_bake, bake_open, type BakeCursor, begin_bake, finish_bake, step_bake } from "@/render/layer/pipeline/chunk_mesher.ts";
import {
  abort_column_scan,
  begin_column_scan,
  column_scan_open,
  type ColumnRunSet,
  type ColumnScan,
  finish_column_scan,
  release_column_runs,
  scan_column_rows,
} from "@/render/layer/pipeline/voxel_scan.ts";
import { type ChunkVolume, release_chunk_volume } from "@/render/layer/pipeline/volume.ts";
import type { FrameBudget } from "@/modules/world/sched/tick.ts";
import { get_lod_guide, get_lod_pair_visibility, type LodStep, step_lod_toward } from "@/render/lod.ts";
import { apply_visibility, type ChunkMeshRegistry, type ChunkVisibilityDecision, type MountedLodMesh, visibility_coverage } from "@/render/lod_visibility.ts";

// ---- Budgets and tiers ----

/* Nothing here is deferred any more. Both the scan and the bake step a few
 * rows at a time under the frame's shared budget (voxel_scan.ts,
 * chunk_mesher.ts), so an expensive chunk costs more frames rather than a
 * longer one — which is what the per-chunk defer-once heuristics that used to
 * sit here were approximating from a cost estimate. */

/**
 * The coarsest LOD_STEP_BY_LEVEL tier (render/lod.ts) doubles as the
 * memory-saving "nothing built" state for a chunk that's off-screen or
 * hasn't measured its own real target yet — one real box's worth of
 * geometry per chunk instead of zero, so a later re-entry always has
 * *something* to transition from rather than popping in from a true void.
 */
export const OFF_SCREEN_LOD_STEP: LodStep = 64;

/**
 * How many built LOD tiers one chunk may keep. Three is the floor — the
 * active primary, its secondary during a blend, and OFF_SCREEN's own
 * standing entry — so four leaves exactly one tier of hysteresis for the
 * threshold hunting the cache exists to absorb, and nothing beyond that.
 * Measured before this cap existed: live mesh bytes climbed to 1.26GB while
 * the in-frustum count and the geometry count held flat, which was this
 * cache filling in every tier a chunk had ever been asked for.
 */
const MAX_CACHED_LOD_TIERS = 4;

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/** Cached rather than read per chunk: recheck() runs over every live node
 * on its own interval, and a store read there is a couple of thousand
 * lookups a tick for a flag that changes when someone clicks a toggle. */
let pixel_dither = useDebugStore.getState().pixelDither;
useDebugStore.subscribe((state) => {
  pixel_dither = state.pixelDither;
});

// ---- Dev-only counters, read by the scheduler's frame sample ----

export type TimingStats = { total_ms: number; count: number; max_ms: number };

export function record_timing(stats: TimingStats, ms: number): void {
  stats.total_ms += ms;
  stats.count++;
  stats.max_ms = Math.max(stats.max_ms, ms);
}

/** Geometry creation + GPU buffer upload + bounds, per attached mesh. */
export const mesh_mount_stats: TimingStats = { total_ms: 0, count: 0, max_ms: 0 };
/** geometry.dispose() on detach — a zoom-out flips many chunks at once. */
export const mesh_unmount_stats: TimingStats = { total_ms: 0, count: 0, max_ms: 0 };

/**
 * Cumulative (since page load) LOD build timing, dev-only — no console.log
 * anywhere near this (a hot console during a build storm would itself skew
 * the measurement); just plain object writes each call, exposed read-only
 * via ChunkStage's __PXL_RENDER_STATS__ dev global.
 */
export const lod_build_stats = {
  scan_ms: 0,
  scan_count: 0,
  scan_max_ms: 0,
  scan_bands: 0,
  scan_bands_max: 0,
  build_ms: 0,
  build_count: 0,
  build_max_ms: 0,
  build_runs: 0,
  build_runs_max: 0,
};

/** Splits one commit() call's cost, dev-only. `attach_ms_max` replaces what
 * used to be `set_layout_ms_max`: the same moment, but now it is the real
 * scene mutation rather than a React state update that only scheduled one. */
export const commit_step_stats = {
  build_ms_max: 0,
  bounds_ms_max: 0,
  attach_ms_max: 0,
};

/**
 * Chunks that appeared / disappeared since the last frame sample, dev-only.
 * `rebuilt` counts layouts actually applied — the imperative successor to
 * counting React re-renders, and a truer number: a re-render could happen
 * for reasons that changed nothing, an applied layout could not.
 */
export const node_counts = { added: 0, removed: 0, rebuilt: 0 };

/**
 * Every live node, for the retention walk below.
 *
 * The pool can say exactly what it holds and the tile store can say how many
 * pages it decoded, but between them sat 4.4GB nobody could name: measured
 * heap 5268MB against 840MB of pool and ~36MB of tiles. Guessing which
 * subsystem owns an unattributed gap is what this whole exercise keeps
 * proving to be a waste, so the gap gets a counter instead.
 */
const LIVE_NODES = new Set<ChunkNode>();

export type ChunkRetention = {
  nodes: u32;
  /** Nodes holding a scanned run set, and what those sets cost in the pool. */
  run_sets: u32;
  run_set_mb: u32;
  /** Built LOD tiers held across every node's cache — bounded by
   * MAX_CACHED_LOD_TIERS each, but the product is what matters. */
  tiers: u32;
  /** Decor: cached per-step builds, and the transform objects inside them.
   * Neither is pooled, and an instance is a plain object. */
  decor_steps: u32;
  decor_instances: u32;
};

export function chunk_retention(): ChunkRetention {
  const out: ChunkRetention = { nodes: 0, run_sets: 0, run_set_mb: 0, tiers: 0, decor_steps: 0, decor_instances: 0 };
  let run_bytes = 0;
  for (const node of LIVE_NODES) {
    out.nodes++;
    const runs = node.retained_column_runs;
    if (runs) {
      out.run_sets++;
      run_bytes += runs.block.bytes;
    }
    out.tiers += node.retained_tier_cnt;
    for (const per_bundle of node.retained_decor) {
      out.decor_steps++;
      for (const variants of per_bundle) {
        for (const list of variants) out.decor_instances += list.length;
      }
    }
  }
  out.run_set_mb = Math.round(run_bytes / 1048576);
  return out;
}

export function take_node_counts(): { added: u32; removed: u32; rebuilt: u32 } {
  const out = { added: node_counts.added, removed: node_counts.removed, rebuilt: node_counts.rebuilt };
  node_counts.added = 0;
  node_counts.removed = 0;
  node_counts.rebuilt = 0;
  return out;
}

// ---- The scheduler's view of a chunk ----

export type ChunkLodControl = {
  blend_active: boolean;
  settling: boolean;
  in_frustum: boolean;
  distance2: f64;
  /** Squared distance from where the camera is heading (sched/priority.ts).
   * Orders the work; `distance2` still picks the tier. */
  lead_distance2: f64;
  pending: boolean;
  /** When `pending` last went from false to true, in ms; -1 while not
   * pending. What the ledger reads to see whether anything is being starved
   * — the progress rule guarantees one step a frame, not which one. */
  pending_since: f64;
  /** Holding the scan slot (voxel_scan.ts) or the bake slot
   * (chunk_mesher.ts). The commit loop puts this control first: nothing else
   * can use that slot until it finishes, and a step cap reached on nearer
   * chunks would otherwise starve it — and with it everything waiting. */
  holds_slot: boolean;
  recheck(camera: THREE.Camera, frustum: THREE.Frustum, lead: THREE.Vector3): void;
  commit(budget: FrameBudget): boolean;
};

/**
 * A plain module singleton now, not a React context. Chunk nodes are not
 * components and have no tree to read a provider from, and there was only
 * ever one registry.
 */
export type LodSchedulerRegistry = { controls: Set<ChunkLodControl> };
export const lod_scheduler: LodSchedulerRegistry = { controls: new Set() };

// ---- Layout ----

/**
 * One entry per bundle (matching bundle_assets' own order), each itself one
 * entry per that bundle's own variant. `secondary` mirrors LodLayout's
 * exactly (null outside a blend band) rather than decor tracking its own
 * independent blend state.
 */
type DecorLayoutPair = {
  primary: ModelInstanceTransformList[][];
  secondary: ModelInstanceTransformList[][] | null;
};
const EMPTY_DECOR_PAIR: DecorLayoutPair = { primary: [], secondary: null };

type LodLayout = {
  primary: LodStep;
  secondary: LodStep | null;
  buckets: BuiltChunkBucket[];
  decor: DecorLayoutPair;
  bounds: ChunkBounds | null;
  ready: boolean;
};

function blend_hash(key: string): f64 {
  let hash = 2166136261;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) / 0x1_0000_0000;
}

function set_three_box(box: THREE.Box3, bounds: ChunkBounds): THREE.Box3 {
  box.min.set(bounds.min_x, bounds.min_y, bounds.min_z);
  box.max.set(bounds.max_x, bounds.max_y, bounds.max_z);
  return box;
}

/** Shared placeholder a mesh holds before its first real geometry and again
 * after that geometry is disposed — never uploaded, never mutated. */
const EMPTY_GEOMETRY = new THREE.BufferGeometry();

/** One attached cube mesh: the scene object, the mesh data behind it, and
 * the registry entry the LOD blend flips. Kept together because they are
 * created and destroyed as one thing. */
type AttachedCubes = {
  mesh: THREE.Mesh;
  bucket: BuiltChunkBucket;
  mounted: MountedLodMesh | null;
};

export type ChunkNodeDeps = {
  page_idx: IVec;
  offset: ChunkOffsets;
  stack: WorldLayerStack;
  camera: THREE.Camera;
  anchor_registry: AnchorRegistry;
};

export class ChunkNode implements ChunkLodControl {
  /** This chunk's own scene node, positioned at its page-local origin. */
  readonly group: THREE.Group;

  blend_active = true;
  settling = false;
  in_frustum: boolean;
  distance2: f64;
  lead_distance2: f64;
  pending = false;
  pending_since: f64 = -1;

  private readonly page_off: IVec;
  private readonly key: ChunkKey;
  private readonly object_key: IVec;
  /** This chunk's own absolute lattice origin, and its identity in
   * decor/field.ts — decor lives in one world-wide field now, so it needs
   * both a stable name and the offset to place chunk-local instances at. */
  private readonly base_i: IVec;
  private readonly decor_key: string;
  private readonly stable_hash: f64;
  private readonly registry: ChunkMeshRegistry = { meshes: new Set() };

  private readonly bucket_cache = new Map<LodStep, BuiltChunkBucket>();
  private readonly decor_cache = new Map<LodStep, ModelInstanceTransformList[][]>();
  /**
   * Every column's runs, scanned once from the live volume and reused across
   * every LOD tier and every transition after that — the composited dense
   * bands live for one synchronous pass, but this copy of their *results*
   * (a packed run set, not views into the volume) survives it. Its block
   * comes from the same pool the vertex blocks do, so it has to be handed
   * back the same way — see clear_column_runs.
   */
  private column_runs: ColumnRunSet | null = null;
  /** A bake in progress, held across frames like the scan and bounded the
   * same way: one chunk at a time. `step` is the tier it is baking. */
  private bake: { step: LodStep; cursor: BakeCursor; spent_ms: number } | null = null;
  /** The tiers the current build_buckets call is after. */
  private wanted: readonly LodStep[] = [];
  /**
   * A scan in progress, held across frames: the composite it reads and the
   * row it is up to. Pooled bands stay checked out for as long as this is
   * set, which voxel_scan.ts bounds to one chunk at a time — so a node that
   * stops needing its scan (disposed, source changed, left the frustum) has
   * to abort it rather than drop it. See abort_scan.
   */
  private scan: { volume: ChunkVolume; cursor: ColumnScan; spent_ms: number } | null = null;

  private layout: LodLayout;
  private desired: { primary: LodStep; secondary: LodStep | null };

  private tile: unknown;
  private show_structures: boolean;
  private objects_revision: number;
  private decor_revision: number;
  private show_decor: boolean;
  private bundle_assets: readonly DecorBundleAssets[];

  private attached_cubes: AttachedCubes[] = [];
  private readonly unsubscribes: (() => void)[] = [];

  /** What this chunk last told decor/field.ts. `dirty` forces the next sync
   * through even when neither of the other two moved — a rebuilt layout is
   * the same chunk showing the same tier, with different instances in it. */
  private decor_shown = false;
  private decor_role: ChunkLodRole = "primary";
  private decor_dirty = true;

  // Scratch, reused by recheck() so a per-frame sweep over thousands of
  // nodes allocates nothing.
  private readonly center = new THREE.Vector3();
  private readonly local_box = new THREE.Box3();
  private readonly world_box = new THREE.Box3();
  private readonly sphere: THREE.Sphere;

  private disposed = false;

  /** The one place `pending` is written, so the time it became pending is
   * never out of step with it. */
  private set_pending(pending: boolean): void {
    if (pending && !this.pending) this.pending_since = performance.now();
    if (!pending) this.pending_since = -1;
    this.pending = pending;
  }

  get holds_slot(): boolean {
    return this.scan !== null || this.bake !== null;
  }

  constructor(private readonly deps: ChunkNodeDeps) {
    const { page_idx, offset, anchor_registry, camera } = deps;
    this.page_off = ivec.gather(BigInt(offset.base_x), BigInt(offset.base_y), 0n);
    this.key = { page_idx, page_off: this.page_off };
    this.object_key = object_column_of(page_idx, this.page_off);
    this.base_i = ivec.add(FROM_L3_IDX(page_idx), this.page_off);
    this.decor_key = `${page_idx.toString()}:${offset.key}`;
    this.stable_hash = blend_hash(this.decor_key);
    this.sphere = new THREE.Sphere(this.center, APPROXIMATE_CHUNK_RADIUS);

    this.group = new THREE.Group();
    this.group.name = `chunk/${offset.key}`;
    this.group.position.set(offset.base_x * U_PER_CELL, offset.base_y * U_PER_CELL, 0);

    const view = estimate_initial_chunk_view(camera, anchor_registry.cur_map_origin, this.base_i);
    // Seeded from the same one-shot camera-relative estimate rather than a
    // true (Infinity) sentinel: the scheduler ranks a same-frame batch of
    // fresh controls by distance2, and every one of them starts with nothing
    // built to measure exactly, so Infinity would degrade that ranking to
    // registration order for the whole batch.
    this.in_frustum = view.in_frustum;
    this.distance2 = view.distance2;
    this.lead_distance2 = view.distance2;
    this.desired = { primary: view.in_frustum ? view.lod_step : OFF_SCREEN_LOD_STEP, secondary: null };
    this.layout = {
      primary: this.desired.primary,
      secondary: null,
      buckets: [],
      decor: EMPTY_DECOR_PAIR,
      bounds: null,
      ready: false,
    };

    this.tile = useTileStore.getState().get_tile(page_idx);
    this.show_structures = useSettingsStore.getState().show_structures;
    this.objects_revision = object_column_revision(this.object_key);
    this.decor_revision = decor_column_revision(this.object_key);
    this.show_decor = useSettingsStore.getState().show_decor;
    this.bundle_assets = decor_bundle_assets();

    // Direct store subscriptions rather than hooks: a node is not a
    // component, and a placement should not have to travel through a render
    // to reach the chunk it changed.
    this.unsubscribes.push(
      useTileStore.subscribe(() => this.on_source_change()),
      useSettingsStore.subscribe(() => {
        this.on_source_change();
        this.on_decor_change();
      }),
      subscribe_object_column(this.object_key, () => this.on_source_change()),
      subscribe_decor_column(this.object_key, () => this.on_decor_change()),
      subscribe_decor_assets(() => this.on_decor_assets()),
    );

    lod_scheduler.controls.add(this);
    if (import.meta.env.DEV) LIVE_NODES.add(this);
    if (import.meta.env.DEV) node_counts.added++;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    lod_scheduler.controls.delete(this);
    LIVE_NODES.delete(this);
    for (const off of this.unsubscribes) off();
    this.abort_work();
    decor_field.clear_chunk(this.decor_key);
    this.detach_all();
    for (const bucket of this.layout.buckets) release_built_mesh(bucket.cubes);
    this.layout = { ...this.layout, buckets: [], decor: EMPTY_DECOR_PAIR, ready: false };
    this.release_bucket_cache();
    this.decor_cache.clear();
    this.clear_column_runs();
    this.group.removeFromParent();
    if (import.meta.env.DEV) node_counts.removed++;
  }

  /** Read by chunk_retention; dev-only, and never a way to reach in and
   * mutate what this node owns. */
  get retained_column_runs(): ColumnRunSet | null {
    return this.column_runs;
  }
  get retained_tier_cnt(): u32 {
    return this.bucket_cache.size;
  }
  get retained_decor(): Iterable<ModelInstanceTransformList[][]> {
    return this.decor_cache.values();
  }

  // ---- caches ----

  /** The cache's own reference to every bucket it holds, dropped together. */
  private release_bucket_cache(): void {
    for (const bucket of this.bucket_cache.values()) release_built_mesh(bucket.cubes);
    this.bucket_cache.clear();
  }

  /**
   * Both build caches AND the scan they were built from — the two moments
   * that need a hard reset (a source revision changed, or this chunk just
   * left the frustum for commit() to act on) always want all three gone
   * together. The frustum-exit path in recheck() wants the softer prune
   * below instead: keep something cheap resident, not nothing.
   */
  private reset_all_caches(): void {
    this.abort_work();
    this.release_bucket_cache();
    this.decor_cache.clear();
    this.clear_column_runs();
  }

  private clear_column_runs(): void {
    if (!this.column_runs) return;
    release_column_runs(this.column_runs);
    this.column_runs = null;
  }

  /** Drops every cached tier except OFF_SCREEN's, so a later re-entry has a
   * real tier to transition from rather than a void. */
  private prune_to_default(): void {
    for (const [step, bucket] of this.bucket_cache) {
      if (step === OFF_SCREEN_LOD_STEP) continue;
      this.bucket_cache.delete(step);
      release_built_mesh(bucket.cubes);
    }
    for (const step of this.decor_cache.keys()) {
      if (step !== OFF_SCREEN_LOD_STEP) this.decor_cache.delete(step);
    }
  }

  /** Evicts oldest-built first, never the tiers currently asked for or the
   * off-screen default. The layout holds its own reference, so evicting a
   * tier that is still on screen costs a rebuild, not a black chunk. */
  private trim_bucket_cache(active: readonly LodStep[]): void {
    for (const [step, bucket] of this.bucket_cache) {
      if (this.bucket_cache.size <= MAX_CACHED_LOD_TIERS) return;
      if (step === OFF_SCREEN_LOD_STEP || active.includes(step)) continue;
      this.bucket_cache.delete(step);
      release_built_mesh(bucket.cubes);
    }
  }

  // ---- source changes ----

  /**
   * A source revision rebuilds the currently displayed LOD, not the whole
   * visible world. `objects_revision` is the SOLID-only revision: a
   * decor-only change never lands here. A solid change still clears decor's
   * cache too, because a newly placed cube can occlude a procedurally
   * scattered decor candidate that used to render there.
   */
  private on_source_change(): void {
    if (this.disposed) return;
    const tile = useTileStore.getState().get_tile(this.deps.page_idx);
    const show_structures = useSettingsStore.getState().show_structures;
    const objects_revision = object_column_revision(this.object_key);
    if (Object.is(this.tile, tile) && this.show_structures === show_structures && this.objects_revision === objects_revision) return;
    this.tile = tile;
    this.show_structures = show_structures;
    this.objects_revision = objects_revision;

    this.reset_all_caches();
    if (!this.layout.ready) {
      if (this.layout.bounds !== null) this.apply_layout({ ...this.layout, bounds: null });
      return;
    }
    const content = this.build_layout_content(this.layout.primary, this.layout.secondary);
    // Deferred — keep whatever is on screen, just mark not-ready so the next
    // recheck() sets pending and retries.
    if (content === null) {
      this.apply_layout({ ...this.layout, ready: false });
      return;
    }
    this.apply_layout({ ...this.layout, ...content, ready: true });
  }

  /**
   * Decor models finish loading well after the first chunks are built, and
   * a node that never heard about it drew no decor for the rest of its
   * life — which is why decor showed or did not depending on whether the
   * browser cache beat the first chunk build. Decor meshes only: a model
   * arriving says nothing about cube content.
   */
  private on_decor_assets(): void {
    if (this.disposed) return;
    const assets = decor_bundle_assets();
    if (this.bundle_assets === assets) return;
    this.bundle_assets = assets;
    this.decor_cache.clear();
    if (!this.layout.ready) return;
    this.apply_layout({ ...this.layout, decor: this.build_decor_pair(this.layout.primary, this.layout.secondary) });
  }

  /**
   * Decor's own revision-only rebuild. It must never touch bucket_cache or
   * column_runs: that would reintroduce the needless cube rescan/rebake —
   * visible as a flicker — this split exists to avoid.
   */
  private on_decor_change(): void {
    if (this.disposed) return;
    const revision = decor_column_revision(this.object_key);
    // show_decor sits here rather than in on_source_change for the reason
    // this split exists at all: hiding scattered decor changes no cube, and
    // rebuilding them would flicker. It is the decor half's own
    // show_structures.
    const show_decor = useSettingsStore.getState().show_decor;
    if (revision === this.decor_revision && show_decor === this.show_decor) return;
    this.decor_revision = revision;
    this.show_decor = show_decor;
    this.decor_cache.clear();
    if (!this.layout.ready) return;
    this.apply_layout({ ...this.layout, decor: this.build_decor_pair(this.layout.primary, this.layout.secondary) });
  }

  // ---- building ----

  /**
   * Cube content only (terrain + solid objects, chunk_mesher.ts's baked
   * mesh). A null return means "not finished this frame" — the scan or the
   * bake is part-way, or waiting on the slot another chunk holds — distinct
   * from an empty result (built, genuinely nothing to draw).
   *
   * Decor is deliberately not built here: any decor-only change would
   * otherwise force this to re-run and rescan cube content that never
   * changed. build_decor_step below is the split-out replacement, and the
   * two still run together whenever cube content itself rebuilds, so decor
   * is never committed without the cube content it sits on.
   */
  private build_buckets(primary: LodStep, secondary: LodStep | null, budget: FrameBudget | null): BuiltChunkBucket[] | null {
    const steps = secondary === null || secondary === primary ? [primary] : [primary, secondary];
    const missing = steps.filter((step) => !this.bucket_cache.has(step));
    this.wanted = steps;

    if (missing.length > 0) {
      if (!this.column_runs) {
        const state = this.advance_scan(budget);
        if (state !== "done") return null;
      }
      const column_runs = this.column_runs;
      if (column_runs && this.advance_bake(column_runs, budget) !== "done") return null;
    }

    // Tiers outside the current pair are kept rather than evicted, because a
    // build storm during a pan hunts back and forth across a threshold and
    // rebuilding from column_runs isn't free — but only up to
    // MAX_CACHED_LOD_TIERS of them.
    this.trim_bucket_cache(steps);
    return steps
      .map((step) => this.bucket_cache.get(step))
      .filter((bucket): bucket is BuiltChunkBucket => bucket !== undefined)
      .map((bucket) => ({ ...bucket, role: (bucket.lod_step === primary ? "primary" : "secondary") as ChunkLodRole }))
      .sort((a, b) => a.lod_step - b.lod_step);
  }

  /**
   * Moves the scan along, and says where it got to.
   *
   * "done" — column_runs is filled and the bake can proceed.
   * "busy" — rows were scanned, more remain; the chunk stays pending and the
   *          caller reports real work done, since it was.
   * "blocked" — nothing scanned: another chunk holds the one scan slot, or no
   *          layer had painted this key yet (a real if rare transient a batch
   *          of chunks racing their first build can hit — NOT "nothing here,
   *          done forever", which is why it keeps the chunk pending rather
   *          than falling through to an empty bucket list and a permanent
   *          hole).
   *
   * With no budget (a source-change rebuild outside the tick) the scan runs
   * to completion in one call, as it always did.
   */
  private advance_scan(budget: FrameBudget | null): "done" | "busy" | "blocked" {
    const step_t0 = performance.now();
    if (!this.scan) {
      // Cheap check before the expensive compose: composing and then finding
      // the slot taken would spend a couple of milliseconds per pending chunk
      // for nothing, every frame the slot is busy.
      if (column_scan_open()) return "blocked";
      const volume = composite_chunk_layers(this.deps.stack, this.key);
      if (volume === null) return "blocked";
      const cursor = begin_column_scan(volume);
      if (cursor === null) {
        release_chunk_volume(volume);
        return "blocked";
      }
      this.scan = { volume, cursor, spent_ms: 0 };
      lod_build_stats.scan_bands += volume.bands.length;
      lod_build_stats.scan_bands_max = Math.max(lod_build_stats.scan_bands_max, volume.bands.length);
    }

    // At least one step, whatever the budget says — forward progress is the
    // invariant — then as many more as the budget still has room for. A step
    // is a few rows, around a millisecond on a band-heavy chunk.
    let done: boolean;
    do {
      done = scan_column_rows(this.scan.cursor);
    } while (!done && (budget === null || budget.remaining() > 0));
    // Time actually spent scanning, summed across the frames it took — not
    // the wall time from first step to last, which would count the frames
    // in between as cost.
    this.scan.spent_ms += performance.now() - step_t0;
    if (!done) return "busy";

    this.column_runs = finish_column_scan(this.scan.cursor);
    release_chunk_volume(this.scan.volume);
    const scan_ms = this.scan.spent_ms;
    this.scan = null;
    lod_build_stats.scan_ms += scan_ms;
    lod_build_stats.scan_count++;
    lod_build_stats.scan_max_ms = Math.max(lod_build_stats.scan_max_ms, scan_ms);
    return "done";
  }

  /**
   * Bakes whatever tier `wanted` still lacks, stepping under the budget, and
   * says where it got to — the same three answers advance_scan gives.
   *
   * A bake already open is carried to completion even if its tier is no
   * longer wanted: it is a bounded amount of work, the result is cached
   * rather than wasted, and restarting on every change of mind is how a
   * camera hunting across a threshold would keep a chunk from ever finishing.
   */
  private advance_bake(column_runs: ColumnRunSet, budget: FrameBudget | null): "done" | "busy" | "blocked" {
    for (;;) {
      if (!this.bake) {
        const step = this.wanted.find((s) => !this.bucket_cache.has(s));
        if (step === undefined) return "done";
        if (bake_open()) return "blocked";
        const cursor = begin_bake(column_runs, step);
        if (cursor === null) return "blocked";
        this.bake = { step, cursor, spent_ms: 0 };
      }

      const t0 = performance.now();
      let done: boolean;
      do {
        done = step_bake(this.bake.cursor);
      } while (!done && (budget === null || budget.remaining() > 0));
      this.bake.spent_ms += performance.now() - t0;
      if (!done) return "busy";

      const { step, cursor, spent_ms } = this.bake;
      this.bake = null;
      this.bucket_cache.set(step, bucket_from_baked(finish_bake(cursor), step, "primary"));
      lod_build_stats.build_ms += spent_ms;
      lod_build_stats.build_count++;
      lod_build_stats.build_max_ms = Math.max(lod_build_stats.build_max_ms, spent_ms);
      lod_build_stats.build_runs += column_runs.count;
      lod_build_stats.build_runs_max = Math.max(lod_build_stats.build_runs_max, column_runs.count);

      // A second wanted tier starts in this same call only if the budget
      // still has room; otherwise it waits for the next commit.
      if (budget !== null && budget.remaining() <= 0) {
        return this.wanted.some((s) => !this.bucket_cache.has(s)) ? "busy" : "done";
      }
    }
  }

  /** Drops any scan or bake in progress and returns what it holds. Called
   * wherever this chunk stops needing them — which must include every place
   * `pending` goes false, or the slot stays taken and nothing else builds. */
  private abort_work(): void {
    this.abort_scan();
    if (this.bake) {
      abort_bake(this.bake.cursor);
      this.bake = null;
    }
  }

  /** Drops a scan in progress and returns its composite to the pool. */
  private abort_scan(): void {
    if (!this.scan) return;
    abort_column_scan(this.scan.cursor);
    release_chunk_volume(this.scan.volume);
    this.scan = null;
  }

  /** Decor's own build — no volume scan, no dependency on the cube caches
   * at all (a decor entry never paints a voxel). */
  private build_decor_step(step: LodStep): ModelInstanceTransformList[][] {
    const cached = this.decor_cache.get(step);
    if (cached) return cached;
    const built = this.bundle_assets.map(({ bundle }) => build_decor_instances(this.tile as never, step, this.deps.page_idx, this.deps.offset.base_x, this.deps.offset.base_y, bundle));
    this.decor_cache.set(step, built);
    return built;
  }

  private build_decor_pair(primary: LodStep, secondary: LodStep | null): DecorLayoutPair {
    return {
      primary: this.build_decor_step(primary),
      secondary: secondary !== null && secondary !== primary ? this.build_decor_step(secondary) : null,
    };
  }

  private build_layout_content(
    primary: LodStep,
    secondary: LodStep | null,
  ): { buckets: BuiltChunkBucket[]; decor: DecorLayoutPair; bounds: ChunkBounds | null } | null {
    const buckets = this.build_buckets(primary, secondary, null);
    if (buckets === null) return null;
    return { buckets, decor: this.build_decor_pair(primary, secondary), bounds: chunk_bounds(buckets) };
  }

  // ---- attaching ----

  /**
   * The one place the layout changes, and the reason this file exists: the
   * new layout's meshes are built and attached *here*, synchronously, so
   * there is never a moment where a layout names meshes nothing owns. Under
   * React that moment was real and the LOD scheduler ran inside it.
   *
   * Retain-before-release, so a bucket carried across two layouts never
   * transiently reaches zero.
   */
  private apply_layout(next: LodLayout): void {
    const previous = this.layout;
    for (const bucket of next.buckets) retain_built_mesh(bucket.cubes);
    this.layout = next;
    this.attach(next);
    for (const bucket of previous.buckets) release_built_mesh(bucket.cubes);
    if (import.meta.env.DEV) node_counts.rebuilt++;
  }

  private attach(layout: LodLayout): void {
    const t0 = import.meta.env.DEV ? performance.now() : 0;
    this.detach_all();
    for (const bucket of layout.buckets) {
      if (bucket.cubes.vertex_cnt > 0) this.attach_cubes(bucket);
    }
    this.decor_dirty = true;
    this.sync_decor();
    if (import.meta.env.DEV) record_timing(mesh_mount_stats, performance.now() - t0);
  }

  private attach_cubes(bucket: BuiltChunkBucket): void {
    const built = bucket.cubes;
    const geometry = create_geometry(built);
    const mesh = new THREE.Mesh(geometry, chunk_material());
    // Unconditional: the real cost is gated at the renderer level by the
    // Canvas's own `shadows` prop, and three.js compiles no shadow-map
    // shader while gl.shadowMap.enabled is false.
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.name = `${bucket.role}/lod/${bucket.lod_step}`;
    built.visibility.fill(bucket.role === "primary" ? 1 : 0);

    // Not geometry.computeBoundingBox()/computeBoundingSphere(): those walk
    // every vertex through three.js's generic accumulation, real cost on a
    // rebuild storm. mesh_bounds' direct min/max scan over the already-baked
    // real positions is exact and far cheaper.
    const bounds = mesh_bounds(built);
    if (bounds) {
      geometry.boundingBox = set_three_box(new THREE.Box3(), bounds);
      const center = geometry.boundingBox.getCenter(new THREE.Vector3());
      geometry.boundingSphere = new THREE.Sphere(center, center.distanceTo(geometry.boundingBox.max));
    } else {
      geometry.boundingBox = new THREE.Box3();
      geometry.boundingSphere = new THREE.Sphere();
    }

    const attribute = geometry.getAttribute(VISIBLE_ATTR);
    let mounted: MountedLodMesh | null = null;
    if (attribute instanceof THREE.BufferAttribute) {
      attribute.needsUpdate = true;
      mounted = { built, role: bucket.role, attribute };
      this.registry.meshes.add(mounted);
    }
    this.group.add(mesh);
    this.attached_cubes.push({ mesh, bucket, mounted });
  }

  /**
   * Tells decor/field.ts what this chunk shows right now, if that has
   * changed.
   *
   * A chunk owns no decor mesh any more; it owns a *contribution*, and the
   * three things that can change it are whether the chunk is on screen, which
   * LOD tier it is showing, and whether its instances were rebuilt. Anything
   * else — an asset arriving, another chunk moving, the floating origin
   * shifting — is not this chunk's business and never was.
   */
  private sync_decor(want_role: ChunkLodRole = this.decor_role): void {
    const shown = this.in_frustum;
    const role: ChunkLodRole = this.layout.decor.secondary !== null ? want_role : "primary";
    if (!this.decor_dirty && shown === this.decor_shown && role === this.decor_role) return;
    this.decor_dirty = false;
    this.decor_shown = shown;
    this.decor_role = role;
    if (!shown) {
      decor_field.clear_chunk(this.decor_key);
      return;
    }

    const source = role === "secondary" ? this.layout.decor.secondary! : this.layout.decor.primary;
    const contribution: Map<string, DecorVariantContribution> = new Map();
    this.bundle_assets.forEach(({ bundle, geometries, materials }, bi) => {
      const variants: ModelInstanceTransformList[] | undefined = source[bi];
      if (!variants) return;
      geometries.forEach((geometry, variant) => {
        const material = materials[variant];
        const instances = variants[variant];
        // A variant whose model or skin is still in flight contributes
        // nothing; the arrival rebuilds this chunk's decor and lands here
        // again with both halves in hand.
        if (!geometry || !material || !instances || instances.length === 0) return;
        contribution.set(variant_key(bundle.kind, variant), { geometry, material, instances });
      });
    });
    decor_field.set_chunk(this.decor_key, this.base_i, contribution as DecorContribution);
  }

  /**
   * One decision, two consumers. Cube content dithers between the two tiers
   * per fragment; decor takes whichever tier it is mostly on, since an
   * instance is drawn or it is not. That was a `visible` flip on a mesh and
   * is now which tier the chunk contributes.
   */
  private apply_decision(decision: ChunkVisibilityDecision): void {
    apply_visibility(this.registry, decision);
    this.sync_decor(visibility_coverage(decision.secondary) > visibility_coverage(decision.primary) ? "secondary" : "primary");
  }

  private detach_all(): void {
    if (this.attached_cubes.length === 0) return;
    const t0 = import.meta.env.DEV ? performance.now() : 0;
    for (const entry of this.attached_cubes) {
      if (entry.mounted) this.registry.meshes.delete(entry.mounted);
      this.group.remove(entry.mesh);
      entry.mesh.geometry.dispose();
      entry.mesh.geometry = EMPTY_GEOMETRY;
    }
    this.attached_cubes = [];
    if (import.meta.env.DEV) record_timing(mesh_unmount_stats, performance.now() - t0);
  }

  // ---- ChunkLodControl ----

  recheck(camera: THREE.Camera, frustum: THREE.Frustum, lead: THREE.Vector3): void {
    if (this.disposed) return;
    const bounds = this.layout.bounds;
    if (bounds === null) {
      chunk_distance_to_camera(this.group, CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0, camera, this.center);
    } else {
      set_three_box(this.local_box, bounds);
      this.local_box.getCenter(this.center).applyMatrix4(this.group.matrixWorld);
    }
    this.distance2 = this.center.distanceToSquared(camera.position);
    this.lead_distance2 = this.center.distanceToSquared(lead);
    if (bounds === null) {
      const inside = Math.abs(camera.position.x - this.center.x) <= CHUNK_WORLD_SPAN * 0.5 &&
        Math.abs(camera.position.y - this.center.y) <= CHUNK_WORLD_SPAN * 0.5;
      this.in_frustum = inside || frustum.intersectsSphere(this.sphere);
    } else {
      this.world_box.copy(this.local_box).applyMatrix4(this.group.matrixWorld);
      const inside = camera.position.x >= this.world_box.min.x && camera.position.x <= this.world_box.max.x &&
        camera.position.y >= this.world_box.min.y && camera.position.y <= this.world_box.max.y;
      this.in_frustum = inside || frustum.intersectsBox(this.world_box);
    }
    this.group.visible = this.in_frustum;

    if (!this.in_frustum) {
      // Do not let GPU buffers or derived surfaces accumulate as the camera
      // visits the world. An off-screen chunk keeps only its source key and
      // rebuilds if it re-enters.
      this.desired = { primary: OFF_SCREEN_LOD_STEP, secondary: null };
      this.set_pending(this.layout.ready);
      // A scan only means anything while this chunk is going to commit. Left
      // open here it would hold the one scan slot for as long as the chunk
      // stayed off-screen, and every other chunk in the world would report
      // "blocked" — which read as: move somewhere new and nothing builds.
      if (!this.pending) this.abort_work();
      this.blend_active = false;
      this.settling = false;
      this.prune_to_default();
      this.apply_decision({ primary: 1, secondary: 0 });
      return;
    }

    const guide = get_lod_guide(Math.sqrt(this.distance2), this.layout.primary);
    // An unloaded chunk has nothing on screen to transition from: build its
    // correct tier immediately instead of flashing 32 -> 16 -> target merely
    // because the memory-saving empty state uses OFF_SCREEN_LOD_STEP.
    const stepped = this.layout.ready ? step_lod_toward(this.layout.primary, guide.lod_step) : guide.lod_step;
    this.settling = stepped !== guide.lod_step;
    const blending = !this.settling && guide.blend_active &&
      guide.blend_finer_step !== null && guide.blend_coarser_step !== null;
    // Inside a band the finer tier is always primary, whatever guide.lod_step
    // currently says. It crosses from finer to coarser at the band's
    // midpoint, and letting that crossing rename the two tiers made the
    // layout pending exactly halfway through the fade — which abandons the
    // blend and snaps the chunk to one tier until the rebuild lands.
    //
    // That is why zooming out popped while zooming in did not: coarsening
    // commits at the threshold itself, in the middle of the band, while
    // refining commits at the band's own inner edge where the fade is over
    // anyway. Neither direction needs a rebuild for it — the two tiers are
    // the same two meshes either way, only their labels were swapping.
    const primary = blending ? guide.blend_finer_step! : stepped;
    const secondary = blending ? guide.blend_coarser_step! : null;
    this.desired = { primary, secondary };
    this.set_pending(!this.layout.ready || this.layout.primary !== primary || this.layout.secondary !== secondary);
    // Same rule as the off-screen branch: a tier that was being scanned for
    // and is no longer wanted must give the slot back now, not never.
    if (!this.pending) this.abort_work();
    this.blend_active = !this.settling && guide.blend_active;

    if (this.pending || !this.blend_active || guide.blend_finer_step === null || guide.blend_coarser_step === null) {
      this.apply_decision({ primary: 1, secondary: 0 });
      return;
    }
    // Two ways to spend a blend band. Per chunk: a stable hash decides which
    // tier this whole chunk shows, so neighbours cross at different moments
    // and the world does not pop in one frame — cheap, and nothing is ever
    // drawn twice. Per fragment: both tiers draw, punched with complementary
    // dither patterns, which is what engines mean by an LOD cross-fade. The
    // second reads as a real fade and costs the overdraw of drawing a
    // blending chunk twice; it also wants temporal AA to average the pattern,
    // which this renderer does not have (SMAA is spatial), so a moving camera
    // will show the pattern crawling.
    const pair = get_lod_pair_visibility(this.stable_hash, guide.coarse_fraction);
    // The coarser tier leads and the finer one lags, rather than the two
    // splitting the band between them. Complementary patterns only tile
    // without gaps if both tiers fill the same volume, and a coarse box is
    // shrunk to its cell's occupancy — so where it has pulled away from the
    // fine geometry, exclusive patterns would dissolve to background instead
    // of to the other tier. Overlapping them costs the overdraw of a doubly
    // drawn chunk at mid-band and leaves no hole that isn't the coarse tier's
    // own.
    const f = guide.coarse_fraction;
    const finer = pixel_dither ? 2 + Math.max(0, f * 2 - 1) : (pair.finer ? 1 : 0);
    const coarser = pixel_dither ? Math.min(1, f * 2) : (pair.coarser ? 1 : 0);
    const primary_is_finer = this.layout.primary === guide.blend_finer_step;
    this.apply_decision({
      primary: primary_is_finer ? finer : coarser,
      secondary: primary_is_finer ? coarser : finer,
    });
  }

  commit(budget: FrameBudget): boolean {
    if (this.disposed || !this.pending) return false;
    if (!this.in_frustum) {
      this.reset_all_caches();
      // Keep the tiny measured AABB after releasing GPU/surface data so a
      // later polar change can re-enter using real geometry, not z=0.
      this.apply_layout({
        primary: OFF_SCREEN_LOD_STEP,
        secondary: null,
        buckets: [],
        decor: EMPTY_DECOR_PAIR,
        bounds: this.layout.bounds,
        ready: false,
      });
      this.set_pending(false);
      return true;
    }

    const desired = this.desired;
    const build_t0 = import.meta.env.DEV ? performance.now() : 0;
    const buckets = this.build_buckets(desired.primary, desired.secondary, budget);
    if (import.meta.env.DEV) commit_step_stats.build_ms_max = Math.max(commit_step_stats.build_ms_max, performance.now() - build_t0);
    // Not built this frame: the scan or the bake is part-way, or waiting on
    // the slot another chunk holds. Stay pending and
    // leave whatever is on screen alone rather than popping it to empty. A
    // scan that advanced is real work and is reported as such, so the
    // budget loop counts it — the alternative reads as "nothing happened"
    // and lets the loop walk on to start scans it cannot start.
    if (buckets === null) return this.holds_slot;

    const bounds_t0 = import.meta.env.DEV ? performance.now() : 0;
    const bounds = chunk_bounds(buckets);
    if (import.meta.env.DEV) commit_step_stats.bounds_ms_max = Math.max(commit_step_stats.bounds_ms_max, performance.now() - bounds_t0);

    const attach_t0 = import.meta.env.DEV ? performance.now() : 0;
    this.apply_layout({
      primary: desired.primary,
      secondary: desired.secondary,
      buckets,
      decor: this.build_decor_pair(desired.primary, desired.secondary),
      bounds,
      ready: true,
    });
    if (import.meta.env.DEV) commit_step_stats.attach_ms_max = Math.max(commit_step_stats.attach_ms_max, performance.now() - attach_t0);
    this.set_pending(false);
    return true;
  }
}
