// @pxl/web/bootstrap/worker.ts

import { bootstrap_render } from "./render.ts";
// import { bootstrap_compute } from "./compute.ts";

export function bootstrap_worker(): void {
  bootstrap_render();
}
