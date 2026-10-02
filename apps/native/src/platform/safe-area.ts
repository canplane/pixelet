// @pxl/native/platform/safe-area.ts

import { Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { PLATFORM } from "./module.ts";

export interface SafeArea {
  top: number;
  bottom: number;
  /** Android's navigation buttons take a touch; gesture handles and the home indicator do not. */
  tappable: number;
}

export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return { top, bottom, tappable: Platform.OS === "android" ? PLATFORM.tappableBottom() : 0 };
}
