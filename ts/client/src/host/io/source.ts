// @pxl/client/host/io/source.ts

/**
 * The tile source's declaration (the bucket's, which script/publish.py writes,
 * or a dev tile server's), asked until it answers and read once. The kernel is
 * handed its own part, the top zoom, the formats and the shards
 * (WORLD_SOURCE), and observes nothing before; the versions stay here, for the
 * bakes' objects (tile.ts) and the settings.
 */

import { Command, encode_world_source } from "@/host/abi/cmd.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/host/config/route.ts";
import { GUEST_DOORBELL, HOST } from "@/host/mem.ts";
import { online, when_online } from "@/host/online.ts";
import { BASE_LAYERS, type BaseLayer } from "@/host/config/published.spec.ts";
import { bracket } from "@/bracket.tsx";

/** What of a declaration this side reads: the top zoom and each kind's
 * format, for the kernel; each kind's version, and a shard's own. */
export interface Declaration {
  top_zoom: number;
  format: Record<BaseLayer, number>;
  version: Record<BaseLayer, string>;
  shard: Record<string, { version?: Partial<Record<BaseLayer, string>> }>;
}

/** A version is one segment of a URL, so a tile's name stays under it (pxl-spec bake/source.rs `is_version`). */
const VERSION = /^[A-Za-z0-9._-]{1,32}$/;

/** The declaration's text, where every kind has a version and every version
 * is a URL's segment, the top zoom and each kind's format are u32s, and the
 * shards are named; none where it is not, and the kernel, never handed it,
 * observes nothing. Which zooms, formats and shard names it follows is the
 * kernel's to refuse (pxl-kernel sys/io/source.rs). */
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
  const whole = (n: unknown) => typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffffffff;
  if (!("top_zoom" in d) || !whole(d.top_zoom)) return null;
  if (!("format" in d) || typeof d.format !== "object" || d.format === null || !BASE_LAYERS.every((k) => whole((d.format as Record<string, unknown>)[k]))) return null;
  if (!("shard" in d)) return null;
  const shard = d.shard;
  if (typeof shard !== "object" || shard === null || Array.isArray(shard)) return null;
  for (const s of Object.values(shard)) {
    if (typeof s !== "object" || s === null || ("version" in s && !versions(s.version, false))) return null;
  }
  return d as Declaration;
}

/** Where the declaration stands: asked, failing (refused or unanswered, and asked again), refused for good (one this client cannot read), or declared. */
export const enum Source {
  Asking = 0,
  Failing = 1,
  Refused = 2,
  Declared = 3,
}

let state = Source.Asking;
let declared: Declaration | null = null;
let shown: readonly { kind: BaseLayer; version: string }[] = [];
const listeners = new Set<() => void>();

function set(next: Source): void {
  state = next;
  for (const listener of listeners) listener();
}

export function source_state(): Source {
  return state;
}

/** The declaration handed to the kernel; none until then. */
export function source_declared(): Declaration | null {
  return declared;
}

/** Each kind's own version as declared, empty until then. */
export function source_versions(): readonly { kind: BaseLayer; version: string }[] {
  return shown;
}

export function watch_source(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The most wait after a refused ask, doubled each time up to the most, and
 * the wait drawn below it (full jitter): clients one outage failed together
 * do not come back together. Offline, nothing is asked until the device
 * says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

async function asked(): Promise<string | null> {
  try {
    const res = await bracket.origin.get(TILE_SOURCE_ROUTE);
    if (res.ok) return await res.text();
    // A refusal's body is not read: let go, not held until collected.
    await res.body?.cancel();
    return null;
  } catch (err) {
    console.warn(`[source] ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

export async function declare_source(): Promise<void> {
  let source = await asked();
  let wait_ms = ASK_AGAIN_MS;
  while (source === null) {
    if (online()) {
      set(Source.Failing);
      await new Promise((resolve) => setTimeout(resolve, Math.random() * wait_ms));
      wait_ms = Math.min(2 * wait_ms, ASK_AGAIN_MOST_MS);
    } else await when_online();
    source = await asked();
  }
  const read = declaration(source);
  if (read === null) {
    console.error("[source] the declaration is not one this client reads: a kind unversioned, a version no URL segment, a zoom or format not a u32, or no shards");
    set(Source.Refused);
    return;
  }
  const shards = Object.keys(read.shard);
  const pkt = encode_world_source(GUEST_DOORBELL, read.top_zoom, BASE_LAYERS.map((kind) => read.format[kind]), shards);
  if (!GUEST_DOORBELL.fits(pkt)) {
    console.error(`[source] the declaration's ${shards.length} shards are more than a command carries`);
    set(Source.Refused);
    return;
  }
  HOST.send(Command.WORLD_SOURCE, pkt);
  declared = read;
  shown = BASE_LAYERS.map((kind) => ({ kind, version: read.version[kind] }));
  set(Source.Declared);
}
