// @pxl/client/service/sys/fs/tile.ts

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
} from "@/service/origin/publication.spec.ts";
import { href, tile_url } from "@/service/origin/route.ts";
import { get } from "@/service/sys/fs/read.ts";
import type { Declaration } from "@/service/sys/fs/source.ts";

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

function route_of(decl: Declaration, bake: u32, name: string): string {
  const kind = BASE_LAYERS[bake];
  return tile_url(kind, tileObject(version_of(decl, kind, name), name));
}

export function read_object(
  decl: Declaration,
  bake: u32,
  name: string,
  signal: AbortSignal,
): Promise<U8Array> {
  return get(route_of(decl, bake, name), signal);
}

/** The whole URL `read_object` reads. */
export function object_url(decl: Declaration, bake: u32, name: string): string {
  return href(route_of(decl, bake, name));
}
