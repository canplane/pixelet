// @pxl/web/render/layer/pipeline/compositor.ts

/** Ordered WorldLayer composition with scratch rendering and a tiny ray LRU. */

import type { u32 } from "@xpute/core/kit/word.ts";

import { alloc_chunk_volume, type ChunkVolume, release_chunk_volume, seal_chunk_volume } from "./volume.ts";
import { alloc_chunk_surface, type ChunkSurface } from "./chunk_surface.ts";
import type { ChunkKey, WorldLayerRevision, WorldLayerStack } from "./contract.ts";

const stats = {
  hit: 0,
  miss: 0,
  eviction: 0,
  render_builds: 0,
  render_compose_ms: 0,
  render_consume_ms: 0,
  surface_builds: 0,
  surface_compose_ms: 0,
  max_compose_ms: 0,
  max_consume_ms: 0,
  max_surface_compose_ms: 0,
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
export function composite_chunk_layers(stack: WorldLayerStack, key: ChunkKey): ChunkVolume | null {
  const out = alloc_chunk_volume();
  let any_present = false;
  for (const layer of stack) any_present = layer.paint(key, out) || any_present;
  if (!any_present) {
    release_chunk_volume(out);
    return null;
  }
  return seal_chunk_volume(out);
}

/**
 * Minecraft-style heightmap path for coarse LODs. `undefined` means at least
 * one layer lacks the top-only contract and the caller must use dense
 * composition; `null` means the supported stack painted no surface.
 */
export function composite_chunk_surface_layers(
  stack: WorldLayerStack,
  key: ChunkKey,
): ChunkSurface | null | undefined {
  if (stack.some((layer) => layer.paint_surface === undefined)) return undefined;
  const start = performance.now();
  const out = alloc_chunk_surface();
  let any_present = false;
  for (const layer of stack) {
    const painted = layer.paint_surface!(key, out);
    if (painted === undefined) return undefined;
    any_present = painted || any_present;
  }
  const elapsed = performance.now() - start;
  stats.surface_builds++;
  stats.surface_compose_ms += elapsed;
  stats.max_surface_compose_ms = Math.max(stats.max_surface_compose_ms, elapsed);
  return any_present ? out : null;
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

type CacheEntry = {
  stack: WorldLayerStack;
  revisions: readonly WorldLayerRevision[];
  volume: ChunkVolume | null;
};

const ray_cache = new Map<string, CacheEntry>();
/** A hover ray crosses at most a few 64-wide batches at the current range. */
const MAX_RAY_ENTRIES = 4;
/** Hard CPU budget: at 1.25 MiB each, eight direct bands are about 10 MiB. */
const MAX_RAY_BANDS = 8;
let ray_cache_bands = 0;

export function compositor_cache_stats(): Readonly<typeof stats> & { entries: u32; bands: u32 } {
  return { ...stats, entries: ray_cache.size, bands: ray_cache_bands };
}

export function clear_compositor_cache(): void {
  for (const entry of ray_cache.values()) {
    if (entry.volume) release_chunk_volume(entry.volume);
  }
  ray_cache.clear();
  ray_cache_bands = 0;
}

function chunk_key_str(key: ChunkKey): string {
  return `${key.page_idx.toString()}:${key.page_off.toString()}`;
}

type AcquiredComposite = { volume: ChunkVolume | null; temporary: boolean };

function acquire_raycast_composite(stack: WorldLayerStack, key: ChunkKey): AcquiredComposite {
  const revisions = stack.map((layer) => layer.revision(key));
  const cache_key = chunk_key_str(key);
  const hit = ray_cache.get(cache_key);

  if (hit && hit.stack === stack && same_layer_revisions(hit.revisions, revisions)) {
    stats.hit++;
    ray_cache.delete(cache_key);
    ray_cache.set(cache_key, hit);
    return { volume: hit.volume, temporary: false };
  }

  stats.miss++;
  if (hit?.volume) {
    ray_cache_bands -= hit.volume.bands.length;
    release_chunk_volume(hit.volume);
  }
  ray_cache.delete(cache_key);
  const volume = composite_chunk_layers(stack, key);
  const new_bands = volume?.bands.length ?? 0;
  if (new_bands > MAX_RAY_BANDS) return { volume, temporary: true };
  ray_cache.set(cache_key, { stack, revisions, volume });
  ray_cache_bands += new_bands;

  while (ray_cache.size > MAX_RAY_ENTRIES || ray_cache_bands > MAX_RAY_BANDS) {
    const oldest_key = ray_cache.keys().next().value;
    if (oldest_key === undefined) break;
    const oldest = ray_cache.get(oldest_key);
    if (oldest?.volume) {
      ray_cache_bands -= oldest.volume.bands.length;
      release_chunk_volume(oldest.volume);
    }
    ray_cache.delete(oldest_key);
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
