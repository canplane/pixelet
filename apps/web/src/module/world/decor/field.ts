// @pxl/web/module/world/decor/field.ts

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
 * **A contribution is written as it arrives, and never read again.** The
 * instances come as lanes the chunk owns (decor/instance.ts) and may hand
 * back to the pool the moment `set_chunk` returns, so this keeps nothing of
 * them: the matrices are composed on the spot, and what a slot needs for a
 * hit — the entry's band, position and column — is copied into lanes of its
 * own. A floating-origin shift is then a translation of every live matrix
 * by the exact integer delta, not a rebuild from a source this no longer
 * holds. `update` runs once a frame for that shift and for compaction.
 *
 * LOD stops being a second mechanism. A chunk contributes what should be
 * drawn right now, so a tier change is a different contribution rather than a
 * visibility flag on a mesh that exists either way, and a chunk that leaves
 * the frustum withdraws rather than being hidden.
 *
 * **The geometry and material ride along with the instances** instead of
 * being looked up here. Partly so this module stays free of the app's
 * state — asset_store.ts reaches store.ts, the same line
 * instanced_model.ts's own header draws — and partly because it removes
 * the second arrival path: a chunk already
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

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import type { U32Array } from "@xpute/core/kit/array.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { compose_instance_matrix } from "./instanced_model.ts";
import { type DecorInstances, HIDDEN_FLAG, NO_ENTRY } from "./instance.ts";
import { entry_id_of } from "@/module/world/object/store.ts";
import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

/** Identifies one variant across every bundle. Named by kind rather than
 * the bundle's position in the registry, so the mesh name — `decor/<key>` —
 * says what it draws (sched/draw_census.ts reads it). */
export type VariantKey = string;
export const variant_key = (kind: string, variant: u32): VariantKey => `${kind}:${variant}`;

export type DecorVariantContribution = {
  geometry: THREE.BufferGeometry;
  material: THREE.Material;
  instances: DecorInstances;
};

/** What one chunk contributes. Held rather than merged so a chunk can be
 * replaced or withdrawn without anything else being recomputed. */
export type DecorContribution = ReadonlyMap<VariantKey, DecorVariantContribution>;

