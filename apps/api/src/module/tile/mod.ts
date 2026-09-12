// @pxl/api/module/tile/mod.ts
//
// On-demand bake for local dev only, and the ingested assets from disk.
// Production serves these from R2
// (see .agents/skills/bake-tiles/SKILL.md); mount.ts gates load() behind
// __IS_DEV__ so a deployed build never registers these routes at all.

import type { ServerContext } from "@/main.ts";
import type { AppResources } from "@/app/resource.ts";

import * as terrain_service from "./terrain/service.ts";
import * as poi_service from "./poi/service.ts";
import * as geocode_service from "./geocode/service.ts";
import * as asset_service from "./asset/service.ts";

export function load(ctx: ServerContext, resrc: AppResources) {
  terrain_service.register(ctx.router, resrc);
  poi_service.register(ctx.router, resrc);
  geocode_service.register(ctx.router, resrc);
  asset_service.register(ctx.router, resrc);
}
