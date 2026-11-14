// @pxl/xr/app.config.ts

/** The phone app's table, its paths named from here, with Horizon OS's
 * plugins in place of expo-location's. Nothing builds it yet (README.md). */

import { app } from "../client/app.config.ts";

type Plugin = string | [string, unknown];

const EXPO = app("../client").expo;
const PHONE_PLUGINS = EXPO.plugins as Plugin[];
const name = (p: Plugin) => (Array.isArray(p) ? p[0] : p);
const [, PERMISSION] = PHONE_PLUGINS.find(
  (p) => name(p) === "expo-location",
) as [string, unknown];

export default {
  expo: {
    ...EXPO,
    plugins: [
      ["expo-horizon-core", { supportedDevices: "quest3|quest3s" }],
      ["expo-horizon-location", PERMISSION],
      ...PHONE_PLUGINS.filter((p) => name(p) !== "expo-location"),
      "./plugin/quest-build.ts",
    ],
  },
};
