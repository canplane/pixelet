// @pxl/web/app/bootstrap.ts

import { boot_kernel } from "@/kernel/kernel.ts";
import { bootstrapI18n } from "@/locale/i18n.ts";

export function bootstrapApp(): void {
  bootstrapI18n();
  boot_kernel();
}
