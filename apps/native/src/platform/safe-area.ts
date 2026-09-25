// @pxl/native/platform/safe-area.ts

import { useSafeAreaInsets } from "react-native-safe-area-context";

export interface SafeArea {
  top: number;
  bottom: number;
}

/** What the notch and the home indicator take off the screen's edges. */
export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return { top, bottom };
}
