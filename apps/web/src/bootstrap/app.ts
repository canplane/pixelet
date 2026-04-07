// @pxl/web/bootstrap/app.ts

import { bootstrap_render } from "./render.ts";
// import { bootstrap_compute } from "./compute.ts";

export async function bootstrap_app(): Promise<void> {
  await bootstrap_render();
  // await bootstrap_compute();
}
