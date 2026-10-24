// @pxl/client/kernel/io/object.ts

/**
 * A bake's object as the kernel names it, a number (host.json `root`), made
 * the object the source serves (spec/spec/route.json `object`): a tile's
 * subject its name, under the version the declaration gives it, its finest
 * shard's own where one names one; or the Earth's sphere (`SPHERE`), under
 * the earth's own version. The kernel reads the declaration for what is its
 * own, the zooms, the shards and the formats, and never its versions:
 * which object a number is, is pixelet's end of the kernel's to say.
 * object.fixture.json holds what the kernel made of these when it named
 * them itself, and object.test.ts that this makes the same.
 */

import type { BaseLayer } from "@/config/published.spec.ts";
import { BASE_LAYERS } from "@/config/published.spec.ts";
import { TILE_SPHERE, tileObject } from "@/config/route.spec.ts";
import { SPHERE } from "@/kernel/abi/host.spec.ts";

/** What of a declaration this side reads: each kind's version, and a shard's own. */
export interface Declaration {
  version: Record<BaseLayer, string>;
  shard: Record<string, { version?: Partial<Record<BaseLayer, string>> }>;
}

/** A version is one segment of a URL, so a tile's name stays under it (pxl-spec bake/source.rs `is_version`). */
const VERSION = /^[A-Za-z0-9._-]{1,32}$/;

/** The declaration's text, where every kind has a version and every version
 * is a URL's segment; none where it is not, and the kernel, never handed it,
 * observes nothing. The rest is the kernel's to refuse (pxl-kernel io/source.rs). */
export function declaration(text: string): Declaration | null {
  let d: unknown;
  try {
    d = JSON.parse(text);
  } catch {
    return null;
  }
  const versions = (v: unknown, every: boolean) =>
    typeof v === "object" && v !== null &&
    Object.entries(v).every(([k, s]) => (BASE_LAYERS as readonly string[]).includes(k) && typeof s === "string" && VERSION.test(s)) &&
    (!every || BASE_LAYERS.every((k) => k in v));
  if (typeof d !== "object" || d === null || !("version" in d) || !versions(d.version, true)) return null;
  const shard = "shard" in d ? d.shard : {};
  if (typeof shard !== "object" || shard === null || Array.isArray(shard)) return null;
  for (const s of Object.values(shard)) {
    if (typeof s !== "object" || s === null || ("version" in s && !versions(s.version, false))) return null;
  }
  return d as Declaration;
}

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
