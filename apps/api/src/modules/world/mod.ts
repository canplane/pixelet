// @pxl/api/modules/world/mod.ts

import type { ServerContext } from "@/main.ts";
import type { AppResources } from "@/app/resources.ts";

import * as terrain_service from "./terrain/service.ts";

export function load(ctx: ServerContext, resrc: AppResources) {
  terrain_service.register(resrc);
}
