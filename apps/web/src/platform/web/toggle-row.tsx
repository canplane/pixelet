// @pxl/web/platform/web/toggle-row.tsx

/**
 * A row that holds a setting's switch, on a page: the whole row is the
 * switch, and the switch in it only shows which way it is. A switch that
 * took the press as well would turn with the row and back — a click on it
 * two toggles — and a page's switch has no press of its own worth keeping.
 * A device's lets the switch alone take the touch, as its settings do.
 */

import type { ReactNode } from "react";
import { Pressable, StyleSheet, Switch, View } from "react-native";
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
    <Pressable onPress={() => onValueChange(!value)} accessibilityRole="switch" aria-checked={value} style={style}>
      {children}
      <View accessibilityElementsHidden importantForAccessibility="no-hide-descendants" style={styles.shown}>
        <Switch
          value={value}
          trackColor={{ false: color.track, true: color.chosen }}
          thumbColor={color.white}
          {
            // react-native-web colors an on thumb by this alone, and teal where
            // it is not given; React Native's types do not name it.
            ...{ activeThumbColor: color.white }
          }
        />
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  shown: { pointerEvents: "none" },
});
