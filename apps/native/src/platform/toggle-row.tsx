// @pxl/native/platform/toggle-row.tsx

/**
 * A settings row on an iPhone: the switch alone takes the touch. UIKit's
 * switch, not SwiftUI's Toggle, which inside a sheet missed about two quick
 * taps in five.
 */

import { Switch, View } from "react-native";

import { color } from "@/shared/ui/theme.ts";
import type { ToggleRowProps } from "@/shared/ui/toggle-row.ts";

export function ToggleRow({ value, onValueChange, style, children }: ToggleRowProps) {
  return (
    <View style={style}>
      {children}
      <Switch value={value} onValueChange={onValueChange} trackColor={{ true: color.chosen }} />
    </View>
  );
}
