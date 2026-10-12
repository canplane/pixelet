// @pxl/web/kernel/io/source.ts

/**
 * The tile source's declaration (published.json's `SOURCE`), asked of the
 * tile root until it answers and handed to the kernel unread: the published
 * bake's out of the bucket, or a dev tile server's. The kernel observes
 * nothing until then.
 */

import { Command, encode_world_source } from "@/kernel/abi/cmd.spec.ts";
import type { BaseLayer } from "@/config/published.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/config/route.ts";
import { originGet } from "@platform/origin.ts";
import { GUEST_DOORBELL } from "@/kernel/mem.ts";
import { send_next } from "@/kernel/sched.ts";

const ASK_AGAIN_MS = 2_000;

/** Each kind's path as declared, its published version, for showing; none from a dev server. */
export let declared_path: Partial<Record<BaseLayer, string>> = {};

async function asked(): Promise<string | null> {
  try {
    const res = await originGet(TILE_SOURCE_ROUTE);
    return res.ok ? await res.text() : null;
  } catch (err) {
    console.warn(`[source] ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

export async function declare_source(): Promise<void> {
  let source = await asked();
  while (source === null) {
    await new Promise((resolve) => setTimeout(resolve, ASK_AGAIN_MS));
    source = await asked();
  }
  const pkt = encode_world_source({ source });
  if (!GUEST_DOORBELL.fits(pkt)) {
    console.error(`[source] the declaration, ${source.length} bytes, is more than a command carries`);
    return;
  }
  try {
    declared_path = (JSON.parse(source) as { path?: Partial<Record<BaseLayer, string>> }).path ?? {};
  } catch {
    // The kernel refuses what does not parse, and says so.
  }
  send_next(Command.WORLD_SOURCE, pkt);
}
