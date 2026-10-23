// @pxl/native/safe-area.ts

import { Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import type { SafeArea } from "@pxl/client/shell/ui/bottom-rest.ts";
import { PLATFORM } from "./module.ts";

export function useSafeArea(): SafeArea {
  const { top, bottom } = useSafeAreaInsets();
  return { top, bottom, tappable: Platform.OS === "android" ? PLATFORM.tappableBottom() : 0 };
}
