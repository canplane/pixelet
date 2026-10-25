// @pxl/client/namespace/source.ts

/**
 * The tile source's declaration (ts/app/web/script/publish.ts writes the bucket's), asked of the
 * tile root until it answers: the published bake's out of the bucket, or a dev
 * tile server's. It is read here once: the kernel is handed what is its own,
 * the top zoom, each kind's format and the shards (WORLD_SOURCE), and observes
 * nothing until then; the versions stay here, for the objects the bakes'
 * roots are bound to (tile.ts) and for what the settings show.
 */

import { Command, encode_world_source } from "@/abi/cmd.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/config/route.ts";
import { GUEST_DOORBELL } from "@/host/mem.ts";
import { send_next } from "@/host/sched.ts";
import { online, when_online } from "@/host/online.ts";
import { BASE_LAYERS, type BaseLayer } from "@/config/published.spec.ts";
import { type Declaration, declaration } from "@/namespace/tile.ts";
import { bracket } from "@/bracket.tsx";

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
  send_next(Command.WORLD_SOURCE, pkt);
  declared = read;
  shown = BASE_LAYERS.map((kind) => ({ kind, version: read.version[kind] }));
  set(Source.Declared);
}
