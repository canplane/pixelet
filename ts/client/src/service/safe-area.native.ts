// @pxl/client/service/safe-area.native.ts

import { Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import type { SafeArea } from "./types.ts";
import { PLATFORM } from "./pxl-platform.ts";

export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return {
    top,
    bottom,
    tappable: Platform.OS === "android" ? PLATFORM.tappableBottom() : 0,
  };
}
