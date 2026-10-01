// @pxl/web/kernel/io/source.ts

/**
 * What the tile source declares of itself, handed to the kernel once
 * (WORLD_SOURCE): the coarsest level of the pyramid it supplies whole, where
 * the kernel holds its floor (crates/pxl/web/kernel/src/world/paging/floor.rs).
 *
 * A published bake's declaration is published.json's `SOURCE`, in the bundle.
 * A dev tile server bakes on demand to its own `--top-level` and answers the
 * same object at `TILE_SOURCE_ROUTE`. Which of the two is read turns on the
 * one fact `tileUrl` already turns on, whether tiles come from a published
 * bake, and the kernel never learns which it was.
 *
 * Until the declaration arrives the kernel observes nothing — a cold start,
 * and never a floor taken at a level the source may not supply — so a dev
 * server not answering yet (it may still be compiling) is asked again rather
 * than guessed at.
 */

import { Command, encode_world_source } from "@/kernel/abi/cmd.spec.ts";
import { PUBLISHED_TILES } from "@/config/deploy.ts";
import { PUBLISHED_SOURCE, type TileSource } from "@/config/published.spec.ts";
import { TILE_SOURCE_ROUTE } from "@/config/route.ts";
import { originGet } from "@platform/origin.ts";
import { send_next } from "@/kernel/sched.ts";

/** How long a source that did not answer waits before it is asked again, in
 * milliseconds: a dev server compiling or restarting answers within seconds,
 * and nothing is drawn meanwhile either way. */
const ASK_AGAIN_MS = 2_000;

/** The dev server's declaration, or null where it did not give one. */
async function asked(): Promise<TileSource | null> {
  try {
    const res = await originGet(TILE_SOURCE_ROUTE);
    if (!res.ok) return null;
    const top: unknown = ((await res.json()) as Partial<TileSource> | null)?.top_level;
    return typeof top === "number" && Number.isInteger(top) && top >= 0 ? { top_level: top } : null;
  } catch (err) {
    console.warn(`[source] ${err instanceof Error ? err.message : String(err)}`);
    return null;
  }
}

/** Reads the source's declaration, asking until it answers, and hands it to
 * the kernel. */
export async function declare_source(): Promise<void> {
  let source = PUBLISHED_TILES ? PUBLISHED_SOURCE : await asked();
  while (source === null) {
    await new Promise((resolve) => setTimeout(resolve, ASK_AGAIN_MS));
    source = await asked();
  }
  send_next(Command.WORLD_SOURCE, encode_world_source({ top_level: source.top_level }));
}
