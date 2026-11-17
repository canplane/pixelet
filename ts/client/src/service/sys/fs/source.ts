// @pxl/client/service/sys/fs/source.ts

/**
 * The tile source's declaration, which the bucket serves (script/publish.py
 * writes it) or a dev tile server answers. It is asked for until it answers,
 * and read once. The core is handed its own part of it, the top zoom, the
 * formats and the shards (pxl:core source `declare`), and observes nothing
 * before that.
 * The versions stay on this side, where the bakes' objects (tile.ts) and the
 * settings read them.
 */

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import type { Sender } from "@xpute/link";
import { declare } from "@/abi/source.ts";
import { get, TILE_SOURCE_ROUTE } from "@/service/origin/route.ts";
import { online, whenOnline } from "@/service/origin/online.ts";
import {
  BASE_KINDS,
  VERSION_SZ,
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
function isVersion(s: string): boolean {
  return (
    s.length >= 1 &&
    s.length <= VERSION_SZ &&
    [...s].every((c) => VERSION_CHARS.includes(c))
  );
}

/** Reads a declaration parsed from its JSON, or returns null when its shape is
 * wrong. Only the shape is checked here. Whether the core follows the zoom, the
 * formats and the shard names is the core's to decide, and it refuses what it
 * does not (pxl-core sys/io/source.rs). */
export function declarationOf(parsed: unknown): Declaration | null {
  const versions = (v: unknown, every: boolean) =>
    typeof v === "object" &&
    v !== null &&
    Object.entries(v).every(
      ([k, s]) =>
        (BASE_KINDS as readonly string[]).includes(k) &&
        typeof s === "string" &&
        isVersion(s),
    ) &&
    (!every || BASE_KINDS.every((k) => k in v));
  if (
    typeof parsed !== "object" ||
    parsed === null ||
    !("version" in parsed) ||
    !versions(parsed.version, true)
  )
    return null;
  const whole = (n: unknown) =>
    typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 0xffffffff;
  if (!("min_zoom" in parsed) || !whole(parsed.min_zoom)) return null;
  if (
    !("format" in parsed) ||
    typeof parsed.format !== "object" ||
    parsed.format === null ||
    !BASE_KINDS.every((k) =>
      whole((parsed.format as Record<string, unknown>)[k]),
    )
  )
    return null;
  if (!("shard" in parsed)) return null;
  const shard = parsed.shard;
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
  return parsed as Declaration;
}

let state: Source = SOURCE_ASKING;
let currentDeclaration: Declaration | null = null;
const listeners = new ListenerSet();

function set(next: Source): void {
  state = next;
  listeners.notify();
}

export function sourceState(): Source {
  return state;
}

/** The declaration the core took, or null before it took one. */
export function sourceDeclaration(): Declaration | null {
  return currentDeclaration;
}

export function sourceVersions(): Declaration["version"] | null {
  return currentDeclaration?.version ?? null;
}

export const watchSource = listeners.watch;

/** Resolves with the declaration once the core takes it, and rejects once
 * the source is refused or `signal` aborts: a file opened under it waits for
 * it (binding.ts), as a reloaded native UI opens the tiles a running core
 * asks for again before it has declared the source. What it watches goes
 * when it settles. */
export function whenSourceDeclared(signal: AbortSignal): Promise<Declaration> {
  return new Promise((resolve, reject) => {
    const settle = (): boolean => {
      if (currentDeclaration !== null) resolve(currentDeclaration);
      else if (state === SOURCE_REFUSED)
        reject(new Error("[source] the tile source is refused"));
      else if (signal.aborted) reject(signal.reason);
      else return false;
      return true;
    };
    if (settle()) return;
    const onChange = () => {
      if (!settle()) return;
      stop();
      signal.removeEventListener("abort", onChange);
    };
    const stop = watchSource(onChange);
    signal.addEventListener("abort", onChange);
  });
}

/** On the web, the core and the asking for the declaration run in a worker.
 * The page's thread holds no files, so the UI learns the source's state and
 * declaration through this (runtime/worker.ts). */
export function mirrorSource(
  next: Source,
  declaration: Declaration | null,
): void {
  currentDeclaration = declaration;
  set(next);
}

/** After a failed ask, the next one waits a random time below a bound. The
 * bound starts at `ASK_AGAIN_MS` and doubles after each failure, up to
 * `ASK_AGAIN_MOST_MS`. The random wait keeps clients that one outage failed
 * together from asking again together. While the device is offline, nothing
 * is asked until it says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

/** Asks for the declaration and answers its parsed JSON, or null where the ask
 * failed. A body that is not JSON is a failed ask, asked again, not a refusal:
 * a connection cut while the body comes in may still end in an ok response
 * with a short body (Expo's fetch on Android resolves `text()` so), and a
 * captive portal answers ok with a page of its own. */
async function askDeclaration(): Promise<unknown> {
  try {
    const res = await get(TILE_SOURCE_ROUTE);
    if (res.ok) return JSON.parse(await res.text()) as unknown;
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

/** Resolves once the source is declared or refused, `sourceState` then
 * saying which. The core is handed its part of the declaration over `link`
 * (pxl:core source `declare`). It is refused when that part is more than one
 * frame can carry, or when the core refuses it, as it does, among others, a
 * format this build cannot read. */
export async function declareSource(link: Sender): Promise<void> {
  let source = await askDeclaration();
  let waitMs = ASK_AGAIN_MS;
  while (source === null) {
    if (online()) {
      set(SOURCE_FAILING);
      await new Promise((resolve) =>
        setTimeout(resolve, Math.random() * waitMs),
      );
      waitMs = Math.min(2 * waitMs, ASK_AGAIN_MOST_MS);
    } else await whenOnline();
    source = await askDeclaration();
  }
  const read = declarationOf(source);
  if (read === null) {
    console.error(
      "[source] the declaration is not one this client reads: a kind unversioned, a version no URL segment, a zoom or format not a u32, or no shards",
    );
    set(SOURCE_REFUSED);
    return;
  }
  const shards = Object.keys(read.shard);
  const formats = Uint32Array.from(BASE_KINDS, (kind) => read.format[kind]);
  try {
    await declare(link, read.min_zoom, formats, shards);
  } catch {
    console.error(
      `[source] the core took no declaration of formats ${JSON.stringify(read.format)} and ${shards.length} shards`,
    );
    set(SOURCE_REFUSED);
    return;
  }
  currentDeclaration = read;
  set(SOURCE_DECLARED);
}
