// @pxl/api/modules/tiles/mod.ts
//
// On-demand bake for local dev only. Production serves these from R2
// (see .agents/skills/bake-tiles/SKILL.md); mount.ts gates load() behind
// __IS_DEV__ so a deployed build never registers these routes at all.

import type { ServerContext } from "@/main.ts";
import type { AppResources } from "@/app/resources.ts";

import * as terrain_service from "./terrain/service.ts";
import * as poi_service from "./poi/service.ts";
import * as geocode_service from "./geocode/service.ts";

export function load(ctx: ServerContext, resrc: AppResources) {
  terrain_service.register(ctx.router, resrc);
  poi_service.register(ctx.router, resrc);
  geocode_service.register(ctx.router, resrc);
}
