// @pxl/api/app/mount.ts

import type { ServerContext } from "@/main.ts";

import type { AppResources } from "./resources.ts";

// modules
import * as tiles_module from "@/modules/tiles/mod.ts";
import * as search_service from "@/modules/search/service.ts";

export function mount_app(ctx: ServerContext, resrc: AppResources): void {
  // dev-only: production bakes tiles offline, publishes to R2, and never
  // calls these routes — see modules/tiles/mod.ts's own top comment.
  if (__IS_DEV__) tiles_module.load(ctx, resrc);
  search_service.register(ctx.router, resrc);
}
