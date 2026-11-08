// @pxl/client/service/sys/fs/source.ts

/**
 * The tile source's declaration, which the bucket serves (script/publish.py
 * writes it) or a dev tile server answers. It is asked for until it answers,
 * and read once. The core is handed its own part of it, the top zoom, the
 * formats and the shards (SOURCE_DECLARE), and observes nothing before that.
 * The versions stay on this side, where the bakes' objects (tile.ts) and the
 * settings read them.
 */

import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { get, TILE_SOURCE_ROUTE } from "@/service/origin/route.ts";
import { online, when_online } from "@/service/origin/online.ts";
import {
  BASE_LAYERS,
  type BaseLayer,
  VERSION_BYTES,
  VERSION_CHARS,
} from "@/service/origin/publication.spec.ts";
import {
  SOURCE_ASKING,
  SOURCE_DECLARED,
  SOURCE_FAILING,
  SOURCE_REFUSED,
  type Source,
} from "@/abi/state.ts";

/** The fields of a declaration that this client reads. A shard may declare
 * its own version of a kind. */
export interface Declaration {
  top_zoom: number;
  format: Record<BaseLayer, number>;
  version: Record<BaseLayer, string>;
  shard: Record<string, { version?: Partial<Record<BaseLayer, string>> }>;
}

/** A version must be one segment of a URL, so that a tile's path stays under
 * its version. The rule is pxl-spec bake/source.rs `is_version`, and both are
 * tested against spec/golden/bake/version.tsv. */
function is_version(s: string): boolean {
  return (
    s.length >= 1 &&
    s.length <= VERSION_BYTES &&
    [...s].every((c) => VERSION_CHARS.includes(c))
  );
}

/** Reads a declaration's text, or returns null when its shape is wrong. Only
 * the shape is checked here. Whether the core follows the zoom, the formats
 * and the shard names is the core's to decide, and it refuses what it does not
 * (pxl-core sys/io/source.rs). */
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

/** The declaration handed to the core, or null before it is handed. */
export function source_declaration(): Declaration | null {
  return cur_declaration;
}

export function source_versions(): Declaration["version"] | null {
  return cur_declaration?.version ?? null;
}

export const watch_source = listeners.watch;

/** Resolves with true once the source is declared, or with false once it is
 * refused. */
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

/** On a page, the core and the asking for the declaration run in a worker.
 * The page's thread holds no files, so the UI learns the source's state and
 * declaration through this (runtime/open.ts). */
export function mirror_source(
  next: Source,
  declaration: Declaration | null,
): void {
  cur_declaration = declaration;
  set(next);
}

/** After a failed ask, the next one waits a random time below a bound. The
 * bound starts at ASK_AGAIN_MS and doubles after each failure, up to
 * ASK_AGAIN_MOST_MS. The random wait keeps clients that one outage failed
 * together from asking again together. While the device is offline, nothing
 * is asked until it says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

async function ask_declaration(): Promise<string | null> {
  try {
    const res = await get(TILE_SOURCE_ROUTE);
    if (res.ok) return await res.text();
    // The body of a refusal is not read. Cancelling it lets the connection go
    // at once, rather than when the response is collected.
    await res.body?.cancel();
    return null;
  } catch (err) {
    console.warn(
      `[source] ${err instanceof Error ? err.message : String(err)}`,
    );
    return null;
  }
}

/** Hands the core its part of the declaration (SOURCE_DECLARE). It returns
 * false when that part is more than one command can carry, or when the core
 * refuses it. The core refuses, among others, a format this build cannot
 * read. */
export type SendSource = (
  top_zoom: number,
  formats: number[],
  shards: string[],
) => boolean;

/** Resolves once the source is declared or refused. `source_state` then says
 * which. */
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
  // This is set before SOURCE_DECLARE is sent, because a frame that runs
  // inside that call already opens tiles under the declaration.
  cur_declaration = read;
  if (
    !send(
      read.top_zoom,
      BASE_LAYERS.map((kind) => read.format[kind]),
      shards,
    )
  ) {
    console.error(
      `[source] the core took no declaration of formats ${JSON.stringify(read.format)} and ${shards.length} shards`,
    );
    cur_declaration = null;
    set(SOURCE_REFUSED);
    return;
  }
  set(SOURCE_DECLARED);
}
