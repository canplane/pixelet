// @pxl/client/ui/feature/setting/toggle-row.ios.tsx

/**
 * A settings row on an iPhone, where the switch alone takes the touch. It is
 * UIKit's switch rather than SwiftUI's Toggle, which inside a sheet misses
 * about two quick taps in five.
 */

import { Switch, View } from "react-native";

import { color } from "@/ui/shared/component/theme.ts";
import type { ToggleRowProps } from "./toggle-row.tsx";

// The default file's other exports, beside the ToggleRow this file draws
// itself.
export * from "./toggle-row.tsx";

export function ToggleRow({
  value,
  onValueChange,
  style,
  children,
}: ToggleRowProps) {
  return (
    <View style={style}>
      {children}
      <Switch
        value={value}
        onValueChange={onValueChange}
        trackColor={{ true: color.chosen }}
      />
    </View>
  );
}
