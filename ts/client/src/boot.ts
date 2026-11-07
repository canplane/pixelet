// @pxl/client/boot.ts

/** The client booted as this file loads: index.ts imports it ahead of Expo on
 * a page and of the shell everywhere, so the worker and the kernel's module
 * load while those evaluate. */

import { boot } from "@/runtime/boot.ts";

boot();
