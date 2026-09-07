// @pxl/api/modules/world/mod.ts

import type { ServerContext } from "@/main.ts";
import type { AppResources } from "@/app/resources.ts";

import * as terrain_service from "./terrain/service.ts";
import * as poi_service from "./poi/service.ts";

export function load(ctx: ServerContext, resrc: AppResources) {
  terrain_service.register(ctx.router, resrc);
  poi_service.register(ctx.router, resrc);
}
