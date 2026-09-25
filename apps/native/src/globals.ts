// @pxl/native/globals.ts

import { Platform } from "react-native";

/** The constants Vite defines for the web bundle (vite.config.ts `define`),
 * set here as globals, since Metro defines none of them. */
const g = globalThis as Record<string, unknown>;
g.__IS_DEV__ = __DEV__;
g.__DEV_BUNDLE__ = __DEV__;
g.__APP_ENV__ = __DEV__ ? "development" : "production";
g.__PUBLISHED_TILES__ = true;
g.__APP_NAME__ = "pixelet";
g.__ENV__ = Platform.OS === "web" ? "browser" : "native";
g.__ROLE__ = "client";
