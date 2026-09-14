// @pxl/web/render/layer/pipeline/core_pool.ts

/**
 * The core memory and its map: one fixed `WebAssembly.Memory`, cut into
 * sections with fixed starts and sizes, each its own buddy pool over its
 * range. No section may borrow from another, so a burst in one kind of
 * thing can never evict another kind, "full" always names which kind, and
 * a structure that must sit in linear memory is placed there by
 * construction rather than by load order
 * (issue/20260913-sections-of-the-core-memory-and-what-full-means.md).
 *
 * The memory is fixed-length rather than resizable, and never grown, for
 * three measured reasons
 * (issue/20260911-single-thread-core-and-when-to-spawn-a-worker.md): a view
 * into a grown memory is detached, and every mesh span and run lane is such
 * a view; views into a resizable buffer read two and a half times slower in
 * V8; and WebGL and WebGPU reject them outright. A reservation is committed
 * lazily — untouched pages cost address space, not memory — so it is sized
 * for the peak rather than to the byte. JavaScriptCore nonetheless reports
 * the whole reservation as the page's JavaScript memory (a Safari heap
 * timeline read it as one 512 MB ArrayBuffer), which is why a phone gets
 * half of it.
 *
 * Sections, in address order:
 *
 *   module   the Rust core's own: its stack, its data and its heap, at the
 *            front because that is where a wasm32 module's linker puts them
 *            (`--stack-first`, and data at the base). No pool — the module
 *            is the only writer, and crates/xpute-core/src/ds/arena.rs is the
 *            allocator over it. Its size is the one number both sides
 *            agree on, and the module says its own at instantiation so the
 *            map is checked rather than trusted.
 *   fixed    what is allocated once at startup and never freed — the change
 *            log's journal and tables, the scheduler's lanes, the mesher's
 *            scratch, later the command rings. Refuses rather than spills:
 *            a shortfall here is a startup failure, never a runtime one, and
 *            the section's layout is the contract a WASM core is handed.
 *   page     resident terrain and POI pages. Long-lived; bounded by the
 *            cleanup count and this cap.
 *   mesh     chunk meshes, run sets, tier caches, voxel bands — what churns
 *            as the view moves. The section the reclaim pass answers to.
 *
 * Sizes come from what the ledger has reported (a reservation peak of
 * 320 MB in one pool, ~4 MB a terrain page, the largest mesh block 1 MB) and
 * are adjusted after a phone reading. Past its range the page and mesh
 * sections still spill to plain regions the collector can take back — the
 * previous behaviour, kept until the byte budget bounds what is drawn
 * (BACKLOG P1); the spill is counted per section and the ledger shows it.
 *
 * The buddy over each section's range runs in the Rust module
 * (crates/xpute-core/src/ds/buddy.rs): the module is instantiated over the
 * memory here, before any section is cut, and each section's pool names
 * its blocks through it — the core owns the allocation of the memory it
 * works in, and what the module cannot reach, the plain regions past a
 * range, stays the TypeScript pool's. A device that could reserve no
 * memory gets no module and plain pools.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { type AddressedAlloc, buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { BuddyAllocator, type BuddyStat, type PoolKernel } from "@xpute/core/ds/buddy.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { instantiate_pxl, type PxlExports } from "@/wasm/pxl.ts";

const REGION_BYTES = 1 << 26;
const WASM_PAGE = 1 << 16;

/**
 * The module section, from address 0. Equal to crates/xpute-core/src/ds/arena.rs's
 * MODULE_BYTES, and checked against it when the module is instantiated
 * (wasm/pxl.ts). Carved out of the first region: the fixed section's pool
 * cuts what is left of that region in 16 MB regions, so the map stays
 * region-aligned where the pools need it, and a fixed allocation bigger
 * than one — nothing is, the tables are small — would be refused rather
 * than spilled. Half the region for the module: a dense chunk's scratch
 * ran a 16 MB section out (b3, 2026-09-13).
 */
export const MODULE_BYTES: u32 = 32 << 20;
const FIXED_REGION_BYTES: u32 = 16 << 20;

