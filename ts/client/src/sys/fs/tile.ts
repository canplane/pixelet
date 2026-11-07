// @pxl/client/sys/fs/tile.ts

/**
 * The bakes' backend: a name the core opens, a tile's quadkey or the Earth's
 * sphere, made the object the source serves (spec/spec/route.json `object`)
 * under the version the declaration gives it, its finest shard's where one
 * names one. Which version a name is under is the client's to say; the name is
 * the core's. tile.fixture.json holds what the core once made of them, and
 * tile.test.ts that this makes the same.
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

/** The version `name` is served under: the finest shard over it, the one its
 * name prefixes, that names one, the last of the finest as declared; else the
 * kind's own. A shard names none of the Earth's (pxl-geo-bake source.rs). */
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

/** A bake's object by its name, under the source `declaration`. */
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
