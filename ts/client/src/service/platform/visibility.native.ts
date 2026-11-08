// @pxl/client/service/platform/visibility.native.ts

import { AppState } from "react-native";

/** Whether the app is in front, as React Native's AppState says. In the
 * background, the app may be suspended or ended without further notice. */
export function watch(report: (visible: boolean) => void): void {
  AppState.addEventListener("change", (state) => report(state === "active"));
}
