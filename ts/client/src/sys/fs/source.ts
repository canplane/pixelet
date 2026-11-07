// @pxl/client/sys/fs/source.ts

/**
 * The tile source's declaration (the bucket's, which script/publish.py writes,
 * or a dev tile server's), asked until it answers and read once. The kernel is
 * handed its own part, the top zoom, the formats and the shards
 * (SOURCE_DECLARE), and observes nothing before; the versions stay here, for the
 * bakes' objects (tile.ts) and the settings.
 */

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { TILE_SOURCE_ROUTE } from "@/origin/route.ts";
import { online, when_online } from "@/origin/online.ts";
import {
  BASE_LAYERS,
  type BaseLayer,
  VERSION_BYTES,
  VERSION_CHARS,
} from "@/origin/publication.spec.ts";
import {
  SOURCE_ASKING,
  SOURCE_DECLARED,
  SOURCE_FAILING,
  SOURCE_REFUSED,
  type Source,
} from "@/abi/state.ts";
import * as origin from "@/service/origin.ts";

/** What of a declaration this side reads: the top zoom and each kind's
 * format, for the kernel; each kind's version, and a shard's own. */
export interface Declaration {
  top_zoom: number;
  format: Record<BaseLayer, number>;
  version: Record<BaseLayer, string>;
  shard: Record<string, { version?: Partial<Record<BaseLayer, string>> }>;
}

/** A version is one segment of a URL, so a tile's name stays under it (pxl-spec
 * bake/source.rs `is_version`, by the same bytes and the same characters). */
function is_version(s: string): boolean {
  return (
    s.length >= 1 &&
    s.length <= VERSION_BYTES &&
    [...s].every((c) => VERSION_CHARS.includes(c))
  );
}

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
    typeof v === "object" &&
    v !== null &&
    Object.entries(v).every(
      ([k, s]) =>
        (BASE_LAYERS as readonly string[]).includes(k) &&
        typeof s === "string" &&
        is_version(s),
    ) &&
    (!every || BASE_LAYERS.every((k) => k in v));
  if (
    typeof d !== "object" ||
    d === null ||
    !("version" in d) ||
    !versions(d.version, true)
  )
    return null;
  const whole = (n: unknown) =>
    typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffffffff;
  if (!("top_zoom" in d) || !whole(d.top_zoom)) return null;
  if (
    !("format" in d) ||
    typeof d.format !== "object" ||
    d.format === null ||
    !BASE_LAYERS.every((k) => whole((d.format as Record<string, unknown>)[k]))
  )
    return null;
  if (!("shard" in d)) return null;
  const shard = d.shard;
  if (typeof shard !== "object" || shard === null || Array.isArray(shard)) {
    return null;
  }
  for (const s of Object.values(shard)) {
    if (
      typeof s !== "object" ||
      s === null ||
      ("version" in s && !versions(s.version, false))
    )
      return null;
  }
  return d as Declaration;
}

let state: Source = SOURCE_ASKING;
let cur_declaration: Declaration | null = null;
const listeners = new Listeners();

function set(next: Source): void {
  state = next;
  listeners.notify();
}

export function source_state(): Source {
  return state;
}

/** The declaration handed to the kernel; none until then. */
export function source_declaration(): Declaration | null {
  return cur_declaration;
}

/** Each kind's own version as declared; none until then. */
export function source_versions(): Declaration["version"] | null {
  return cur_declaration?.version ?? null;
}

export const watch_source = listeners.watch;

/** Settled: true where the source is declared, false where it is refused. */
export function settled(): Promise<boolean> {
  return new Promise((done) => {
    const look = () => {
      if (state === SOURCE_DECLARED || state === SOURCE_REFUSED) {
        done(state === SOURCE_DECLARED);
        return true;
      }
      return false;
    };
    if (!look()) watch_source(() => void look());
  });
}

/** Where the source stands as the kernel's worker has it, for a page's UI,
 * which holds no files. */
export function mirror_source(
  next: Source,
  declaration: Declaration | null,
): void {
  cur_declaration = declaration;
  set(next);
}

/** The most wait after a refused ask, doubled each time up to the most, and
 * the wait drawn below it (full jitter): clients one outage failed together
 * do not come back together. Offline, nothing is asked until the device
 * says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

async function ask_declaration(): Promise<string | null> {
  try {
    const res = await origin.get(TILE_SOURCE_ROUTE);
    if (res.ok) return await res.text();
    // A refusal's body is not read: let go, not held until collected.
    await res.body?.cancel();
    return null;
  } catch (err) {
    console.warn(
      `[source] ${err instanceof Error ? err.message : String(err)}`,
    );
    return null;
  }
}

/** Hands the kernel its part of the declaration (SOURCE_DECLARE); false where
 * it is more than a command carries or the kernel refuses it, a format this
 * build does not read among what it refuses. */
export type SendSource = (
  top_zoom: number,
  formats: number[],
  shards: string[],
) => boolean;

/** Settled once the source is declared or refused (`source_state`). */
export async function declare_source(send: SendSource): Promise<void> {
  let source = await ask_declaration();
  let wait_ms = ASK_AGAIN_MS;
  while (source === null) {
    if (online()) {
      set(SOURCE_FAILING);
      await new Promise((resolve) =>
        setTimeout(resolve, Math.random() * wait_ms),
      );
      wait_ms = Math.min(2 * wait_ms, ASK_AGAIN_MOST_MS);
    } else await when_online();
    source = await ask_declaration();
  }
  const read = declaration(source);
  if (read === null) {
    console.error(
      "[source] the declaration is not one this client reads: a kind unversioned, a version no URL segment, a zoom or format not a u32, or no shards",
    );
    set(SOURCE_REFUSED);
    return;
  }
  const shards = Object.keys(read.shard);
  // Before SOURCE_DECLARE: a frame in that call opens a tile under it.
  cur_declaration = read;
  if (
    !send(
      read.top_zoom,
      BASE_LAYERS.map((kind) => read.format[kind]),
      shards,
    )
  ) {
    console.error(
      `[source] the kernel took no declaration of formats ${JSON.stringify(read.format)} and ${shards.length} shards`,
    );
    cur_declaration = null;
    set(SOURCE_REFUSED);
    return;
  }
  set(SOURCE_DECLARED);
}
