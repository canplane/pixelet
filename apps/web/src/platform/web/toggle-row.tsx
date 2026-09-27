// @pxl/web/platform/web/toggle-row.tsx

/**
 * A row that holds a setting's switch, on a page: the whole row is the
 * switch, and the switch in it only shows which way it is. A switch that
 * took the press as well would turn with the row and back — a click on it
 * two toggles — and a page's switch has no press of its own worth keeping.
 * A device's lets the switch alone take the touch, as its settings do.
 *
 * The switch is drawn as an iPhone's (iOS 26's UISwitch, which the device
 * shows beside this): React Native's own is react-native-web's on a page and
 * on Android, a thin track under a thumb wider than it, which is Material's
 * older switch and no platform's the app is held to. The track fills with
 * the chosen coral by an opacity and the thumb slides by a translation, the
 * two things an overlay moves (AGENTS.md).
 */

import type { ReactNode } from "react";
import { Pressable, StyleSheet, View } from "react-native";
import type { StyleProp, ViewStyle } from "react-native";

import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { color } from "@/shared/ui/theme.ts";

export interface ToggleRowProps {
  value: boolean;
  onValueChange: (value: boolean) => void;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

/** iOS 26's switch: a 63 × 28 track, and a capsule thumb 2 in from its ends. */
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
  on: { position: "absolute", top: 0, right: 0, bottom: 0, left: 0, borderRadius: TRACK.height / 2, backgroundColor: color.chosen },
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
