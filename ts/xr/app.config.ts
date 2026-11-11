// @pxl/xr/app.config.ts

/** The phone app's table, its paths named from here, with Horizon OS's
 * plugins in place of expo-location's. Nothing builds it yet (README.md). */

import { app } from "../client/app.config.ts";

type Plugin = string | [string, unknown];

const { expo } = app("../client");
const phone = expo.plugins as Plugin[];
const name = (p: Plugin) => (Array.isArray(p) ? p[0] : p);
const [, permission] = phone.find((p) => name(p) === "expo-location") as [
  string,
  unknown,
];

export default {
  expo: {
    ...expo,
    plugins: [
      ["expo-horizon-core", { supportedDevices: "quest3|quest3s" }],
      ["expo-horizon-location", permission],
      ...phone.filter((p) => name(p) !== "expo-location"),
      "./plugin/quest-build.ts",
    ],
  },
};
