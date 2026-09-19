// @pxl/web/app/bootstrap.ts

import { boot_kernel } from "@/kernel/kernel.ts";
import { bootstrap_i18n } from "@/locale/i18n.ts";

export function bootstrap_app(): void {
  bootstrap_i18n();
  boot_kernel();
}
