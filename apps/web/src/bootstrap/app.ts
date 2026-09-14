// @pxl/web/bootstrap/app.ts

import { bootstrap_i18n } from "./i18n.ts";
// import { bootstrap_compute } from "./compute.ts";

// deno-lint-ignore require-await -- bootstrap_compute's own await is commented out below; stays async so uncommenting it isn't a signature change
export async function bootstrap_app(): Promise<void> {
  bootstrap_i18n();
  // await bootstrap_compute();
}
