// @pxl/web/module/world/chunk_node.ts

/**
 * One chunk's glue half.
 *
 * Every decision about a chunk is the module's (crates/pxl-core/src/world/sched.rs):
 * whether it is on screen, which tiers it shows and how a blend weighs them,
 * what to build and when, what to give back. What stays here is what the
 * decisions act on and the sources they are made from: the tiers and decor
 * blocks the module handed over, and the subscriptions that say a chunk's
 * sources changed.
 *
 * A mesh has exactly one owner at a time — the tier cache, the layout, or
 * both through a count — and a layout takes its hold in the call that
 * applies it, so a layout never names a mesh nothing owns. That rule came
 * out of three reference-counting bugs under React's deferred mounts (the
 * git history around the retired chunk_stage.tsx), and is why this is a
 * plain object.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { CELL_SHAMT } from "@pxl/shared/bake/constant.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { Terrain } from "@pxl/shared/bake/format.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { decor_column_revision, object_column_of, object_column_revision, subscribe_decor_column, subscribe_object_column } from "@/module/world/object/store.ts";
import { register_tile_interest, useTileStore } from "@/module/world/terrain/store.ts";
import { bucket_from_baked, type BuiltChunkBucket, type ChunkLodRole, type ChunkOffsets, release_built_mesh, retain_built_mesh } from "@/render/mesh/chunk_bucket.ts";
import type { BakedChunkMesh } from "@/render/mesh/mesh_block.ts";
import { CORE_BUILD, release_chunk_mesh } from "@/wasm/core_build.ts";
import { decor_block_release, decor_block_retain, type DecorBlock } from "@/render/mesh/decor_block.ts";
import { CORE_SECTIONS } from "@/wasm/core_pool.ts";
import { lod_build_stats } from "@/module/world/lod_build_stat.ts";
export { lod_build_stats };
import type { LodStep } from "@/module/world/lod.ts";

/**
 * How much built work chunks may keep *while off screen*: run sets and
 * cached tiers, both in the mesh section and sized in bytes. Leaving the
 * frustum used to drop all of it on the first release, and a rotation or a
 * zoom in and out rebuilt every chunk from the scan up; the module reclaims
 * only past this, farthest first (issue/20260912-what-to-render-when.md).
 * It budgets only the optional part: on-screen chunks hold their tiers
 * because that is the layout. Half the mesh section: the other half is the
 * on-screen layouts, the volumes a build composes in, and the room a large
 * block needs in one piece — at a quarter of the whole core reservation
 * (128 MB of a 256 MB section) the section sat two-thirds full and
 * fragmented, and refused the fine tiers' blocks 88,000 times in a minute
 * (2026-09-14).
 */
export const RETAIN_BUDGET_BYTES: u32 = Math.floor(CORE_SECTIONS.mesh.bytes / 2);

// ---- Dev-only counters, read by the scheduler's frame sample ----

/** Chunks that appeared, disappeared and applied a layout since the last
 * frame sample, dev-only. */
export const node_counts = { added: 0, removed: 0, rebuilt: 0, released: 0 };

/** The last layouts that landed, dev-only: when, where, which tier — the
 * order the world fills in, read off the console. */
export const landing_log: { t: number; base_x: number; base_y: number; step: LodStep; ready: boolean }[] = [];
const LANDING_LOG_CAP = 2048;

export function take_node_counts(): { added: u32; removed: u32; rebuilt: u32; released: u32 } {
  const out = { ...node_counts };
  Object.assign(node_counts, { added: 0, removed: 0, rebuilt: 0, released: 0 });
  return out;
}

export type ChunkRetention = {
  nodes: u32;
  /** Nodes holding a run set, and what those sets cost in the pool. */
  run_sets: u32;
  run_set_mb: u32;
  /** Built tiers held across every node's cache. */
  tiers: u32;
  /** Decor: cached per-step blocks, and the instances inside them. */
  decor_steps: u32;
  decor_instances: u32;
};

// ---- The live chunks ----

/** Every live node by the module's id. */
export const chunk_nodes = new Map<u32, ChunkNode>();

export function chunk_retention(): ChunkRetention {
  const out: ChunkRetention = { nodes: 0, run_sets: 0, run_set_mb: 0, tiers: 0, decor_steps: 0, decor_instances: 0 };
  let run_bytes = 0;
  for (const node of chunk_nodes.values()) {
    out.nodes++;
    if (node.retained_run_bytes > 0) {
      out.run_sets++;
      run_bytes += node.retained_run_bytes;
    }
    out.tiers += node.retained_tier_cnt;
    for (const decor of node.retained_decor) {
      out.decor_steps++;
      for (const inst of decor.groups) if (inst) out.decor_instances += inst.len;
    }
  }
  out.run_set_mb = Math.round(run_bytes / 1048576);
  return out;
}

