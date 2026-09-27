// @pxl/native/platform/safe-area.ts

import { Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { PLATFORM } from "./module.ts";

export interface SafeArea {
  top: number;
  bottom: number;
  /** The part of `bottom` that takes a touch — a system's buttons, which the
   * chrome stands clear of (shared/ui/bottom-rest.ts). Android's navigation
   * buttons do, its gesture handle does not; an iPhone's home indicator does
   * not either. */
  tappable: number;
}

/** What the notch, the home indicator and the navigation bar take off the
 * screen's edges. Read again whenever those change, which is when the
 * insets do. */
export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return { top, bottom, tappable: Platform.OS === "android" ? PLATFORM.tappableBottom() : 0 };
}
