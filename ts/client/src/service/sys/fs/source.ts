// @pxl/client/service/sys/fs/source.ts

/**
 * The tile source's declaration, which the bucket serves (script/publish.py
 * writes it) or a dev tile server answers. It is asked for until it answers,
 * and read once, its published JSON into the core's record (`decl`). The core
 * is handed it whole (pxl:core source `declare`) and observes nothing before
 * that; it keeps it for its life and tells it to the UI as it is made and to
 * each UI that attaches (overlay `source`). What the core tells is the one
 * thing that makes the source declared on this side, the copy the bakes'
 * objects (tile.ts) and the settings read the versions from (`take_source`),
 * so a new UI on the same core takes it from there and declares nothing.
 */

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import type { Sender } from "@xpute/link";
import { declare } from "@/abi/source.ts";
import type { Declaration } from "@/abi/types.ts";
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
/** How the asking stands, before the core tells its declaration. */
export type Asking =
  typeof SOURCE_ASKING | typeof SOURCE_FAILING | typeof SOURCE_REFUSED;
export type Source = Asking | typeof SOURCE_DECLARED;

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

/** Reads a declaration's published JSON into the core's record, each kind's
 * format and version a list in `BASE_KINDS`' order and each shard's versions
 * too, none where it takes the source's, or returns null when its shape is
 * wrong. Only the shape is checked here. Whether the core follows the zoom,
 * the formats and the shard names is the core's to decide, and it refuses
 * what it does not (pxl-core sys/io/source.rs). */
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
  const shards = d.shard as Record<
    string,
    { version?: Record<string, string> }
  >;
  if (typeof shards !== "object" || shards === null || Array.isArray(shards)) {
    return null;
  }
  for (const s of Object.values(shards)) {
    if (
      typeof s !== "object" ||
      s === null ||
      ("version" in s && !versions(s.version, false))
    )
      return null;
  }
  const format = d.format as Record<string, number>;
  const version = d.version as Record<string, string>;
  return {
    min_zoom: d.min_zoom as number,
    formats: Uint32Array.from(BASE_KINDS, (kind) => format[kind]!),
    versions: BASE_KINDS.map((kind) => version[kind]!),
    shards: Object.entries(shards).map(([name, s]) => ({
      name,
      versions: BASE_KINDS.map((kind) => s.version?.[kind] ?? null),
    })),
  };
}

/** How the asking stands, this side's (`declare_source`) or, on a page, the
 * worker's (`mirror_source`); and the declaration the core told
 * (`take_source`), which alone makes the source declared. */
let asking: Asking = SOURCE_ASKING;
let cur_decl: Declaration | null = null;
const listeners = new ListenerSet();

function set(next: Asking): void {
  asking = next;
  listeners.notify();
}

export function source_state(): Source {
  return cur_decl === null ? asking : SOURCE_DECLARED;
}

/** How the asking stands, which a page's worker mirrors to the page. */
export function source_asking(): Asking {
  return asking;
}

/** Each kind's version, in `BASE_KINDS`' order, or null before the core told
 * its declaration. */
export function source_versions(): readonly string[] | null {
  return cur_decl?.versions ?? null;
}

export const watch_source = listeners.watch;

/** Resolves with the declaration the core holds, once it has told it: a file
 * opened under it waits for it (binding.ts), as a page's worker hears it
 * through the page (runtime/boot.ts) after the core opens its first tiles. */
export function when_source_declared(): Promise<Declaration> {
  return new Promise((resolve) => {
    if (cur_decl !== null) return resolve(cur_decl);
    const stop = watch_source(() => {
      if (cur_decl === null) return;
      stop();
      resolve(cur_decl);
    });
  });
}

/** On a page, the core and the asking for the declaration run in a worker.
 * The page's thread holds no files, so the UI learns how the worker's asking
 * stands through this (runtime/worker.ts `source_asking`), and the
 * declaration from the core (`take_source`). */
export function mirror_source(next: Asking): void {
  set(next);
}

/** Takes the declaration `decl` the core holds for its life, as it tells it
 * (overlay `source`): as it is declared, and to a new UI on the same core, as
 * a device's runs on beneath a reloaded UI, which then declares nothing. It
 * alone makes the source declared on this side. */
export function take_source(decl: Declaration): void {
  cur_decl = decl;
  listeners.notify();
}

/** After a failed ask, the next one waits a random time below a bound. The
 * bound starts at `ASK_AGAIN_MS` and doubles after each failure, up to
 * `ASK_AGAIN_MOST_MS`. The random wait keeps clients that one outage failed
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

/** Asks for the declaration and hands it to the core over `link` (pxl:core
 * source `declare`), which tells it back (`take_source`); a refusal is this
 * side's to state: it is refused when it is more than one frame can carry, or
 * when the core refuses it, as it does, among others, a format this build
 * cannot read. */
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
  try {
    await declare(link, read);
  } catch {
    console.error(
      `[source] the core took no declaration of formats ${[...read.formats].join(", ")} and ${read.shards.length} shards`,
    );
    set(SOURCE_REFUSED);
  }
}
