// @pxl/client/service/sys/fs/tile.ts

/**
 * The backend of the bakes' files. The core opens a tile by its quadkey, or
 * the Earth's sphere (binding.ts). This reads the object the source serves for
 * that name (spec/spec/bake/publication.json5 `OBJECT`), under the version
 * the declaration gives it. The name is the core's, and the version is the
 * client's to choose. tile.fixture.json records the object each name must
 * map to, and tile.test.ts holds this module to it.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { tile_object, type BASE_KINDS } from "@/abi/publication.spec.ts";
import { tileUrl } from "@/service/origin/route.ts";
import { get } from "@/service/sys/fs/read.ts";
import type { Declaration } from "@/service/sys/fs/source.ts";

/** The version `name` is served under. Among the shards whose quadkey
 * prefixes `name` and that declare their own version of `kind`, the finest
 * gives it. Without such a shard, the kind's own version does. No shard
 * declares a version of the Earth (pxl-geo-bake source.rs). */
export function versionOf(
  d: Declaration,
  kind: (typeof BASE_KINDS)[number],
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

export function readObject(
  declaration: Declaration,
  kind: (typeof BASE_KINDS)[number],
  name: string,
  signal: AbortSignal,
): Promise<U8Array> {
  const object = tile_object(versionOf(declaration, kind, name), name);
  return get(tileUrl(kind, object), signal);
}
