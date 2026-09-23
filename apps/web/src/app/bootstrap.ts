// @pxl/web/app/bootstrap.ts

import { declareUnsupported } from "@/app/store/unsupported-store.ts";
import { boot_kernel } from "@/kernel/kernel.ts";
import { missing_before_boot } from "@/kernel/render/gpu.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

export function bootstrapApp(): void {
  bootstrapI18n();
  // The locale first, because what this may have to say is said in it. A
  // browser that cannot draw is told so instead of being started: the kernel
  // would otherwise fetch its module, open a world and raise toasts about
  // loading it, under a notice saying none of it can be shown.
  const missing = missing_before_boot();
  if (missing) {
    declareUnsupported(missing);
    return;
  }
  boot_kernel();
}