/**
 * The map, in regions of 64 MB. Eight on the desktop, four on a phone: the
 * whole reservation is charged to the page by JavaScriptCore, and a phone's
 * budget is a fraction of the desktop's.
 *
 *              desktop    phone
 *   fixed         1          1     the module's 32 MB, then the tables,
 *                                  which are small, and headroom for rings
 *   page          3          2     every page the terrain store keeps
 *                                  resident (terrain/config.ts derives the
 *                                  count from this section), and the ones
 *                                  in their release quarantine
 *   mesh          4          1     the churn; the reclaim pass keeps it under
 *
 * A page the section cannot hold spills to a plain region the module
 * cannot read, and its chunks are never built (core_build.ts's `reaches`):
 * a page is 4 MB of ground lanes and a 16 KB elevation block, so two
 * regions held 31 of the 32 pages kept — one page missing from the world,
 * whichever arrived last (2026-09-14).
 */
const LAYOUT = HANDHELD ? { fixed: 1, page: 2, mesh: 1 } : { fixed: 1, page: 3, mesh: 4 };
const CORE_REGIONS = LAYOUT.fixed + LAYOUT.page + LAYOUT.mesh;

function reserve_core_memory(): WebAssembly.Memory | null {
  for (const regions of [CORE_REGIONS, Math.ceil(CORE_REGIONS / 2)]) {
    const pages = (regions * REGION_BYTES) / WASM_PAGE;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // too large for this device; try the smaller reservation
    }
  }
  return null;
}

export const CORE_MEMORY: WebAssembly.Memory | null = reserve_core_memory();

/** The module section's range, for the check at instantiation and the
 * ledger: `[0, MODULE_BYTES)` of the core memory. */
export const MODULE_SECTION: { readonly offset: u32; readonly bytes: u32 } = { offset: 0, bytes: MODULE_BYTES };

/**
 * The one module over the core memory, or null where there is no core
 * memory. Awaited here: the sections' pools are cut through it, and the
 * first fixed allocation happens at import, so nothing may run before it
 * has arrived. A module that fails to load fails the client — the core
 * memory exists for the core, and the TypeScript pools alone would be a
 * different program.
 */
export const CORE_PXL: PxlExports | null = CORE_MEMORY === null ? null : await instantiate_pxl(CORE_MEMORY, MODULE_BYTES);

/** A section's kernel: pool `id` in the module, over `[offset, offset + bytes)`. */
function kernel_of(pxl: PxlExports, offset: u32, bytes: u32, region_bytes: u32, min_block_bytes: u32): PoolKernel & { id: u32 } {
  const id = pxl.pxl_pool_new(offset, bytes, region_bytes, min_block_bytes);
  return {
    id,
    acquire: (bytes) => pxl.pxl_pool_acquire(id, bytes),
    release: (at, bytes) => pxl.pxl_pool_release(id, at, bytes) !== 0,
    is_free: (at, bytes) => pxl.pxl_pool_is_free(id, at, bytes) !== 0,
    stat: () => new Uint32Array(pxl.memory.buffer, pxl.pxl_pool_stat(id), 12),
  };
}

export type SectionName = "fixed" | "page" | "mesh";

export type Section = {
  readonly name: SectionName;
  /** Where the section starts in the core memory, and its size. */
  readonly offset: u32;
  readonly bytes: u32;
  readonly pool: BuddyAllocator;
  readonly alloc: AddressedAlloc;
  /** The module's id for the pool over this section's range, where there
   * is a module. */
  readonly kernel_id: u32 | null;
};

/**
 * Cuts the reservation into the map. A device that reserved half asks for
 * the same proportions over what it got; a device that reserved nothing
 * gets plain pools with the same names, which behave as the one pool did
 * before this existed.
 */
