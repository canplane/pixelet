// @pxl/web/render/layer/pipeline/compositor.ts

/** Ordered WorldLayer composition with scratch rendering and a tiny ray LRU. */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { CORE_ALLOC } from "./core_pool.ts";

import { alloc_chunk_volume, type ChunkVolume, release_chunk_volume, seal_chunk_volume } from "./volume.ts";
import type { ChunkKey, WorldLayerRevision, WorldLayerStack } from "./contract.ts";

const stats = {
  hit: 0,
  miss: 0,
  eviction: 0,
  render_builds: 0,
  render_compose_ms: 0,
  render_consume_ms: 0,
  max_compose_ms: 0,
  max_consume_ms: 0,
};

export function same_layer_revisions(a: readonly WorldLayerRevision[], b: readonly WorldLayerRevision[]): boolean {
  if (a.length !== b.length) return false;
  for (let layer = 0; layer < a.length; layer++) {
    const ar = a[layer], br = b[layer];
    if (ar.length !== br.length) return false;
    for (let i = 0; i < ar.length; i++) {
      if (!Object.is(ar[i], br[i])) return false;
    }
  }
  return true;
}

/** Pure composition, retained for tests and the bounded raycast cache. */
export function composite_chunk_layers(stack: WorldLayerStack, key: ChunkKey, opts?: { for_raycast?: boolean }): ChunkVolume | null {
  const out = alloc_chunk_volume();
  let any_present = false;
  for (const layer of stack) any_present = layer.paint(key, out, opts) || any_present;
  if (!any_present) {
    release_chunk_volume(out);
    return null;
  }
  return seal_chunk_volume(out);
}

/**
 * Render path: the dense bands live only for this synchronous consumer.
 * Components retain the resulting GPU instance buffers, never the 64^3 field.
 */
export function with_temporary_composite<T>(
  stack: WorldLayerStack,
  key: ChunkKey,
  consume: (volume: ChunkVolume) => T,
): T | null {
  const compose_start = performance.now();
  const volume = composite_chunk_layers(stack, key);
  const compose_ms = performance.now() - compose_start;
  stats.render_builds++;
  stats.render_compose_ms += compose_ms;
  stats.max_compose_ms = Math.max(stats.max_compose_ms, compose_ms);
  if (!volume) return null;
  const consume_start = performance.now();
  try {
    return consume(volume);
  } finally {
    const consume_ms = performance.now() - consume_start;
    stats.render_consume_ms += consume_ms;
    stats.max_consume_ms = Math.max(stats.max_consume_ms, consume_ms);
    release_chunk_volume(volume);
  }
}

/**
 * The raycast composites kept between hovers, as a small table: the chunk's
 * absolute base in an IndexedMap in the core memory, when each was last
 * used and how many bands it holds as lanes, and the three things that are
 * references — the stack it was built from, the layer revisions it is valid
 * for, the pooled volume — as handles at the position. Least recently used
 * is evicted first.
 */
const ray_index = new IndexedMap(I64_KEYS, 16, CORE_ALLOC);
const ray_used = new Vector(Uint32Array, { init_cap: 16, alloc: CORE_ALLOC });
const ray_bands = new Vector(Uint32Array, { init_cap: 16, alloc: CORE_ALLOC });
const ray_stack: WorldLayerStack[] = [];
const ray_revisions: (readonly WorldLayerRevision[])[] = [];
const ray_volume: (ChunkVolume | null)[] = [];
let ray_tick: u32 = 0;
/** A hover ray crosses at most a few 64-wide batches at the current range. */
const MAX_RAY_ENTRIES = 4;
/** Hard CPU budget: at 1.25 MiB each, eight direct bands are about 10 MiB. */
const MAX_RAY_BANDS = 8;
let ray_cache_bands = 0;

export function compositor_cache_stats(): Readonly<typeof stats> & { entries: u32; bands: u32 } {
  return { ...stats, entries: ray_index.size, bands: ray_cache_bands };
}

function ray_remove(pos: u32): void {
  const volume = ray_volume[pos];
  if (volume) {
    ray_cache_bands -= volume.bands.length;
    release_chunk_volume(volume);
  }
  const from = ray_index.remove_at(pos);
  if (from !== pos) {
    ray_used.mem[pos] = ray_used.mem[from];
    ray_bands.mem[pos] = ray_bands.mem[from];
    ray_stack[pos] = ray_stack[from];
    ray_revisions[pos] = ray_revisions[from];
    ray_volume[pos] = ray_volume[from];
  }
  ray_used.truncate(from);
  ray_bands.truncate(from);
  ray_stack.length = from;
  ray_revisions.length = from;
  ray_volume.length = from;
}

export function clear_compositor_cache(): void {
  while (ray_index.size > 0) ray_remove(ray_index.size - 1);
  ray_cache_bands = 0;
}

function chunk_base(key: ChunkKey): bigint {
  return ivec.add(FROM_L3_IDX(key.page_idx), key.page_off);
}

type AcquiredComposite = { volume: ChunkVolume | null; temporary: boolean };

function acquire_raycast_composite(stack: WorldLayerStack, key: ChunkKey): AcquiredComposite {
  const revisions = stack.map((layer) => layer.revision(key));
  const base = chunk_base(key);
  let pos = ray_index.find(base);

  if (pos >= 0 && ray_stack[pos] === stack && same_layer_revisions(ray_revisions[pos], revisions)) {
    stats.hit++;
    ray_used.mem[pos] = ++ray_tick;
    return { volume: ray_volume[pos], temporary: false };
  }

  stats.miss++;
  if (pos >= 0) ray_remove(pos);
  const volume = composite_chunk_layers(stack, key, { for_raycast: true });
  const new_bands = volume?.bands.length ?? 0;
  if (new_bands > MAX_RAY_BANDS) return { volume, temporary: true };
  pos = ray_index.insert(base);
  ray_used.set(pos, ++ray_tick);
  ray_bands.set(pos, new_bands);
  ray_stack[pos] = stack;
  ray_revisions[pos] = revisions;
  ray_volume[pos] = volume;
  ray_cache_bands += new_bands;

  while (ray_index.size > MAX_RAY_ENTRIES || ray_cache_bands > MAX_RAY_BANDS) {
    let oldest = 0;
    for (let i = 1; i < ray_index.size; i++) if (ray_used.mem[i] < ray_used.mem[oldest]) oldest = i;
    ray_remove(oldest);
    stats.eviction++;
  }
  return { volume, temporary: false };
}

/**
 * Raycast-only bounded LRU. Oversized vertical chunks are consumed once and
 * released instead of being allowed to violate the mobile memory ceiling.
 */
export function with_raycast_composite<T>(
  stack: WorldLayerStack,
  key: ChunkKey,
  consume: (volume: ChunkVolume | null) => T,
): T {
  const acquired = acquire_raycast_composite(stack, key);
  try {
    return consume(acquired.volume);
  } finally {
    if (acquired.temporary && acquired.volume) release_chunk_volume(acquired.volume);
  }
}
