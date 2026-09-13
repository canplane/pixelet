// @pxl/web/module/world/chunk_node.ts

/**
 * One chunk's scene half.
 *
 * Every decision about a chunk is the module's (crates/pxl-core/src/sched.rs):
 * whether it is on screen, which tiers it shows and how a blend weighs them,
 * what to build and when, what to give back. What stays here is what the
 * decisions act on and the sources they are made from: the `THREE.Group` and
 * the meshes attached to it, the tiers the module handed over, decor's build
 * and its contribution to decor/field.ts, and the subscriptions that say a
 * chunk's sources changed.
 *
 * A mesh has exactly one owner at a time — the tier cache, the layout, or
 * both through a count — and a layout's meshes are attached in the call that
 * applies it, so a layout never names a mesh nothing owns. That rule came
 * out of three reference-counting bugs under React's deferred mounts (the
 * git history around chunk_stage.tsx), and is why this is a plain object.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CELL_SHAMT, CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { Terrain } from "@pxl/shared/bake/format.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { decor_field, type DecorContribution, type DecorVariantContribution, variant_key } from "@/module/world/decor/field.ts";
import { build_decor_instances, type DecorInstancesByVariant } from "@/module/world/decor/decor_layer.ts";
import { instances_release, instances_retain } from "@/module/world/decor/instance.ts";
import { decor_bundle_assets, type DecorBundleAssets, subscribe_decor_assets } from "@/module/world/decor/asset_store.ts";
import { decor_column_revision, object_column_of, object_column_revision, subscribe_decor_column, subscribe_object_column } from "@/module/world/object/store.ts";
import { chunk_material, create_geometry, VISIBLE_ATTR } from "@/module/world/render/chunk_material.ts";
import { register_tile_interest, useTileStore } from "@/module/world/terrain/store.ts";
import { bucket_from_baked, type BuiltChunkBucket, type ChunkLodRole, type ChunkOffsets, mesh_bounds, release_built_mesh, retain_built_mesh } from "@/render/layer/pipeline/chunk_bucket.ts";
import type { BakedChunkMesh } from "@/render/layer/pipeline/chunk_mesher.ts";
import { CORE_BUILD, release_chunk_mesh } from "@/render/layer/pipeline/core_build.ts";
import { CORE_MEMORY } from "@/render/layer/pipeline/core_pool.ts";
import { lod_build_stats } from "@/module/world/lod_build_stat.ts";
export { lod_build_stats };
import type { LodStep } from "@/render/lod.ts";
import { apply_visibility, type ChunkMeshRegistry, type MountedLodMesh } from "@/render/lod_visibility.ts";

/**
 * How much built work chunks may keep *while off screen*: run sets and
 * cached tiers, both in the core pool and sized in bytes. Leaving the frustum
 * used to drop all of it on the first release, and a rotation or a zoom in
 * and out rebuilt every chunk from the scan up; the module reclaims only past
 * this, farthest first (issue/20260912-what-to-render-when.md). It budgets
 * only the optional part: on-screen chunks hold their tiers because that is
 * the layout. A quarter of the core reservation, the currency the crash case
 * was measured in.
 */
export const RETAIN_BUDGET_BYTES: u32 = Math.floor((CORE_MEMORY?.buffer.byteLength ?? 256 * 1048576) / 4);

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

// ---- Dev-only counters, read by the scheduler's frame sample ----

/** Chunks that appeared, disappeared and applied a layout since the last
 * frame sample, dev-only. */
export const node_counts = { added: 0, removed: 0, rebuilt: 0, released: 0 };

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
  /** Decor: cached per-step builds, and the lane instances inside them. */
  decor_steps: u32;
  decor_instances: u32;
};

// ---- The live chunks ----

/** Every live node by the module's id, and by its absolute base. */
export const chunk_nodes = new Map<u32, ChunkNode>();
const by_base = new Map<IVec, ChunkNode>();

