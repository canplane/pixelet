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
import { tile_object, BASE_KINDS } from "@/abi/publication.spec.ts";
import type { Declaration } from "@/abi/types.ts";
import { tile_url } from "@/service/origin/route.ts";
import { get } from "@/service/sys/fs/read.ts";

/** The version `name` is served under. Among the shards whose quadkey
 * prefixes `name` and that declare their own version of `kind`, the finest
 * gives it. Without such a shard, the kind's own version does. No shard
 * declares a version of the Earth (pxl-geo-bake source.rs). */
export function version_of(
  d: Declaration,
  kind: (typeof BASE_KINDS)[number],
  name: string,
): string {
  const idx = BASE_KINDS.indexOf(kind);
  let finest: [number, string] | null = null;
  for (const shard of d.shards) {
    const own = shard.versions[idx] ?? null;
    if (
      own !== null &&
      name.startsWith(shard.name) &&
      (finest === null || shard.name.length >= finest[0])
    )
      finest = [shard.name.length, own];
  }
  return finest?.[1] ?? d.versions[idx]!;
}

export function read_object(
  decl: Declaration,
  kind: (typeof BASE_KINDS)[number],
  name: string,
  signal: AbortSignal,
): Promise<U8Array> {
  const object = tile_object(version_of(decl, kind, name), name);
  return get(tile_url(kind, object), signal);
}