/**
 * A variant's drawn instances, or its hit-only ones — see `Bank.drawn`. Two
 * banks per variant at most, addressed the same way. A bank is named once
 * by its key and thereafter by its index in `banks`; a chunk's placement
 * is which banks hold a run for it, which each bank's own run index says.
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

const LANE = { init_cap: 64, alloc: CORE_ALLOC } as const;

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
  /**
   * The runs, one per chunk holding instances in this bank: the chunk's
   * base in an IndexedMap, and at its position where the run starts, how
   * much it was given, and how much of that it is using.
   */
  runs: IndexedMap<bigint>;
  run_start: Vector<U32Array>;
  run_cap: Vector<U32Array>;
  run_count: Vector<U32Array>;
  /** Freed runs, as start and capacity, taken back by capacity class. */
  free_start: Vector<U32Array>;
  free_cap: Vector<U32Array>;
  /**
   * What each slot holds, as the store's own numbers — the entry's band and
   * packed position, and the column it is under — so a raycast's
   * `instanceId` resolves to an entry the eraser can act on without an
   * object per slot. NO_ENTRY in `band` is a vacant slot or a procedural
   * instance, neither of which a hit resolves to. Grown with `capacity`.
   */
  src_band: Uint32Array;
  src_pos: Uint32Array;
  src_column: BigUint64Array;
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

  /** The banks, by index; `bank_keys` names them so a contribution's variant
   * key finds its bank. A handful per session, one per variant and side. */
  private readonly banks: Bank[] = [];
  private readonly bank_keys: BankKey[] = [];
  /** Scratch for set_chunk: which banks this chunk wrote this time. */
  private readonly wrote = new Vector(Uint8Array, LANE);

  /** The floating origin the live buffers are expressed against. A shift
   * rebases everything, which is rare — L4 is 4096u — and keeps the floats
   * the GPU sees small, which is the whole point of having an origin. */
  private packed_origin: IVec = 0n;

  constructor() {
    this.group.name = "decor-field";
    // One mesh spans the whole loaded world, so its bounds are the world's:
    // there is nothing for a frustum test to reject. Chunks withdraw when
    // they leave the frustum, which is where the culling actually happens.
    this.group.frustumCulled = false;
  }

  /** Once a frame, from the render loop: follows a floating-origin shift
   * and compacts any bank that has drifted. */
  update(map_origin: IVec): void {
    if (map_origin !== this.packed_origin) this.shift(map_origin);
    for (const bank of this.banks) {
      if (bank.end > COMPACT_FLOOR && bank.end > bank.live * COMPACT_RATIO) this.compact(bank);
    }
  }

  /**
   * Replaces one chunk's contribution, writing it now.
   *
   * `base` is the chunk's own absolute lattice origin and its identity
   * here; the instances inside are chunk-local, exactly as they are built,
   * and are only borrowed for the duration of this call. Anything absent
   * from `contribution` is absent from the world, so a chunk withdraws by
   * clearing rather than by contributing something empty.
   */
  set_chunk(base: IVec, contribution: DecorContribution): void {
    const local = ivec.sub(base, this.packed_origin);
    const ox = ivec.to_real(ivec.X(local));
    const oy = ivec.to_real(ivec.Y(local));
    const oz = ivec.to_real(ivec.Z(local));

    const wrote = this.wrote;
    wrote.truncate(0);
    for (let b = 0; b < this.banks.length; b++) wrote.push(0);
    for (const [vkey, variant] of contribution) {
      const inst = variant.instances;
      let hidden_cnt = 0;
      for (let i = 0; i < inst.len; i++) if (inst.flags[i] & HIDDEN_FLAG) hidden_cnt++;
      if (inst.len - hidden_cnt > 0) {
        const b = this.ensure_bank(vkey, variant, true);
        this.write(this.banks[b], base, inst, false, inst.len - hidden_cnt, ox, oy, oz);
        wrote.set(b, 1);
      }
      if (hidden_cnt > 0) {
        const b = this.ensure_bank(vkey + HIDDEN_SUFFIX, variant, false);
        this.write(this.banks[b], base, inst, true, hidden_cnt, ox, oy, oz);
        wrote.set(b, 1);
      }
    }

    // A bank this chunk held a run in last time and did not write now.
    for (let b = 0; b < this.banks.length; b++) {
      if (wrote.mem[b] === 0) this.release(this.banks[b], base);
    }
  }

  clear_chunk(base: IVec): void {
    for (const bank of this.banks) this.release(bank, base);
  }

  /** Every mesh a raycast should consider, drawn or hit-only. */
  hit_meshes(): THREE.InstancedMesh[] {
    const out: THREE.InstancedMesh[] = [];
    for (const bank of this.banks) {
      if (bank.live > 0) out.push(bank.mesh);
    }
    return out;
  }

  /** Resolves a raycast hit back to the instance that produced it. */
  /** The store entry id behind a raycast hit, or null for a procedural
   * instance or a vacant slot. */
  entry_at(mesh: THREE.Object3D, instance_id: u32): string | null {
    for (const bank of this.banks) {
      if (bank.mesh !== mesh) continue;
      if (instance_id >= bank.capacity) return null;
      const band = bank.src_band[instance_id];
      if (band === NO_ENTRY) return null;
      return entry_id_of(bank.src_column[instance_id], band, bank.src_pos[instance_id]);
    }
    return null;
  }

  dispose(): void {
    for (const bank of this.banks) {
      this.group.remove(bank.mesh);
      CORE_ALLOC.give(bank.mesh.instanceMatrix.array);
      bank.mesh.dispose();
    }
    this.banks.length = 0;
    this.bank_keys.length = 0;
  }

  // ---- rebasing ----

  /**
   * Moves every live matrix onto the new origin. The buffers hold
   * origin-relative positions and the origin is a lattice value, so the
   * delta is an exact integer and the shift is one addition to each
   * matrix's translation column — vacant slots included, which have no
   * extent and stay invisible wherever they are put.
   */
  private shift(map_origin: IVec): void {
    const delta = ivec.sub(this.packed_origin, map_origin);
    const dx = ivec.to_real(ivec.X(delta));
    const dy = ivec.to_real(ivec.Y(delta));
    const dz = ivec.to_real(ivec.Z(delta));
    this.packed_origin = map_origin;
    for (const bank of this.banks) {
      const array = bank.mesh.instanceMatrix.array;
      for (let i = 0, n = bank.end * 16; i < n; i += 16) {
        array[i + 12] += dx;
        array[i + 13] += dy;
        array[i + 14] += dz;
      }
      bank.mesh.instanceMatrix.clearUpdateRanges();
      bank.mesh.instanceMatrix.needsUpdate = true;
    }
  }

  /**
   * Writes the instances of `inst` whose hidden flag equals `hidden` — `count`
   * of them — into the chunk's run in `bank`. The two banks a variant can
   * land in (drawn and hidden) each take their half of one lane set in one
   * pass.
   */
  private write(bank: Bank, base: IVec, inst: DecorInstances, hidden: boolean, count: u32, ox: number, oy: number, oz: number): void {
    let run = bank.runs.find(base);
    // A run that no longer fits is returned whole rather than extended: the
    // neighbouring indices belong to other chunks.
    if (run >= 0 && bank.run_cap.mem[run] < count) {
      this.free_run(bank, run);
      run = -1;
    }
    if (run < 0) run = this.alloc(bank, base, count);
    const start = bank.run_start.mem[run];
    const cap = bank.run_cap.mem[run];

    const mesh = bank.mesh;
    let w = 0;
    for (let i = 0; i < inst.len; i++) {
      if (((inst.flags[i] & HIDDEN_FLAG) !== 0) !== hidden) continue;
      compose_instance_matrix($matrix, inst.x[i], inst.y[i], inst.z[i], inst.rotation_z[i], inst.scale[i]);
      // The chunk offset is a pure translation, so it lands in the matrix's
      // own last column rather than costing a second compose.
      $matrix.elements[12] += ox;
      $matrix.elements[13] += oy;
      $matrix.elements[14] += oz;
      const at = start + w++;
      mesh.setMatrixAt(at, $matrix);
      bank.src_band[at] = inst.band[i];
      bank.src_pos[at] = inst.pos[i];
      bank.src_column[at] = base;
    }
    for (let i = count; i < cap; i++) {
      mesh.setMatrixAt(start + i, $vacant);
      bank.src_band[start + i] = NO_ENTRY;
    }

    bank.live += count - bank.run_count.mem[run];
    bank.run_count.mem[run] = count;
    this.touch(bank, start, cap);
  }

  private release(bank: Bank, base: IVec): void {
    const run = bank.runs.find(base);
    if (run >= 0) this.free_run(bank, run);
  }

  /** Vacates a run and forgets it; its range goes to the bank's free list. */
  private free_run(bank: Bank, run: u32): void {
    const start = bank.run_start.mem[run];
    const cap = bank.run_cap.mem[run];
    for (let i = 0; i < cap; i++) {
      bank.mesh.setMatrixAt(start + i, $vacant);
      bank.src_band[start + i] = NO_ENTRY;
    }
    bank.live -= bank.run_count.mem[run];
    bank.free_start.push(start);
    bank.free_cap.push(cap);
    const from = bank.runs.remove_at(run);
    if (from !== run) {
      bank.run_start.mem[run] = bank.run_start.mem[from];
      bank.run_cap.mem[run] = bank.run_cap.mem[from];
      bank.run_count.mem[run] = bank.run_count.mem[from];
    }
    bank.run_start.truncate(from);
    bank.run_cap.truncate(from);
    bank.run_count.truncate(from);
    this.touch(bank, start, cap);
  }

  /** A run for the chunk of `need` instances: a freed one of the right
   * class, or a new one off the end. Returns its position in `runs`. */
  private alloc(bank: Bank, base: IVec, need: u32): u32 {
    const cap = slice_cap(need);
    let start: i32 = -1;
    for (let f = bank.free_cap.len - 1; f >= 0; f--) {
      if (bank.free_cap.mem[f] !== cap) continue;
      start = bank.free_start.mem[f];
      const last = bank.free_cap.len - 1;
      bank.free_start.mem[f] = bank.free_start.mem[last];
      bank.free_cap.mem[f] = bank.free_cap.mem[last];
      bank.free_start.truncate(last);
      bank.free_cap.truncate(last);
      break;
    }
    if (start < 0) {
      if (bank.end + cap > bank.capacity) this.grow(bank, bank.end + cap);
      start = bank.end;
      bank.end += cap;
      bank.mesh.count = bank.end;
    }
    const run = bank.runs.insert(base);
    bank.run_start.set(run, start);
    bank.run_cap.set(run, cap);
    bank.run_count.set(run, 0);
    return run;
  }

  /** Hands three the byte range this run occupies, so the upload is the run
   * rather than the buffer. */
  private touch(bank: Bank, start: u32, cap: u32): void {
    bank.mesh.instanceMatrix.addUpdateRange(start * 16, cap * 16);
    bank.mesh.instanceMatrix.needsUpdate = true;
  }

  // ---- banks ----

  /** The bank's index, opening it on first use. */
  private ensure_bank(bank_key: BankKey, variant: DecorVariantContribution, drawn: boolean): u32 {
    const b = this.bank_keys.indexOf(bank_key);
    if (b >= 0) {
      // A skin can finish after its model, so the material may be replaced
      // under a bank; the geometry cannot — asset_store.ts resolves a URL to
      // a geometry once, and a variant with none contributes nothing at all.
      const existing = this.banks[b];
      if (existing.mesh.material !== variant.material) existing.mesh.material = variant.material;
      return b;
    }

    const bank: Bank = {
      key: bank_key,
      mesh: null as unknown as THREE.InstancedMesh,
      geometry: variant.geometry,
      drawn,
      capacity: INITIAL_CAPACITY,
      end: 0,
      live: 0,
      runs: new IndexedMap(I64_KEYS, 64, CORE_ALLOC),
      run_start: new Vector(Uint32Array, LANE),
      run_cap: new Vector(Uint32Array, LANE),
      run_count: new Vector(Uint32Array, LANE),
      free_start: new Vector(Uint32Array, LANE),
      free_cap: new Vector(Uint32Array, LANE),
      src_band: CORE_ALLOC.take(Uint32Array, INITIAL_CAPACITY).fill(NO_ENTRY),
      src_pos: CORE_ALLOC.take(Uint32Array, INITIAL_CAPACITY),
      src_column: CORE_ALLOC.take(BigUint64Array, INITIAL_CAPACITY),
    };
    bank.mesh = this.make_mesh(bank, variant.material, INITIAL_CAPACITY);
    this.group.add(bank.mesh);
    this.banks.push(bank);
    this.bank_keys.push(bank_key);
    return this.banks.length - 1;
  }

  private make_mesh(bank: Bank, material: THREE.Material, capacity: u32): THREE.InstancedMesh {
    const mesh = new THREE.InstancedMesh(bank.geometry, material, capacity);
    mesh.name = `decor/${bank.key}`;
    mesh.count = bank.end;
    mesh.frustumCulled = false;
    mesh.castShadow = bank.drawn;
    mesh.receiveShadow = bank.drawn;
    mesh.visible = bank.drawn;
    // The matrices live in the core memory like every other lane — three
    // uploads from any ArrayBuffer view. InstancedMesh's constructor made a
    // heap buffer of its own, dropped here once per bank or growth.
    mesh.instanceMatrix = new THREE.InstancedBufferAttribute(CORE_ALLOC.take(Float32Array, capacity * 16), 16);
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

    const src_band = CORE_ALLOC.take(Uint32Array, capacity).fill(NO_ENTRY);
    const src_pos = CORE_ALLOC.take(Uint32Array, capacity);
    const src_column = CORE_ALLOC.take(BigUint64Array, capacity);
    src_band.set(bank.src_band.subarray(0, bank.end));
    src_pos.set(bank.src_pos.subarray(0, bank.end));
    src_column.set(bank.src_column.subarray(0, bank.end));
    CORE_ALLOC.give(bank.src_band);
    CORE_ALLOC.give(bank.src_pos);
    CORE_ALLOC.give(bank.src_column);
    bank.src_band = src_band;
    bank.src_pos = src_pos;
    bank.src_column = src_column;

    this.group.remove(bank.mesh);
    CORE_ALLOC.give(bank.mesh.instanceMatrix.array);
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
    const n = bank.runs.size;
    const order = new Uint32Array(n);
    for (let r = 0; r < n; r++) order[r] = r;
    const run_start = bank.run_start.mem;
    order.sort((a, b) => run_start[a] - run_start[b]);
    const array = bank.mesh.instanceMatrix.array;
    let cursor = 0;
    for (const run of order) {
      const start = run_start[run];
      const cap = bank.run_cap.mem[run];
      if (start !== cursor) {
        array.copyWithin(cursor * 16, start * 16, (start + cap) * 16);
        for (let i = 0; i < cap; i++) {
          bank.src_band[cursor + i] = bank.src_band[start + i];
          bank.src_pos[cursor + i] = bank.src_pos[start + i];
          bank.src_column[cursor + i] = bank.src_column[start + i];
        }
        run_start[run] = cursor;
      }
      cursor += cap;
    }
    bank.end = cursor;
    bank.src_band.fill(NO_ENTRY, cursor);
    bank.free_start.truncate(0);
    bank.free_cap.truncate(0);
    bank.mesh.count = cursor;
    // One whole-buffer upload, which is the point of doing this rarely.
    bank.mesh.instanceMatrix.clearUpdateRanges();
    bank.mesh.instanceMatrix.needsUpdate = true;
  }
}

/**
 * The one field. A module singleton rather than a context for the same
 * reason object/store.ts is one: a chunk node is not a component, and
 * camera/input.tsx raycasts it from outside the render tree.
 */
export const decor_field = new DecorField();
