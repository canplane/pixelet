// @pxl/client/service/platform/visibility.native.ts

import { AppState } from "react-native";

/** Whether the app is shown now. */
export function shown(): boolean {
  return AppState.currentState === "active";
}

/** In the background, the app may be suspended or ended without further
 * notice. The answer stops the watch. */
export function watch(report: (visible: boolean) => void): () => void {
  const sub = AppState.addEventListener("change", () => report(shown()));
  return () => sub.remove();
}
