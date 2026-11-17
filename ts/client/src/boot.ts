// @pxl/client/boot.ts

/** Boots the client as soon as this file loads. index.ts imports it before
 * Expo on the web, and before the UI everywhere, so that the worker and the
 * core's module load while those evaluate. */

import { boot } from "@/runtime/boot.ts";

boot();
