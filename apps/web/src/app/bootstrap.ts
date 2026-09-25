// @pxl/web/app/bootstrap.ts

import { host_boot } from "@platform/host.ts";
import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

export function bootstrapApp(): void {
  // The locale first, because what the host may have to say about the
  // device is said in it.
  bootstrapI18n();
  const missing = host_boot();
  if (missing) declareUnsupported(missing);
}
