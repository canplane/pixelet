// @pxl/client/shell/setting/toggle-row.tsx

/**
 * A settings row on a page and on Android: the whole row is the switch, since a switch also
 * taking the press toggled twice. Drawn as iOS 26's switch; RN-web's is
 * Material's older one.
 */

import type { ReactNode } from "react";
import { Pressable, type StyleProp, StyleSheet, View, type ViewStyle } from "react-native";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { color } from "@/shell/ui/theme.ts";

export interface ToggleRowProps {
  value: boolean;
  onValueChange: (value: boolean) => void;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

/** 63 × 28 track, capsule thumb 2 in from its ends. */
const TRACK = { width: 63, height: 28 };
const INSET = 2;
const THUMB = { width: 37, height: TRACK.height - 2 * INSET };
const TRAVEL = TRACK.width - THUMB.width - 2 * INSET;

export function ToggleRow({ value, onValueChange, style, children }: ToggleRowProps) {
  return (
    <Pressable onPress={() => onValueChange(!value)} accessibilityRole="switch" aria-checked={value} style={style}>
      {children}
      <View accessibilityElementsHidden importantForAccessibility="no-hide-descendants" style={styles.track}>
        <Motion to={{ opacity: value ? 1 : 0 }} transition={SNAP} style={styles.on} />
        <Motion to={{ translateX: value ? TRAVEL : 0 }} transition={SNAP} style={styles.thumb} />
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  track: {
    ...TRACK,
    borderRadius: TRACK.height / 2,
    backgroundColor: color.track,
    pointerEvents: "none",
  },
  on: { ...StyleSheet.absoluteFill, borderRadius: TRACK.height / 2, backgroundColor: color.chosen },
  thumb: {
    position: "absolute",
    top: INSET,
    left: INSET,
    ...THUMB,
    borderRadius: THUMB.height / 2,
    backgroundColor: color.white,
    boxShadow: "0 2px 6px rgba(0, 0, 0, 0.15), 0 0 1px rgba(0, 0, 0, 0.1)",
  },
});
