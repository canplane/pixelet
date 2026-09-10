// @pxl/web/modules/world/decor/field.ts

/**
 * Every decor model in the world, as one instanced mesh per variant.
 *
 * What this replaces created a `THREE.Mesh` per (chunk, variant, LOD role),
 * plus a second undrawn one per chunk so the eraser could still raycast what
 * it was hiding, and made and destroyed them as chunks mounted, as LOD tiers
 * changed, and again whenever an asset finished loading. A hundred and forty
 * meshes, continuously churning, each owned by whichever chunk happened to
 * build it — and each one baked from real vertex copies of its instances, so
 * planting one tree remerged a whole chunk's worth of geometry.
 *
 * Here an instance is sixteen floats. There is one mesh per variant, and
 * chunks do not own meshes at all — they contribute instances.
 *
 * **A chunk owns a contiguous slice, and writes only that.** The instance
 * buffer is divided into per-chunk runs; a chunk that changes rewrites its
 * own run and hands three that byte range through `addUpdateRange`, so
 * neither the CPU cost nor the upload scales with the rest of the world. A
 * run is allocated at a power-of-two capacity and freed back to a size class,
 * so a chunk whose count wobbles by one reuses what it has. Unused slots
 * inside a live run, and runs that have been freed, hold a zero-scale matrix:
 * they are still drawn, and they rasterise nothing.
 *
 * That fragmentation is the price of locality, so it is bounded rather than
 * ignored — a bank whose allocated length has drifted to twice its live count
 * compacts, which is a memmove of the runs it already holds, not a rebuild of
 * the matrices in them.
 *
 * **The frame boundary is still the only place buffers are written.** A
 * contribution change marks a chunk dirty and nothing else; `update` runs
 * once a frame, from the render loop, and is the only writer. An asset
 * arriving or a chunk committing can never run code inside someone else's
 * update.
 *
 * LOD stops being a second mechanism. A chunk contributes what should be
 * drawn right now, so a tier change is a different contribution rather than a
 * visibility flag on a mesh that exists either way, and a chunk that leaves
 * the frustum withdraws rather than being hidden.
 *
 * **The geometry and material ride along with the instances** instead of
 * being looked up here. Partly so this module stays importable by
 * `deno test` — asset_store.ts reaches settings_store.ts and its `*.glb?url`
 * imports, which is the same wall instanced_model.ts's own header describes —
 * and partly because it removes the second arrival path: a chunk already
 * rebuilds when an asset lands, so an asset that has not arrived is simply a
 * variant nobody contributes, not a state this has to poll for.
 *
 * What is still linear in the whole world is hit testing:
 * `THREE.InstancedMesh.raycast` walks every instance. The fix for that is
 * ds/spatial_hash.ts rather than anything here — decor placement is already a
 * lattice fact — and it is worth doing when decor is dense enough to notice,
 * not before.
 */

import * as THREE from "three";

