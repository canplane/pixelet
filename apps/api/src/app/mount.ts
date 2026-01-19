// @pxl/api/app/mount.ts

import type { ServerContext } from "@/main.ts";

import type { AppResources } from "./resources.ts";

// modules
import * as world_module from "@/modules/world/mod.ts";

export function mount_app(ctx: ServerContext, resrc: AppResources): void {
  world_module.load(ctx, resrc);
}
