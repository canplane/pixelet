// @pxl/web/bootstrap/worker.ts

import { bootstrap_three } from "./three.ts";
// import { bootstrap_compute } from "./compute.ts";

export function bootstrap_worker(): void {
  bootstrap_three();
  // await bootstrap_compute();
}
