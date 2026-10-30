// @pxl/native/visibility.ts

import { AppState } from "react-native";

/** The app in front or not, as React Native's AppState says: in the
 * background it may be suspended or ended without another word. */
export function watch(report: (visible: boolean) => void): void {
  AppState.addEventListener("change", (state) => report(state === "active"));
}
