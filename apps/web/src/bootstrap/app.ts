// @pxl/web/bootstrap/app.ts

import { bootstrap_three } from "./three.ts";
import { bootstrap_i18n } from "./i18n.ts";
// import { bootstrap_compute } from "./compute.ts";

export async function bootstrap_app(): Promise<void> {
  bootstrap_three();
  bootstrap_i18n();
  // await bootstrap_compute();
}
