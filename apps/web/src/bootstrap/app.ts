// @pxl/web/bootstrap/app.ts

import { bootstrap_three } from "./three.ts";
// import { bootstrap_compute } from "./compute.ts";

export async function bootstrap_app(): Promise<void> {
  bootstrap_three();
  // await bootstrap_compute();
}
