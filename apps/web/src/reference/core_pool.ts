// @pxl/web/reference/core_pool.ts

/**
 * The kernel's sections as the TypeScript references cut them: a pool over
 * each, whose blocks the module's own pool hands out through the rings, so a
 * reference and the module work in the same memory and each compares against
 * the other's bytes in place. The app holds no picture of the sections — the
 * kernel alone places a block — so this lives with the references it serves.
 *
 * Every section refuses rather than spills, since a block outside the memory
 * is one the kernel cannot read.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { type AddressedAlloc, buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { BuddyAllocator, type PoolKernel } from "@xpute/core/ds/buddy.ts";
import { read_section } from "@xpute/core/sys/memory.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { BOOT, boot_block } from "@/wasm/module.ts";
import { packet } from "@/wasm/core_ring.ts";
import { CORE_KERNEL, CORE_MEMORY, CORE_RING } from "@/wasm/core_memory.ts";
import type { MeshBlock } from "@/reference/mesh/mesh_block.ts";

/** crates/pxl/core/src/memory.rs's MIN_BLOCK_BYTES. */
const MIN_BLOCK_BYTES: u32 = 1 << 12;

const boot = boot_block(CORE_KERNEL);

/**
 * A section's kernel: pool `id` in the module. A release is sent rather than
 * asked: whatever could hand the block out again — an acquire, a tick —
 * drains first, so the release is applied before it; and one the pool
 * refuses comes back as the ring's failure, said in the console, where an
 * answer here would cost a turn on every block given back.
 */
function kernel_of(id: u32): PoolKernel {
  return {
    acquire: (n) => CORE_RING.call(Cmd.POOL_ACQUIRE, packet((p) => p.u32(id).u32(n)), (at) => at >>> 0),
    release: (at, n) => {
      CORE_RING.send(0, Cmd.POOL_RELEASE, packet((p) => p.u32(id).u32(at).u32(n)));
      return true;
    },
    is_free: (at, n) => CORE_RING.call(Cmd.POOL_FREE, packet((p) => p.u32(id).u32(at).u32(n)), (free) => free === 1),
    stat: () => new Uint32Array(CORE_MEMORY.buffer, CORE_RING.call(Cmd.POOL_READ, packet((p) => p.u32(id)), (at) => at >>> 0), 12),
  };
}

export type SectionName = "fixed" | "page" | "mesh";

export type Section = {
  readonly name: SectionName;
  /** Where the section starts in the memory, and its size. */
  readonly offset: u32;
  readonly bytes: u32;
  readonly pool: BuddyAllocator;
  readonly alloc: AddressedAlloc;
  /** The module's pool for this section (memory.rs's FIXED_POOL, PAGE_POOL, MESH_POOL). */
  readonly kernel_id: u32;
};

/** The section the boot block describes at word `at`, over the module's pool `id`. */
function section(name: SectionName, id: u32, at: u32): Section {
  const { offset, bytes, region_bytes } = read_section(boot, at);
  const pool = new BuddyAllocator({
    min_block_bytes: MIN_BLOCK_BYTES,
    region_bytes,
    backing: CORE_MEMORY.buffer,
    kernel: kernel_of(id),
    backing_offset: offset,
    backing_bytes: bytes,
    max_idle_regions: 1,
    no_spill: true,
    // `poison` stays off — it is a memset of every released block, which
    // would distort the rebuild storms the references are timed through.
    // Turn it on when hunting a view that outlived its release: the stale
    // reader then sees NaN instead of a plausible neighbouring chunk.
    poison: false,
    bypass: false,
  });
  return { name, offset, bytes, pool, alloc: buddy_alloc(pool), kernel_id: id };
}

export const CORE_SECTIONS: Readonly<Record<SectionName, Section>> = {
  fixed: section("fixed", 0, BOOT.fixed),
  page: section("page", 1, BOOT.page),
  mesh: section("mesh", 2, BOOT.mesh),
};

/** The fixed section: allocated once, never freed. */
export const FIXED_ALLOC: AddressedAlloc = CORE_SECTIONS.fixed.alloc;

/** The mesh section's pool and typed lanes: meshes, run sets, tier caches,
 * voxel bands, and every lane the churning structures carve. */
export const CORE_POOL: BuddyAllocator = CORE_SECTIONS.mesh.pool;
export const CORE_ALLOC: AddressedAlloc = CORE_SECTIONS.mesh.alloc;

/** Hands one baked mesh's block back to the mesh section. Every view into it
 * is dead afterwards. */
export function release_chunk_mesh(mesh: { block: MeshBlock["block"] }): void {
  CORE_POOL.release(mesh.block);
}
