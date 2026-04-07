// @pxl/web/bootstrap/render.ts

import { init_render_runtime } from "@/render/runtime.ts";

let booted = false;

export async function bootstrap_render(): Promise<void> {
  if (booted) return;

  await init_render_runtime();
  booted = true;
}
