// @pxl/client/service/sys/fs/source.ts

/**
 * The tile source's declaration, which the bucket serves (script/publish.py
 * writes it) or a dev tile server answers. It is asked for until it answers,
 * and read once. The core is handed its own part of it, the top zoom, the
 * formats and the shards (SOURCE_DECLARE), and observes nothing before that.
 * The versions stay on this side, where the bakes' objects (tile.ts) and the
 * settings read them.
 */

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import type { Sender } from "@xpute/link";
import { declare } from "@/abi/source.ts";
import { get, TILE_SOURCE_ROUTE } from "@/service/origin/route.ts";
import { online, when_online } from "@/service/origin/online.ts";
import {
  BASE_KINDS,
  VERSION_BYTES,
  VERSION_CHARS,
} from "@/abi/publication.spec.ts";

/**
 * Where the world's source declaration stands:
 * - asking: it is asked for;
 * - failing: an ask was refused or went unanswered, and it is asked again;
 * - refused: it is refused for good, as one this client cannot read;
 * - declared: it is declared.
 */
export const SOURCE_ASKING = 0;
export const SOURCE_FAILING = 1;
export const SOURCE_REFUSED = 2;
export const SOURCE_DECLARED = 3;
export type Source =
  | typeof SOURCE_ASKING
  | typeof SOURCE_FAILING
  | typeof SOURCE_REFUSED
  | typeof SOURCE_DECLARED;

/** The fields of a declaration that this client reads. A shard may declare
 * its own version of a kind. */
export interface Declaration {
  min_zoom: number;
  format: Record<(typeof BASE_KINDS)[number], number>;
  version: Record<(typeof BASE_KINDS)[number], string>;
  shard: Record<
    string,
    { version?: Partial<Record<(typeof BASE_KINDS)[number], string>> }
  >;
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
export function decl(text: string): Declaration | null {
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
        (BASE_KINDS as readonly string[]).includes(k) &&
        typeof s === "string" &&
        is_version(s),
    ) &&
    (!every || BASE_KINDS.every((k) => k in v));
  if (
    typeof d !== "object" ||
    d === null ||
    !("version" in d) ||
    !versions(d.version, true)
  )
    return null;
  const whole = (n: unknown) =>
    typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffffffff;
  if (!("min_zoom" in d) || !whole(d.min_zoom)) return null;
  if (
    !("format" in d) ||
    typeof d.format !== "object" ||
    d.format === null ||
    !BASE_KINDS.every((k) => whole((d.format as Record<string, unknown>)[k]))
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
let cur_decl: Declaration | null = null;
const listeners = new ListenerSet();

function set(next: Source): void {
  state = next;
  listeners.notify();
}

export function source_state(): Source {
  return state;
}

/** The declaration handed to the core, or null before it is handed. */
export function source_decl(): Declaration | null {
  return cur_decl;
}

export function source_versions(): Declaration["version"] | null {
  return cur_decl?.version ?? null;
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
 * declaration through this (runtime/worker.ts). */
export function mirror_source(next: Source, decl: Declaration | null): void {
  cur_decl = decl;
  set(next);
}

/** After a failed ask, the next one waits a random time below a bound. The
 * bound starts at ASK_AGAIN_MS and doubles after each failure, up to
 * ASK_AGAIN_MOST_MS. The random wait keeps clients that one outage failed
 * together from asking again together. While the device is offline, nothing
 * is asked until it says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

async function ask_decl(): Promise<string | null> {
  try {
    const res = await get(TILE_SOURCE_ROUTE);
    if (res.ok) return await res.text();
    // The body of a refusal is not read. Canceling it lets the connection go
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

/** Resolves once the source is declared or refused, `source_state` then
 * saying which. The core is handed its part of the declaration over `link`
 * (SOURCE_DECLARE). It is refused when that part is more than one command
 * can carry, or when the core refuses it, as it does, among others, a format
 * this build cannot read. */
export async function declare_source(link: Sender): Promise<void> {
  let source = await ask_decl();
  let wait_ms = ASK_AGAIN_MS;
  while (source === null) {
    if (online()) {
      set(SOURCE_FAILING);
      await new Promise((resolve) =>
        setTimeout(resolve, Math.random() * wait_ms),
      );
      wait_ms = Math.min(2 * wait_ms, ASK_AGAIN_MOST_MS);
    } else await when_online();
    source = await ask_decl();
  }
  const read = decl(source);
  if (read === null) {
    console.error(
      "[source] the declaration is not one this client reads: a kind unversioned, a version no URL segment, a zoom or format not a u32, or no shards",
    );
    set(SOURCE_REFUSED);
    return;
  }
  const shards = Object.keys(read.shard);
  // This is set before SOURCE_DECLARE is sent, because the frame that takes
  // it already opens tiles under the declaration.
  cur_decl = read;
  const formats = Uint32Array.from(BASE_KINDS, (kind) => read.format[kind]);
  try {
    await declare(link, read.min_zoom, formats, shards);
  } catch {
    console.error(
      `[source] the core took no declaration of formats ${JSON.stringify(read.format)} and ${shards.length} shards`,
    );
    cur_decl = null;
    set(SOURCE_REFUSED);
    return;
  }
  set(SOURCE_DECLARED);
}
