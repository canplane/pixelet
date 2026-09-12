// @pxl/api/module/tile/asset/service.ts
//
// Dev-only: serves the ingested assets from ./out/asset the way the Worker
// serves them from the bucket, so `deno task dev` draws the same models and
// textures a deployment does without the bucket in the loop. Production
// never registers this (mount.ts gates the tiles module behind __IS_DEV__);
// there the same key resolves in R2 — see apps/web/worker/index.ts.

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resource.ts";

const OUT_DIR = new URL("../../../../../../out/asset/", import.meta.url);

/** A key is the sha256 of the object's bytes; the ingest writes each under
 * its type so the content-type is known without an extension. */
const HASH_PATTERN = /^[0-9a-f]{64}$/;
const TYPES = [
  ["glb", "model/gltf-binary"],
  ["png", "image/png"],
  ["webp", "image/webp"],
] as const;

export function register(router: Hono, _resrc: AppResources): void {
  router.get("/api/tiles/asset/:hash", async (c) => {
    const hash: string = c.req.param("hash");
    if (!HASH_PATTERN.test(hash)) return c.text("not found", 404);
    for (const [dir, content_type] of TYPES) {
      try {
        const bytes = await Deno.readFile(new URL(`${dir}/${hash}`, OUT_DIR));
        return c.body(bytes, 200, { "content-type": content_type, "cache-control": "public, max-age=31536000, immutable" });
      } catch (e) {
        if (!(e instanceof Deno.errors.NotFound)) throw e;
      }
    }
    return c.text("not found — run apps/api/src/bake/asset/tool/ingest_asset.ts", 404);
  });
}
