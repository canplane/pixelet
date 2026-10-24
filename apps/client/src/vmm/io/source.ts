// @pxl/client/vmm/io/source.ts

/**
 * The tile source's declaration (script/publish.ts writes the bucket's), asked of the
 * tile root until it answers: the published bake's out of the bucket, or a dev
 * tile server's. The kernel is handed it unread and observes nothing until
 * then; its versions are read here alone, for the objects the bakes' roots are
 * bound to (io/object.ts) and for what the settings show.
 */

import { useSyncExternalStore } from "react";

import { Command, encode_world_source } from "@/vmm/abi/cmd.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/config/route.ts";
import { originGet } from "@/bracket.tsx";
import { GUEST_DOORBELL } from "@/vmm/mem.ts";
import { send_next } from "@/vmm/sched.ts";
import { online, whenOnline } from "@/vmm/online.ts";
import { BASE_LAYERS, type BaseLayer } from "@/config/published.spec.ts";
import { type Declaration, declaration } from "@/vmm/io/object.ts";

let declared: Declaration | null = null;
let shown: readonly { kind: BaseLayer; version: string }[] = [];
const listeners = new Set<() => void>();

/** The declaration handed to the kernel; none until then. */
export function source_declared(): Declaration | null {
  return declared;
}

/** Each kind's own version as declared, empty until then. */
export function useSourceVersions(): readonly { kind: BaseLayer; version: string }[] {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, () => shown);
}

/** The wait after a refused ask, doubled each time up to the most: a source
 * that does not answer is not asked again at a pace. Offline, nothing is
 * asked until the device says it is online. */
const ASK_AGAIN_MS = 2_000;
const ASK_AGAIN_MOST_MS = 60_000;

async function asked(): Promise<string | null> {
  try {
    const res = await originGet(TILE_SOURCE_ROUTE);
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
      await new Promise((resolve) => setTimeout(resolve, wait_ms));
      wait_ms = Math.min(2 * wait_ms, ASK_AGAIN_MOST_MS);
    } else await whenOnline();
    source = await asked();
  }
  const read = declaration(source);
  if (read === null) {
    console.error("[source] the declaration leaves a kind unversioned, or names a version no URL segment");
    return;
  }
  const pkt = encode_world_source(GUEST_DOORBELL, source);
  if (!GUEST_DOORBELL.fits(pkt)) {
    console.error(`[source] the declaration, ${source.length} bytes, is more than a command carries`);
    return;
  }
  send_next(Command.WORLD_SOURCE, pkt);
  declared = read;
  shown = BASE_LAYERS.map((kind) => ({ kind, version: read.version[kind] }));
  for (const listener of listeners) listener();
}