/**
 * The world-wide sources, subscribed once for every node rather than once
 * per node: a tile arrival reaches the nodes of its page and no others; a
 * settings change and a decor asset arrival reach every node. The
 * per-column subscriptions stay per node: the store already routes those.
 */
useSettingsStore.subscribe(() => {
  for (const node of chunk_nodes.values()) {
    node.on_source_change();
    node.on_decor_change();
  }
});
register_tile_interest(() => true, (page_idx) => {
  for (const node of chunk_nodes.values()) if (node.page_idx === page_idx) node.on_source_change();
});
// ---- Layout ----

/**
 * The decor blocks of the layout's two tiers; `secondary` mirrors the
 * layout's (null outside a blend band) rather than decor tracking its own
 * blend. A tier's block is null while the module has none for it.
 */
type DecorLayoutPair = {
  primary: DecorBlock | null;
  secondary: DecorBlock | null;
};
const EMPTY_DECOR_PAIR: DecorLayoutPair = { primary: null, secondary: null };

/** The layout's own hold on the blocks it shows, taken and dropped as one
 * with the buckets' — see apply_layout. */
function retain_decor_pair(pair: DecorLayoutPair): void {
  if (pair.primary) decor_block_retain(pair.primary);
  if (pair.secondary) decor_block_retain(pair.secondary);
}
function release_decor_pair(pair: DecorLayoutPair): void {
  if (pair.primary) decor_block_release(pair.primary);
  if (pair.secondary) decor_block_release(pair.secondary);
}

type LodLayout = {
  primary: LodStep;
  secondary: LodStep | null;
  buckets: BuiltChunkBucket[];
  decor: DecorLayoutPair;
  ready: boolean;
};

/** The hash that picks which side of a per-chunk blend a chunk falls on, so
 * neighbours cross at different moments. */
