// @pxl/client/service/platform/safe-area.native.ts

import { useSafeAreaInsets } from "react-native-safe-area-context";

import type { SafeArea } from "./types.ts";
import { PLATFORM } from "./pxl-platform.ts";

export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return {
    top,
    bottom,
    tappable: PLATFORM.tappable_bottom(),
  };
}