export function chunk_at(base: IVec): ChunkNode | undefined {
  return by_base.get(base);
}

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
    for (const per_bundle of node.retained_decor) {
      out.decor_steps++;
      for (const variants of per_bundle) for (const inst of variants) if (inst) out.decor_instances += inst.len;
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
subscribe_decor_assets(() => {
  for (const node of chunk_nodes.values()) node.on_decor_assets();
});

// ---- Layout ----

/**
 * One entry per bundle (matching bundle_assets' own order), each itself one
 * entry per that bundle's own variant. `secondary` mirrors the layout's
 * (null outside a blend band) rather than decor tracking its own blend.
 */
type DecorLayoutPair = {
  primary: DecorInstancesByVariant[];
  secondary: DecorInstancesByVariant[] | null;
};
const EMPTY_DECOR_PAIR: DecorLayoutPair = { primary: [], secondary: null };

/** The layout's own hold on the lane sets it shows, taken and dropped as one
 * with the buckets' — see apply_layout. */
function retain_decor_pair(pair: DecorLayoutPair): void {
  for (const variants of pair.primary) for (const inst of variants) if (inst) instances_retain(inst);
  if (pair.secondary) { for (const variants of pair.secondary) for (const inst of variants) if (inst) instances_retain(inst); }
}
function release_decor_pair(pair: DecorLayoutPair): void {
  for (const variants of pair.primary) for (const inst of variants) if (inst) instances_release(inst);
  if (pair.secondary) { for (const variants of pair.secondary) for (const inst of variants) if (inst) instances_release(inst); }
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

/** Shared placeholder a mesh holds after its geometry is disposed — never
 * uploaded, never mutated. */
const EMPTY_GEOMETRY = new THREE.BufferGeometry();

/** One attached cube mesh: the scene object, the mesh data behind it, and
 * the registry entry the LOD blend flips. Created and destroyed as one. */
type AttachedCubes = {
  mesh: THREE.Mesh;
  bucket: BuiltChunkBucket;
  mounted: MountedLodMesh | null;
};

export type ChunkNodeDeps = {
  page_idx: IVec;
  offset: ChunkOffsets;
};

export class ChunkNode {
  /** This chunk's own scene node, positioned at its page-local origin. */
  readonly group: THREE.Group;
  /** The page this chunk belongs to, for the page's own mount diff. */
  readonly page_idx: IVec;
  /** This chunk's own absolute lattice origin — its identity in
   * decor/field.ts, and the offset chunk-local instances are placed at. */
  readonly base_i: IVec;

  private readonly object_key: IVec;
  private readonly registry: ChunkMeshRegistry = { meshes: new Set() };
  /** The module's id for this chunk. */
  private readonly id: u32;

  /** The tiers the module handed over, by step. */
  private readonly bucket_cache = new Map<LodStep, BuiltChunkBucket>();
  private readonly decor_cache = new Map<LodStep, DecorInstancesByVariant[]>();
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
  private bundle_assets: readonly DecorBundleAssets[];

  private attached_cubes: AttachedCubes[] = [];
  private readonly unsubscribes: (() => void)[] = [];

  /** What this chunk last told decor/field.ts. `dirty` forces the next sync
   * through even when neither of the other two moved — a rebuilt layout is
   * the same chunk showing the same tier, with different instances in it. */
  private decor_shown = false;
  private decor_role: ChunkLodRole = "primary";
  private decor_dirty = true;

  private disposed = false;

  constructor(private readonly deps: ChunkNodeDeps) {
    const { page_idx, offset } = deps;
    this.page_idx = page_idx;
    const page_off = ivec.gather(BigInt(offset.base_x), BigInt(offset.base_y), 0n);
    this.object_key = object_column_of(page_idx, page_off);
    this.base_i = ivec.add(FROM_L3_IDX(page_idx), page_off);

    this.group = new THREE.Group();
    this.group.name = `chunk/${offset.key}`;
    this.group.position.set(offset.base_x * U_PER_CELL, offset.base_y * U_PER_CELL, 0);
    this.group.visible = false;

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
      subscribe_object_column(this.object_key, () => this.on_source_change()),
      subscribe_decor_column(this.object_key, () => this.on_decor_change()),
    );

    this.id = CORE_BUILD.add(
      this,
      this.base_i,
      ivec.to_real(ivec.X(this.base_i)),
      ivec.to_real(ivec.Y(this.base_i)),
      Number(ivec.X(page_off)) << -CELL_SHAMT,
      Number(ivec.Y(page_off)) << -CELL_SHAMT,
      blend_hash(`${page_idx.toString()}:${offset.key}`),
    );
    this.tell_source();
    chunk_nodes.set(this.id, this);
    by_base.set(this.base_i, this);

    if (import.meta.env.DEV) node_counts.added++;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const off of this.unsubscribes) off();
    CORE_BUILD.remove(this.id);
    chunk_nodes.delete(this.id);
    by_base.delete(this.base_i);
    decor_field.clear_chunk(this.base_i);
    this.detach_all();
    for (const bucket of this.layout.buckets) release_built_mesh(bucket.cubes);
    release_decor_pair(this.layout.decor);
    this.layout = { ...this.layout, buckets: [], decor: EMPTY_DECOR_PAIR, ready: false };
    this.release_bucket_cache();
    this.clear_decor_cache();
    this.group.removeFromParent();
    if (import.meta.env.DEV) node_counts.removed++;
  }

  /** Read by chunk_retention; dev-only. */
  get retained_run_bytes(): u32 {
    return this.runs_bytes;
  }
  get retained_tier_cnt(): u32 {
    return this.bucket_cache.size;
  }
  get retained_decor(): Iterable<DecorInstancesByVariant[]> {
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
    this.group.visible = on;
    this.sync_decor();
  }

  on_evict(step: LodStep): void {
    const bucket = this.bucket_cache.get(step);
    if (!bucket) return;
    this.bucket_cache.delete(step);
    release_built_mesh(bucket.cubes);
    if (!this.shown) this.clear_decor_cache();
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
        this.tell_source();
        CORE_BUILD.build_now(this.id);
      });
      return;
    }
    this.blend = null;
    this.apply_layout({ primary, secondary, buckets, decor: this.build_decor_pair(primary, secondary), ready: true });
  }

  /** One decision, two consumers. Cube content dithers between the two tiers
   * per fragment; decor takes whichever tier the module says it is mostly
   * on, since an instance is drawn or it is not. */
  on_blend(primary: number, secondary: number, decor_secondary: boolean): void {
    this.blend = { primary, secondary, decor_secondary };
    apply_visibility(this.registry, { primary, secondary });
    this.sync_decor(decor_secondary ? "secondary" : "primary");
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
   * Decor models finish loading well after the first chunks are built, and
   * a node that never heard about it drew no decor for the rest of its
   * life. Decor meshes only: a model arriving says nothing about cube content.
   */
  on_decor_assets(): void {
    if (this.disposed) return;
    const assets = decor_bundle_assets();
    if (this.bundle_assets === assets) return;
    this.bundle_assets = assets;
    this.clear_decor_cache();
    if (!this.layout.ready) return;
    this.apply_layout({ ...this.layout, decor: this.build_decor_pair(this.layout.primary, this.layout.secondary) });
  }

  /**
   * Decor's own revision-only rebuild. It must never touch the tiers: that
   * would reintroduce the needless cube rebuild — visible as a flicker —
   * this split exists to avoid.
   */
  on_decor_change(): void {
    if (this.disposed) return;
    const revision = decor_column_revision(this.object_key);
    const show_decor = useSettingsStore.getState().show_decor;
    if (revision === this.decor_revision && show_decor === this.show_decor) return;
    this.decor_revision = revision;
    this.show_decor = show_decor;
    this.clear_decor_cache();
    if (!this.layout.ready) return;
    this.apply_layout({ ...this.layout, decor: this.build_decor_pair(this.layout.primary, this.layout.secondary) });
  }

  // ---- caches ----

  private release_bucket_cache(): void {
    for (const bucket of this.bucket_cache.values()) release_built_mesh(bucket.cubes);
    this.bucket_cache.clear();
  }

  /** Drops the cache's hold on every lane set it has; a set the layout is
   * still showing lives on until the layout lets go. */
  private clear_decor_cache(): void {
    for (const per_bundle of this.decor_cache.values()) {
      for (const variants of per_bundle) for (const inst of variants) if (inst) instances_release(inst);
    }
    this.decor_cache.clear();
  }

  /** Decor's own build — no dependency on the cube tiers at all (a decor
   * entry never paints a voxel). */
  private build_decor_step(step: LodStep): DecorInstancesByVariant[] {
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
    this.detach_all();
    for (const bucket of next.buckets) if (bucket.cubes.vertex_cnt > 0) this.attach_cubes(bucket);
    this.decor_dirty = true;
    if (this.blend !== null) {
      apply_visibility(this.registry, this.blend);
      this.sync_decor(this.blend.decor_secondary ? "secondary" : "primary");
    } else {
      this.sync_decor();
    }
    for (const bucket of previous.buckets) release_built_mesh(bucket.cubes);
    release_decor_pair(previous.decor);
    if (import.meta.env.DEV) node_counts.rebuilt++;
  }

  private attach_cubes(bucket: BuiltChunkBucket): void {
    const built = bucket.cubes;
    const geometry = create_geometry(built);
    const mesh = new THREE.Mesh(geometry, chunk_material());
    // Unconditional: the real cost is gated at the renderer level by the
    // Canvas's own `shadows` prop.
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.name = `${bucket.role}/lod/${bucket.lod_step}`;
    built.visibility.fill(bucket.role === "primary" ? 1 : 0);

    // Not geometry.computeBoundingBox(): mesh_bounds' direct scan over the
    // baked positions is exact and far cheaper on a rebuild storm.
    const bounds = mesh_bounds(built);
    if (bounds) {
      geometry.boundingBox = new THREE.Box3(new THREE.Vector3(bounds.min_x, bounds.min_y, bounds.min_z), new THREE.Vector3(bounds.max_x, bounds.max_y, bounds.max_z));
      const center = geometry.boundingBox.getCenter(new THREE.Vector3());
      geometry.boundingSphere = new THREE.Sphere(center, center.distanceTo(geometry.boundingBox.max));
    } else {
      geometry.boundingBox = new THREE.Box3();
      geometry.boundingSphere = new THREE.Sphere(new THREE.Vector3(CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0), 0);
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
   * Tells decor/field.ts what this chunk shows right now, if that changed:
   * whether the chunk is on screen, which tier it shows, and whether its
   * instances were rebuilt. Nothing else is this chunk's business.
   */
  private sync_decor(want_role: ChunkLodRole = this.decor_role): void {
    const shown = this.shown;
    const role: ChunkLodRole = this.layout.decor.secondary !== null ? want_role : "primary";
    if (!this.decor_dirty && shown === this.decor_shown && role === this.decor_role) return;
    this.decor_dirty = false;
    this.decor_shown = shown;
    this.decor_role = role;
    if (!shown) {
      decor_field.clear_chunk(this.base_i);
      return;
    }
    const source = role === "secondary" ? this.layout.decor.secondary! : this.layout.decor.primary;
    const contribution: Map<string, DecorVariantContribution> = new Map();
    this.bundle_assets.forEach(({ bundle, geometries, materials }, bi) => {
      const variants: DecorInstancesByVariant | undefined = source[bi];
      if (!variants) return;
      geometries.forEach((geometry, variant) => {
        const material = materials[variant];
        const instances = variants[variant];
        // A variant whose model or skin is still in flight contributes
        // nothing; the arrival rebuilds this chunk's decor and lands here.
        if (!geometry || !material || !instances) return;
        contribution.set(variant_key(bundle.kind, variant), { geometry, material, instances });
      });
    });
    decor_field.set_chunk(this.base_i, contribution as DecorContribution);
  }

  private detach_all(): void {
    if (this.attached_cubes.length === 0) return;
    for (const entry of this.attached_cubes) {
      if (entry.mounted) this.registry.meshes.delete(entry.mounted);
      this.group.remove(entry.mesh);
      entry.mesh.geometry.dispose();
      entry.mesh.geometry = EMPTY_GEOMETRY;
    }
    this.attached_cubes = [];
  }
}