function lay_out(memory: WebAssembly.Memory | null): Record<SectionName, Section> {
  const backing = memory?.buffer ?? null;
  const total = backing ? backing.byteLength / REGION_BYTES : CORE_REGIONS;
  const scale = total / CORE_REGIONS;
  const regions = {
    fixed: Math.max(1, Math.round(LAYOUT.fixed * scale)),
    page: Math.max(1, Math.round(LAYOUT.page * scale)),
    mesh: 0,
  };
  regions.mesh = Math.max(1, total - regions.fixed - regions.page);

  let offset = 0;
  const section = (name: SectionName, count: u32, no_spill: boolean): Section => {
    let bytes = count * REGION_BYTES;
    let region_bytes = REGION_BYTES;
    if (name === "fixed") {
      offset += MODULE_BYTES;
      bytes -= MODULE_BYTES;
      region_bytes = FIXED_REGION_BYTES;
    }
    const kernel = CORE_PXL !== null ? kernel_of(CORE_PXL, offset, bytes, region_bytes, 1 << 12) : null;
    const pool = new BuddyAllocator({
      min_block_bytes: 1 << 12,
      region_bytes,
      backing: backing ?? undefined,
      kernel: kernel ?? undefined,
      backing_offset: backing ? offset : undefined,
      backing_bytes: backing ? bytes : undefined,
      max_idle_regions: 1,
      no_spill: backing !== null && no_spill,
      // `poison` stays off even in development — it is a memset of every
      // released block, which would distort the very rebuild storm the pool
      // exists to measure. Turn it on when hunting a view that outlived its
      // release: the stale reader then sees NaN instead of a plausible
      // neighbouring chunk.
      poison: false,
      // Reuse is on: a chunk mesh has exactly two owners — chunk_node.ts's
      // LOD cache and the layout naming it — both mutated synchronously by
      // the node, so there is no window in which a layout names meshes
      // nothing holds. `[pool] double release` or `[chunk_bucket] retain of
      // a released mesh` in the console is a real ownership bug, not noise.
      bypass: false,
    });
    const s: Section = { name, offset, bytes, pool, alloc: buddy_alloc(pool), kernel_id: kernel?.id ?? null };
    offset += bytes;
    return s;
  };
  return {
    fixed: section("fixed", regions.fixed, true),
    page: section("page", regions.page, false),
    mesh: section("mesh", regions.mesh, false),
  };
}

export const CORE_SECTIONS: Readonly<Record<SectionName, Section>> = lay_out(CORE_MEMORY);

/** The page section's size: what bounds how many terrain pages stay
 * resident (terrain/config.ts). */
export const PAGE_SECTION_BYTES: u32 = CORE_SECTIONS.page.bytes;

// The module's volumes take their standing band stores from the fixed
// section, which never runs out, and the rest from the mesh section
// (crates/pxl-core/src/bake/volume.rs).
if (CORE_PXL !== null) CORE_PXL.pxl_volume_pools(CORE_SECTIONS.fixed.kernel_id!, CORE_SECTIONS.mesh.kernel_id!);

/** The fixed section: allocated once, never freed, refuses rather than
 * spills. Where a lane that a kernel must find lives. */
export const FIXED_ALLOC: AddressedAlloc = CORE_SECTIONS.fixed.alloc;

/** The page section's pool: resident terrain and POI pages. */
export const PAGE_POOL: BuddyAllocator = CORE_SECTIONS.page.pool;

/**
 * The mesh section's pool and typed lanes — what was the one core pool.
 * Chunk meshes, run sets, tier caches, voxel bands, and every lane the
 * churning structures carve.
 */
export const CORE_POOL: BuddyAllocator = CORE_SECTIONS.mesh.pool;
export const CORE_ALLOC: AddressedAlloc = CORE_SECTIONS.mesh.alloc;

export function core_pool_stat(): Readonly<BuddyStat> {
  return CORE_POOL.stat;
}

/** Every section's gauges, for the ledger: how much of its range each has
 * cut, what is live in it, and whether it has spilled or refused. */
export function core_sections_stat(): { name: SectionName; bytes: u32; stat: Readonly<BuddyStat> }[] {
  return (["fixed", "page", "mesh"] as const).map((name) => ({ name, bytes: CORE_SECTIONS[name].bytes, stat: CORE_SECTIONS[name].pool.stat }));
}

/** The reservation's size, for the budgets drawn against it. */
export const CORE_MEMORY_BYTES: u32 = CORE_MEMORY?.buffer.byteLength ?? CORE_REGIONS * REGION_BYTES;
