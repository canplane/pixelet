// @pxl/client/sys/fs/tile.ts

/**
 * The backend of the bakes' roots. The core opens a name, which is a tile's
 * quadkey or the Earth's sphere. This reads the object the source serves for
 * that name (spec/spec/route.json `object`), under the version the
 * declaration gives it. The name is the core's, and the version is the
 * client's to choose. tile.fixture.json records the object each name must
 * map to, and tile.test.ts holds this module to it.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  BASE_LAYERS,
  type BaseLayer,
  tileObject,
} from "@/origin/publication.spec.ts";
import { tile_url } from "@/origin/route.ts";
import { get } from "@/sys/fs/read.ts";
import type { Declaration } from "@/sys/fs/source.ts";

/** The version `name` is served under. Among the shards whose quadkey
 * prefixes `name` and that declare their own version of `kind`, the finest
 * gives it. Without such a shard, the kind's own version does. No shard
 * declares a version of the Earth (pxl-geo-bake source.rs). */
export function version_of(
  d: Declaration,
  kind: BaseLayer,
  name: string,
): string {
  let finest: [number, string] | null = null;
  for (const [key, shard] of Object.entries(d.shard)) {
    const own = shard.version?.[kind];
    if (
      own !== undefined &&
      name.startsWith(key) &&
      (finest === null || key.length >= finest[0])
    )
      finest = [key.length, own];
  }
  return finest?.[1] ?? d.version[kind];
}

export function read_object(
  declaration: Declaration,
  bake: u32,
  name: string,
  signal: AbortSignal,
): Promise<U8Array> {
  const kind = BASE_LAYERS[bake];
  const object = tileObject(version_of(declaration, kind, name), name);
  return get(tile_url(kind, object), signal);
}
