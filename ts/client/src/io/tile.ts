// @pxl/client/io/tile.ts

/**
 * The bakes' backend: a number the kernel names (host.json `root`) made the
 * object the source serves (spec/spec/route.json `object`), a tile by its
 * subject under the version the declaration gives it, its finest shard's
 * where one names one, or the Earth's sphere (`SPHERE`). Which object a
 * number is, is the client's to say; tile.fixture.json holds what the kernel
 * once made of them, and tile.test.ts that this makes the same.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { BASE_LAYERS, type BaseLayer } from "@/config/published.spec.ts";
import { TILE_SPHERE, tileObject } from "@/config/route.spec.ts";
import { tile_url } from "@/config/route.ts";
import { SPHERE } from "@/abi/host.spec.ts";
import { get } from "@/io/read.ts";
import type { Declaration } from "@/io/source.ts";

/** A tile's name by its subject, a digit a zoom (pxl-spec space/hierarchy/morton.rs
 * `write_name`, two axes, each a digit from z1): the subject is its key's high
 * half, its digits from the top two a zoom, then a 1. None for a number that is
 * no tile's. */
export function tile_name(subject: bigint): string | null {
  if (subject <= 0n || subject >= 1n << 64n) return null;
  let low = 0;
  while (((subject >> BigInt(low)) & 1n) === 0n) low++;
  if ((62 - low) % 2 !== 0) return null;
  let name = "";
  for (let z = 0; z < (62 - low) / 2; z++) name += String((subject >> BigInt(61 - 2 * z)) & 3n);
  return name;
}

/** The version `name` is served under: the finest shard over it, the one its
 * name prefixes, that names one, the last of the finest as declared; else the
 * kind's own. The Earth's picture is no shard's (pxl-spec bake/source.rs `sharded`). */
export function version_of(d: Declaration, kind: BaseLayer, name: string): string {
  let finest: [number, string] | null = null;
  if (kind !== "earth") {
    for (const [key, shard] of Object.entries(d.shard)) {
      const own = shard.version?.[kind];
      if (own !== undefined && name.startsWith(key) && (finest === null || key.length >= finest[0])) finest = [key.length, own];
    }
  }
  return finest?.[1] ?? d.version[kind];
}

/** The object `number`, a path's one name, is below `kind`'s root; none for one that is no object's. */
export function tile_object(d: Declaration, kind: BaseLayer, number: bigint): string | null {
  if (number === BigInt(SPHERE)) return kind === "earth" ? tileObject(d.version.earth, TILE_SPHERE) : null;
  const name = tile_name(number);
  return name === null ? null : tileObject(version_of(d, kind, name), name);
}

/**
 * A body still packed means the transport did not decode the stored
 * encoding: a browser that cannot, which the Worker cannot refuse, since
 * Cloudflare rewrites `accept-encoding` before the Worker sees it; or a
 * device whose own decode (ts/app/native/src/origin.ts) was skipped.
 */
function packed(body: U8Array): boolean {
  return (body[0] === 0x28 && body[1] === 0xb5) || (body[0] === 0x1f && body[1] === 0x8b);
}

/** A bake's object by its number, under the source `declared`; none before
 * the source is declared, as the kernel asks for none. */
export async function read_object(declared: Declaration | null, bake: u32, number: bigint, signal: AbortSignal): Promise<U8Array> {
  const name = declared === null ? null : tile_object(declared, BASE_LAYERS[bake], number);
  if (name === null) throw new FileError(Errno.ENOENT);
  const body = await get(tile_url(BASE_LAYERS[bake], name), name, signal);
  if (body.byteLength > 0 && packed(body)) {
    // Fail here: handing it on would fail in the kernel's decoder, far from the cause.
    console.warn(`[read] ${name}: the body arrived still packed, with no encoding to undo it`);
    throw new FileError(Errno.EBADMSG);
  }
  return body;
}
