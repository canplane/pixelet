// @pxl/web/render/layer/pipeline/compositor.ts

/**
 * Ordered WorldLayer composition into one pooled volume: the TypeScript
 * composite the module's builds and raycast are held to (core_build.test.ts,
 * core_raycast.test.ts). Nothing on screen is composited here; the module
 * composes its own volumes (crates/pxl-core/src/build.rs) and keeps the
 * raycast's between hovers (crates/pxl-core/src/raycast.rs).
 */

import { alloc_chunk_volume, type ChunkVolume, release_chunk_volume, seal_chunk_volume } from "./volume.ts";
import type { ChunkKey, WorldLayerStack } from "./contract.ts";

/** Every layer painted in order into a fresh, sealed volume; null when no
 * layer had anything for the key, or a paint was starved. */
export function composite_chunk_layers(stack: WorldLayerStack, key: ChunkKey, opts?: { for_raycast?: boolean }): ChunkVolume | null {
  const out = alloc_chunk_volume();
  let any_present = false;
  for (const layer of stack) any_present = layer.paint(key, out, opts) || any_present;
  // A starved volume — a band the pools had no block for — is not the
  // composite that was asked for; the caller is blocked and asks again.
  if (!any_present || out.starved) {
    release_chunk_volume(out);
    return null;
  }
  return seal_chunk_volume(out);
}
