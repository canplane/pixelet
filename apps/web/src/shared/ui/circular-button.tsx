// @pxl/web/shared/ui/circular-button.tsx

import type { ReactNode } from "react";
import { Pressable, type StyleProp, StyleSheet, type ViewStyle } from "react-native";

import { alpha, chrome, color } from "@/shared/ui/theme.ts";

interface CircularButtonProps {
  children: ReactNode;
  onPress: () => void;
  /** What assistive technology reads it as. */
  label: string;
  isActive?: boolean;
  disabled?: boolean;
  /** A lift off the map, for a control that stands on its own. */
  shadow?: boolean;
  style?: StyleProp<ViewStyle>;
}

/** A round map control: a cream disc holding one glyph. */
export function CircularButton({ children, onPress, label, isActive, disabled, shadow = false, style }: CircularButtonProps) {
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      accessibilityRole="button"
      accessibilityLabel={label}
      style={({ pressed }) => [
        styles.disc,
        shadow && !disabled && styles.lifted,
        isActive && styles.active,
        disabled && styles.disabled,
        pressed && styles.pressed,
        style,
      ]}
    >
      {children}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  disc: {
    width: chrome.control,
    height: chrome.control,
    borderRadius: chrome.control / 2,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: color.cream,
  },
  lifted: { boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)" },
  active: { backgroundColor: "#fffbeb", boxShadow: `0 0 0 2px ${alpha(color.amber400, 0.6)}` },
  disabled: { opacity: 0.4 },
  pressed: { transform: [{ scale: 0.95 }] },
});
