// @pxl/native/platform/toggle-row.tsx

/**
 * A row that holds a setting's switch, on an iPhone: the system's own switch,
 * which alone takes the touch, as a device's settings do — the row's words
 * are not a second way to flip it. Taking the touch itself is what gives the
 * switch its press: iOS 26 swells its thumb into glass under a finger, and a
 * switch only shown never is.
 *
 * It is UIKit's switch (React Native's Switch), not SwiftUI's Toggle. Inside
 * a sheet the Toggle sat in SwiftUI under React Native's scroll view under
 * SwiftUI again, and missed a quick tap about two times in five on a phone;
 * UIKit's is a control React Native's scroll view does not take a touch from.
 */

import type { ReactNode } from "react";
import { Switch, View } from "react-native";
import type { StyleProp, ViewStyle } from "react-native";
import { color } from "@/shared/ui/theme.ts";

export interface ToggleRowProps {
  value: boolean;
  onValueChange: (value: boolean) => void;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

export function ToggleRow({ value, onValueChange, style, children }: ToggleRowProps) {
  return (
    <View style={style}>
      {children}
      {
        // On, the switch is the panel's chosen color, as a radio and a chip
        // are; off, the system's.
      }
      <Switch value={value} onValueChange={onValueChange} trackColor={{ true: color.chosen }} />
    </View>
  );
}
