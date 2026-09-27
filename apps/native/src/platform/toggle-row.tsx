// @pxl/native/platform/toggle-row.tsx

/**
 * A row that holds a setting's switch, on an iPhone: the system's own switch
 * (SwiftUI's Toggle), which alone takes the touch, as a device's settings do
 * — the row's words are not a second way to flip it.
 * Taking the touch itself is what gives the switch its press: iOS 26 swells
 * its thumb into glass under a finger, and a switch only shown never is.
 */

import type { ReactNode } from "react";
import { View } from "react-native";
import type { StyleProp, ViewStyle } from "react-native";
import { Host, Switch } from "@expo/ui";
import { tint } from "@expo/ui/swift-ui/modifiers";
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
      <Host matchContents>
        <Switch value={value} onValueChange={onValueChange} modifiers={CHOSEN} />
      </Host>
    </View>
  );
}

// On, the switch is the panel's chosen color, as a radio and a chip are. The
// rest is the system's: SwiftUI colors a Toggle's on track and nothing else,
// and its glass is drawn for the white thumb it keeps.
const CHOSEN = [tint(color.chosen)];