import type { u32 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { compose_instance_matrix, type ModelInstanceTransform } from "./instanced_model.ts";

/** Identifies one variant across every bundle. */
export type VariantKey = string;
export const variant_key = (bundle: u32, variant: u32): VariantKey => `${bundle}:${variant}`;

export type DecorVariantContribution = {
  geometry: THREE.BufferGeometry;
  material: THREE.Material;
  instances: readonly ModelInstanceTransform[];
};

/** What one chunk contributes. Held rather than merged so a chunk can be
 * replaced or withdrawn without anything else being recomputed. */
export type DecorContribution = ReadonlyMap<VariantKey, DecorVariantContribution>;

type Chunk = {
  /** The chunk's own absolute integer origin, in PVF3 lattice units. Kept as
   * the lattice value rather than a float so rebasing on a new floating
   * origin is an exact integer subtraction. */
  base: IVec;
  contribution: DecorContribution;
};

/**
 * A variant's drawn instances, or its hit-only ones — see `Bank.drawn`. Two
 * banks per variant at most, addressed the same way, so a chunk's placement
 * is one flat set of bank keys rather than two parallel structures.
 */
type BankKey = string;
const HIDDEN_SUFFIX = "#hidden";

/** Instances a bank starts with room for, and the smallest run one chunk can
 * hold. A run rounds up to a power of two from here, which is what lets a
 * freed one be reused by the next chunk of roughly that size. */
const INITIAL_CAPACITY: u32 = 256;
const MIN_SLICE: u32 = 8;

/** Compact once the allocated length has drifted to this multiple of what is
 * actually drawn. Below the floor the waste is not worth a memmove. */
const COMPACT_RATIO = 2;
const COMPACT_FLOOR: u32 = 64;

/** One chunk's run inside a bank: where it starts, how much it was given, and
 * how much of that it is currently using. */
type Slice = { start: u32; cap: u32; count: u32 };

type Bank = {
  key: BankKey;
  mesh: THREE.InstancedMesh;
  geometry: THREE.BufferGeometry;
  /**
   * False for the bank that is never rendered and still the real hit target.
   *
   * An entry the eraser hovers is withheld from the drawn buffer so whatever
   * sits behind it becomes visible, but it has to stay hit-testable: a hover
   * that loses its own target releases the hide, which brings the entry back,
   * which lets the hover find it again — hide, miss, release, re-acquire.
   * `THREE.Raycaster` does not consult `visible`, which is what makes a
   * second, invisible mesh the whole of the fix.
   */
  drawn: boolean;
  capacity: u32;
  /** First index never handed to a run. `mesh.count` tracks this. */
  end: u32;
  /** Instances actually being drawn, against which `end` is judged. */
  live: u32;
  slices: Map<string, Slice>;
  /** Freed run starts by capacity class. */
  free: Map<u32, u32[]>;
  /** The transform behind each index, or null for a slot holding nothing —
   * this is what turns a raycast's `instanceId` back into something the
   * eraser can act on. */
  source: (ModelInstanceTransform | null)[];
};

const $matrix = new THREE.Matrix4();
/** A slot that draws nothing: every vertex collapses onto one point, so the
 * triangles have no area and the rasteriser produces no fragments. */
const $vacant = new THREE.Matrix4().makeScale(0, 0, 0);

const slice_cap = (need: u32): u32 => {
  let cap = MIN_SLICE;
  while (cap < need) cap *= 2;
  return cap;
};

export class DecorField {
  readonly group = new THREE.Group();

  private readonly banks = new Map<BankKey, Bank>();
  private readonly chunks = new Map<string, Chunk>();
  /** Which banks each chunk currently holds a run in, so a chunk that stops
   * contributing a variant can be found to release it. */
  private readonly placement = new Map<string, Set<BankKey>>();
  private readonly dirty = new Set<string>();

  /** The floating origin the live buffers are expressed against. A shift
   * rebases everything, which is rare — L4 is 4096u — and keeps the floats
   * the GPU sees small, which is the whole point of having an origin. */
  private packed_origin: IVec = 0n;
  private origin_known = false;

  constructor() {
    this.group.name = "decor-field";
    // One mesh spans the whole loaded world, so its bounds are the world's:
    // there is nothing for a frustum test to reject. Chunks withdraw when
    // they leave the frustum, which is where the culling actually happens.
    this.group.frustumCulled = false;
  }

  /**
   * Writes every dirty chunk's run. Called once a frame, from the render
   * loop, and the only place instance buffers are written.
   */
  update(map_origin: IVec): void {
    if (!this.origin_known || map_origin !== this.packed_origin) {
      this.rebase(map_origin);
    }
    if (this.dirty.size === 0) return;
    for (const key of this.dirty) this.sync(key);
    this.dirty.clear();
    for (const bank of this.banks.values()) {
      if (bank.end > COMPACT_FLOOR && bank.end > bank.live * COMPACT_RATIO) this.compact(bank);
    }
  }

  /**
   * Replaces one chunk's contribution.
   *
   * `base` is the chunk's own absolute lattice origin; the instances inside
   * are chunk-local, exactly as they are built. Anything absent from
   * `contribution` is absent from the world, so a chunk withdraws by clearing
   * rather than by contributing something empty.
   */
  set_chunk(key: string, base: IVec, contribution: DecorContribution): void {
    this.chunks.set(key, { base, contribution });
    this.dirty.add(key);
  }

  clear_chunk(key: string): void {
    if (this.chunks.delete(key)) this.dirty.add(key);
  }

  /** Every mesh a raycast should consider, drawn or hit-only. */
  hit_meshes(): THREE.InstancedMesh[] {
    const out: THREE.InstancedMesh[] = [];
    for (const bank of this.banks.values()) {
      if (bank.live > 0) out.push(bank.mesh);
    }
    return out;
  }

  /** Resolves a raycast hit back to the instance that produced it. */
  instance_at(mesh: THREE.Object3D, instance_id: u32): ModelInstanceTransform | null {
    for (const bank of this.banks.values()) {
      if (bank.mesh === mesh) return bank.source[instance_id] ?? null;
    }
    return null;
  }

  dispose(): void {
    for (const bank of this.banks.values()) {
      this.group.remove(bank.mesh);
      bank.mesh.dispose();
    }
    this.banks.clear();
    this.chunks.clear();
    this.placement.clear();
    this.dirty.clear();
  }

  // ---- rebasing ----

  /** Drops every run and re-queues every chunk against the new origin. The
   * buffers hold origin-relative positions, so this is the one change that
   * genuinely invalidates all of them at once. */
  private rebase(map_origin: IVec): void {
    this.packed_origin = map_origin;
    this.origin_known = true;
    for (const bank of this.banks.values()) {
      bank.end = 0;
      bank.live = 0;
      bank.slices.clear();
      bank.free.clear();
      bank.source.length = 0;
      bank.mesh.count = 0;
      bank.mesh.instanceMatrix.clearUpdateRanges();
    }
    this.placement.clear();
    for (const key of this.chunks.keys()) this.dirty.add(key);
  }

  // ---- one chunk ----

  private sync(key: string): void {
    const chunk = this.chunks.get(key);
    const previous = this.placement.get(key);

    if (!chunk) {
      if (previous) {
        for (const bank_key of previous) this.release(bank_key, key);
        this.placement.delete(key);
      }
      return;
    }

    const local = ivec.sub(chunk.base, this.packed_origin);
    const ox = ivec.to_real(ivec.X(local));
    const oy = ivec.to_real(ivec.Y(local));
    const oz = ivec.to_real(ivec.Z(local));

    const next = new Set<BankKey>();
    for (const [vkey, variant] of chunk.contribution) {
      const drawn: ModelInstanceTransform[] = [];
      let hidden: ModelInstanceTransform[] | null = null;
      for (const transform of variant.instances) {
        if (transform.hidden) (hidden ??= []).push(transform);
        else drawn.push(transform);
      }
      if (drawn.length > 0) {
        this.write(this.ensure_bank(vkey, variant, true), key, drawn, ox, oy, oz);
        next.add(vkey);
      }
      if (hidden !== null) {
        const bank_key = vkey + HIDDEN_SUFFIX;
        this.write(this.ensure_bank(bank_key, variant, false), key, hidden, ox, oy, oz);
        next.add(bank_key);
      }
    }

    if (previous) {
      for (const bank_key of previous) {
        if (!next.has(bank_key)) this.release(bank_key, key);
      }
    }
    this.placement.set(key, next);
  }

  private write(bank: Bank, key: string, list: readonly ModelInstanceTransform[], ox: number, oy: number, oz: number): void {
    let slice = bank.slices.get(key);
    // A run that no longer fits is returned whole rather than extended: the
    // neighbouring indices belong to other chunks.
    if (slice && slice.cap < list.length) {
      this.free_slice(bank, key, slice);
      slice = undefined;
    }
    if (!slice) slice = this.alloc(bank, key, list.length);

    const mesh = bank.mesh;
    for (let i = 0; i < list.length; i++) {
      compose_instance_matrix($matrix, list[i]);
      // The chunk offset is a pure translation, so it lands in the matrix's
      // own last column rather than costing a second compose.
      $matrix.elements[12] += ox;
      $matrix.elements[13] += oy;
      $matrix.elements[14] += oz;
      mesh.setMatrixAt(slice.start + i, $matrix);
      bank.source[slice.start + i] = list[i];
    }
    for (let i = list.length; i < slice.cap; i++) {
      mesh.setMatrixAt(slice.start + i, $vacant);
      bank.source[slice.start + i] = null;
    }

    bank.live += list.length - slice.count;
    slice.count = list.length;
    this.touch(bank, slice);
  }

  private release(bank_key: BankKey, key: string): void {
    const bank = this.banks.get(bank_key);
    const slice = bank?.slices.get(key);
    if (!bank || !slice) return;
    this.free_slice(bank, key, slice);
  }

  private free_slice(bank: Bank, key: string, slice: Slice): void {
    for (let i = 0; i < slice.cap; i++) {
      bank.mesh.setMatrixAt(slice.start + i, $vacant);
      bank.source[slice.start + i] = null;
    }
    bank.live -= slice.count;
    bank.slices.delete(key);
    const list = bank.free.get(slice.cap);
    if (list) list.push(slice.start);
    else bank.free.set(slice.cap, [slice.start]);
    this.touch(bank, slice);
  }

  private alloc(bank: Bank, key: string, need: u32): Slice {
    const cap = slice_cap(need);
    let start = bank.free.get(cap)?.pop();
    if (start === undefined) {
      if (bank.end + cap > bank.capacity) this.grow(bank, bank.end + cap);
      start = bank.end;
      bank.end += cap;
      bank.mesh.count = bank.end;
    }
    const slice: Slice = { start, cap, count: 0 };
    bank.slices.set(key, slice);
    return slice;
  }

  /** Hands three the byte range this run occupies, so the upload is the run
   * rather than the buffer. */
  private touch(bank: Bank, slice: Slice): void {
    bank.mesh.instanceMatrix.addUpdateRange(slice.start * 16, slice.cap * 16);
    bank.mesh.instanceMatrix.needsUpdate = true;
  }

  // ---- banks ----

  private ensure_bank(bank_key: BankKey, variant: DecorVariantContribution, drawn: boolean): Bank {
    const existing = this.banks.get(bank_key);
    if (existing) {
      // A skin can finish after its model, so the material may be replaced
      // under a bank; the geometry cannot — asset_store.ts resolves a URL to
      // a geometry once, and a variant with none contributes nothing at all.
      if (existing.mesh.material !== variant.material) existing.mesh.material = variant.material;
      return existing;
    }

    const bank: Bank = {
      key: bank_key,
      mesh: null as unknown as THREE.InstancedMesh,
      geometry: variant.geometry,
      drawn,
      capacity: INITIAL_CAPACITY,
      end: 0,
      live: 0,
      slices: new Map(),
      free: new Map(),
      source: [],
    };
    bank.mesh = this.make_mesh(bank, variant.material, INITIAL_CAPACITY);
    this.group.add(bank.mesh);
    this.banks.set(bank_key, bank);
    return bank;
  }

  private make_mesh(bank: Bank, material: THREE.Material, capacity: u32): THREE.InstancedMesh {
    const mesh = new THREE.InstancedMesh(bank.geometry, material, capacity);
    mesh.name = `decor/${bank.key}`;
    mesh.count = bank.end;
    mesh.frustumCulled = false;
    mesh.castShadow = bank.drawn;
    mesh.receiveShadow = bank.drawn;
    mesh.visible = bank.drawn;
    mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    // Never culled and never recomputed. The only thing three uses this for
    // here is the early-out in InstancedMesh.raycast, and a sphere covering
    // the loaded world is what a tight one would be anyway — recomputing it
    // per write would put back the whole-buffer pass this design removes.
    mesh.boundingSphere = new THREE.Sphere(new THREE.Vector3(), Infinity);
    return mesh;
  }

  /** Doubles a bank's buffer, carrying the existing runs across verbatim —
   * the matrices are already correct, so this is a copy rather than a
   * rebuild, and every slice keeps its index. */
  private grow(bank: Bank, needed: u32): void {
    let capacity = bank.capacity;
    while (capacity < needed) capacity *= 2;

    const mesh = this.make_mesh(bank, bank.mesh.material as THREE.Material, capacity);
    mesh.instanceMatrix.array.set(bank.mesh.instanceMatrix.array.subarray(0, bank.end * 16));
    mesh.instanceMatrix.needsUpdate = true;

    this.group.remove(bank.mesh);
    bank.mesh.dispose();
    bank.mesh = mesh;
    bank.capacity = capacity;
    this.group.add(mesh);
  }

  /**
   * Slides every live run to the front, in index order.
   *
   * Pure movement: the matrices inside a run are already right, so this is a
   * `copyWithin` per run and no compose at all. Ascending order is what makes
   * it safe in place — a run only ever moves left, so its source is always
   * past what has been written.
   */
  private compact(bank: Bank): void {
    const runs = [...bank.slices.entries()].sort((a, b) => a[1].start - b[1].start);
    const array = bank.mesh.instanceMatrix.array;
    let cursor = 0;
    for (const [, slice] of runs) {
      if (slice.start !== cursor) {
        array.copyWithin(cursor * 16, slice.start * 16, (slice.start + slice.cap) * 16);
        for (let i = 0; i < slice.cap; i++) bank.source[cursor + i] = bank.source[slice.start + i];
        slice.start = cursor;
      }
      cursor += slice.cap;
    }
    bank.end = cursor;
    bank.source.length = cursor;
    bank.free.clear();
    bank.mesh.count = cursor;
    // One whole-buffer upload, which is the point of doing this rarely.
    bank.mesh.instanceMatrix.clearUpdateRanges();
    bank.mesh.instanceMatrix.needsUpdate = true;
  }
}

/**
 * The one field. A module singleton rather than a context for the same
 * reason objects/store.ts is one: a chunk node is not a component, and
 * camera/input.tsx raycasts it from outside the render tree.
 */
export const decor_field = new DecorField();