function blend_hash(key: string): u32 {
  let hash = 2166136261;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

export type ChunkNodeDeps = {
  page_idx: IVec;
  offset: ChunkOffsets;
  /** The id the module mounted the chunk under; without one the node adds
   * its own chunk to the module. */
  id?: u32;
};

export class ChunkNode {
  /** The page this chunk belongs to, for the page's own mount diff. */
  readonly page_idx: IVec;
  /** This chunk's own absolute lattice origin. */
  readonly base_i: IVec;

  /** The object store's key for this chunk's column. */
  readonly object_key: IVec;
  /** The module's id for this chunk. */
  private readonly id: u32;

  /** The tiers and the decor blocks the module handed over, by step. */
  private readonly bucket_cache = new Map<LodStep, BuiltChunkBucket>();
  private readonly decor_cache = new Map<LodStep, DecorBlock>();
  /** How many times the module has been told this chunk's sources: a tier
   * built from an earlier telling is stale and released on arrival. */
  private generation: u32 = 0;
  /** What this chunk's runs hold in the mesh section, as the module said. */
  private runs_bytes: u32 = 0;

  private layout: LodLayout = { primary: 64, secondary: null, buckets: [], decor: EMPTY_DECOR_PAIR, ready: false };
  private shown = false;
  /** The blend the module last told for this layout. The module does not
   * repeat an unchanged one, so a layout this node re-attaches on its own
   * (decor changing) puts it back rather than showing the defaults. */
  private blend: { primary: number; secondary: number; decor_secondary: boolean } | null = null;

  private tile: unknown;
  private show_structures: boolean;
  private objects_revision: number;
  private decor_revision: number;
  private show_decor: boolean;

  private readonly unsubscribes: (() => void)[] = [];

  private disposed = false;

  constructor(private readonly deps: ChunkNodeDeps) {
    const { page_idx, offset } = deps;
    this.page_idx = page_idx;
    const page_off = ivec.gather(BigInt(offset.base_x), BigInt(offset.base_y), 0n);
    this.object_key = object_column_of(page_idx, page_off);
    this.base_i = ivec.add(FROM_L3_IDX(page_idx), page_off);

    this.tile = useTileStore.getState().get_tile(page_idx);
    this.show_structures = useSettingsStore.getState().show_structures;
    this.objects_revision = object_column_revision(this.object_key);
    this.decor_revision = decor_column_revision(this.object_key);
    this.show_decor = useSettingsStore.getState().show_decor;

    // Direct store subscriptions rather than hooks: a node is not a
    // component, and a placement should not have to travel through a render
    // to reach the chunk it changed.
    this.unsubscribes.push(
      subscribe_object_column(this.object_key, () => this.on_source_change()),
      subscribe_decor_column(this.object_key, () => this.on_decor_change()),
    );

    if (deps.id !== undefined) {
      this.id = deps.id;
      CORE_BUILD.attach(this.id, this);
    } else {
      this.id = CORE_BUILD.add(
        this,
        this.base_i,
        ivec.to_real(ivec.X(this.base_i)),
        ivec.to_real(ivec.Y(this.base_i)),
        Number(ivec.X(page_off)) << -CELL_SHAMT,
        Number(ivec.Y(page_off)) << -CELL_SHAMT,
        blend_hash(`${page_idx.toString()}:${offset.key}`),
      );
    }
    this.tell_source();
    chunk_nodes.set(this.id, this);

    if (import.meta.env.DEV) node_counts.added++;
  }

  /** The module unmounted this chunk: everything it held there is gone. */
  on_unmount(): void {
    this.dispose(false);
  }

  dispose(tell_module: boolean = true): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const off of this.unsubscribes) off();
    if (tell_module) CORE_BUILD.remove(this.id);
    chunk_nodes.delete(this.id);
    for (const bucket of this.layout.buckets) release_built_mesh(bucket.cubes);
    release_decor_pair(this.layout.decor);
    this.layout = { ...this.layout, buckets: [], decor: EMPTY_DECOR_PAIR, ready: false };
    this.release_bucket_cache();
    this.clear_decor_cache();
    if (import.meta.env.DEV) node_counts.removed++;
  }

  /** Read by chunk_retention; dev-only. */
  get retained_run_bytes(): u32 {
    return this.runs_bytes;
  }
  get retained_tier_cnt(): u32 {
    return this.bucket_cache.size;
  }
  get retained_decor(): Iterable<DecorBlock> {
    return this.decor_cache.values();
  }

  // ---- the module's calls ----

  on_tier(generation: u32, step: LodStep, mesh: BakedChunkMesh): void {
    if (this.disposed || generation !== this.generation || this.bucket_cache.has(step)) {
      release_chunk_mesh(mesh);
      return;
    }
    this.bucket_cache.set(step, bucket_from_baked(mesh, step, "primary"));
  }

  on_runs(bytes: u32): void {
    this.runs_bytes = bytes;
  }

  on_visible(on: boolean): void {
    this.shown = on;
  }

  /** The module hands a step's decor over only where it believes this node
   * holds none, so one already held is stale (a rebuild this node started
   * with tell_source) and gives way. */
  on_decor(step: LodStep, decor: DecorBlock): void {
    if (this.disposed) {
      decor_block_release(decor);
      return;
    }
    const held = this.decor_cache.get(step);
    if (held) decor_block_release(held);
    this.decor_cache.set(step, decor);
  }

  on_evict(step: LodStep): void {
    const bucket = this.bucket_cache.get(step);
    if (bucket) {
      this.bucket_cache.delete(step);
      release_built_mesh(bucket.cubes);
    }
    const decor = this.decor_cache.get(step);
    if (decor) {
      this.decor_cache.delete(step);
      decor_block_release(decor);
    }
  }

  on_layout(primary: LodStep, secondary: LodStep | null, ready: boolean): void {
    if (this.disposed) return;
    if (!ready) {
      this.blend = null;
      this.apply_layout({ primary, secondary: null, buckets: [], decor: EMPTY_DECOR_PAIR, ready: false });
      if (import.meta.env.DEV) node_counts.released++;
      return;
    }
    const steps = secondary === null ? [primary] : [primary, secondary];
    const buckets = steps
      .map((step) => this.bucket_cache.get(step))
      .filter((bucket): bucket is BuiltChunkBucket => bucket !== undefined)
      .map((bucket) => ({ ...bucket, role: (bucket.lod_step === primary ? "primary" : "secondary") as ChunkLodRole }))
      .sort((a, b) => a.lod_step - b.lod_step);
    if (buckets.length !== steps.length) {
      // The module and this node disagree about what is held. Keep what is
      // on screen and start the chunk over from its sources, which tells the
      // module to forget its tiers and build the layout again.
      // After the completion ring's read this call is inside, which a
      // rebuild would re-enter.
      console.warn(`[render/chunk] the module laid out tiers ${steps} of chunk ${this.id} this node does not hold; rebuilding it`);
      queueMicrotask(() => {
        if (this.disposed) return;
        this.release_bucket_cache();
        this.clear_decor_cache();
        this.tell_source();
        CORE_BUILD.build_now(this.id);
      });
      return;
    }
    if (import.meta.env.DEV && landing_log.length < LANDING_LOG_CAP) {
      landing_log.push({ t: performance.now(), base_x: this.deps.offset.base_x, base_y: this.deps.offset.base_y, step: primary, ready: true });
    }
    const decor = this.decor_pair(primary, secondary);
    // The same tiers told again with the same meshes is the module's decor
    // rebuild (decor_now): the cubes stay attached, or the rebuild would be
    // the flicker the decor/solid split exists to avoid.
    const same_tiers = this.layout.ready && this.layout.buckets.length === buckets.length &&
      buckets.every((b, k) => b.cubes === this.layout.buckets[k].cubes && b.role === this.layout.buckets[k].role);
    if (same_tiers) {
      this.apply_decor(decor);
      return;
    }
    this.blend = null;
    this.apply_layout({ primary, secondary, buckets, decor, ready: true });
  }

  /** One decision, two consumers. Cube content dithers between the two tiers
   * per fragment; decor takes whichever tier the module says it is mostly
   * on, since an instance is drawn or it is not. */
  on_blend(primary: number, secondary: number, decor_secondary: boolean): void {
    this.blend = { primary, secondary, decor_secondary };
  }

  // ---- source changes ----

  /** Tells the module what this chunk is made of now. */
  private tell_source(): void {
    const tile = this.tile as { terrain: Terrain } | null | undefined;
    CORE_BUILD.source(this.id, tile?.terrain ?? null, this.show_structures);
    this.generation++;
  }

  /**
   * A source revision rebuilds what the chunk shows, not the whole visible
   * world: the module is told, and builds this chunk's layout to the end at
   * once, so a placement shows in the frame it was made. `objects_revision`
   * is the SOLID-only revision; a solid change still clears decor's cache,
   * because a newly placed cube can occlude a scattered decor candidate.
   */
  on_source_change(): void {
    if (this.disposed) return;
    const tile = useTileStore.getState().get_tile(this.deps.page_idx);
    const show_structures = useSettingsStore.getState().show_structures;
    const objects_revision = object_column_revision(this.object_key);
    if (Object.is(this.tile, tile) && this.show_structures === show_structures && this.objects_revision === objects_revision) return;
    this.tile = tile;
    this.show_structures = show_structures;
    this.objects_revision = objects_revision;
    this.release_bucket_cache();
    this.clear_decor_cache();
    this.tell_source();
    CORE_BUILD.build_now(this.id);
  }

  /**
   * Decor's own revision-only rebuild, in the module. It must never touch
   * the tiers: that would reintroduce the needless cube rebuild — visible as
   * a flicker — this split exists to avoid.
   */
  on_decor_change(): void {
    if (this.disposed) return;
    const revision = decor_column_revision(this.object_key);
    const show_decor = useSettingsStore.getState().show_decor;
    if (revision === this.decor_revision && show_decor === this.show_decor) return;
    this.decor_revision = revision;
    this.show_decor = show_decor;
    this.clear_decor_cache();
    CORE_BUILD.decor_now(this.id, show_decor);
  }

  // ---- caches ----

  private release_bucket_cache(): void {
    for (const bucket of this.bucket_cache.values()) release_built_mesh(bucket.cubes);
    this.bucket_cache.clear();
  }

  /** Drops the cache's hold on every block it has; a block the layout is
   * still showing lives on until the layout lets go. */
  private clear_decor_cache(): void {
    for (const decor of this.decor_cache.values()) decor_block_release(decor);
    this.decor_cache.clear();
  }

  /** The layout's decor from the cache: the module hands a tier's decor
   * over before the layout, so a tier without one has none. */
  private decor_pair(primary: LodStep, secondary: LodStep | null): DecorLayoutPair {
    return {
      primary: this.decor_cache.get(primary) ?? null,
      secondary: secondary !== null && secondary !== primary ? this.decor_cache.get(secondary) ?? null : null,
    };
  }

  // ---- attaching ----

  /**
   * The one place the layout changes: the new layout's meshes are attached
   * here, synchronously, so there is never a moment where a layout names
   * meshes nothing owns. Retain-before-release, so a bucket carried across
   * two layouts never transiently reaches zero.
   */
  private apply_layout(next: LodLayout): void {
    const previous = this.layout;
    for (const bucket of next.buckets) retain_built_mesh(bucket.cubes);
    retain_decor_pair(next.decor);
    this.layout = next;
    for (const bucket of previous.buckets) release_built_mesh(bucket.cubes);
    release_decor_pair(previous.decor);
    if (import.meta.env.DEV) node_counts.rebuilt++;
  }

  /** The layout's decor alone: the cubes stay as they are. */
  private apply_decor(decor: DecorLayoutPair): void {
    const previous = this.layout.decor;
    retain_decor_pair(decor);
    this.layout = { ...this.layout, decor };
    release_decor_pair(previous);
  }
}
