// @pxl/web/kernel/io/source.ts

/**
 * The tile source's declaration (its coarsest whole zoom), handed to the
 * kernel once: published.json's `SOURCE` in a published build, else asked of
 * the dev server until it answers. The kernel observes nothing until then.
 */

import { Command, encode_world_source } from "@/kernel/abi/cmd.spec.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_SOURCE, type TileSource } from "@/config/published.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/config/route.ts";
import { originGet } from "@platform/origin.ts";
import { send_next } from "@/kernel/sched.ts";

const ASK_AGAIN_MS = 2_000;

async function asked(): Promise<TileSource | null> {
  try {
    const res = await originGet(TILE_SOURCE_ROUTE);
    if (!res.ok) return null;
    const top: unknown = ((await res.json()) as Partial<TileSource> | null)?.top_z;
    return typeof top === "number" && Number.isInteger(top) && top >= 0 ? { top_z: top } : null;
  } catch (err) {
    console.warn(`[source] ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

export async function declare_source(): Promise<void> {
  let source = PUBLISHED_TILES ? PUBLISHED_SOURCE : await asked();
  while (source === null) {
    await new Promise((resolve) => setTimeout(resolve, ASK_AGAIN_MS));
    source = await asked();
  }
  send_next(Command.WORLD_SOURCE, encode_world_source({ top_z: source.top_z }));
}
