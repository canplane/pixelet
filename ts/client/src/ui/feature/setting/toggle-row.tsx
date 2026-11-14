// @pxl/client/ui/feature/setting/toggle-row.tsx

/**
 * A settings row on a page and on Android. The whole row is the switch,
 * because a switch inside a row that also takes the press would toggle twice.
 * It is drawn as iOS 26's switch, since React Native for Web draws Material's
 * older one.
 */

import type { ReactNode } from "react";
import {
  Pressable,
  type StyleProp,
  StyleSheet,
  View,
  type ViewStyle,
} from "react-native";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import { COLOR } from "@/ui/shared/component/theme.ts";

export interface ToggleRowProps {
  value: boolean;
  onValueChange: (value: boolean) => void;
  style?: StyleProp<ViewStyle>;
  children: ReactNode;
}

/** A 63 × 28 track, with a capsule thumb inset 2 from its ends, as iOS 26
 * draws it. */
const TRACK = { width: 63, height: 28 };
const INSET = 2;
const THUMB = { width: 37, height: TRACK.height - 2 * INSET };
const TRAVEL = TRACK.width - THUMB.width - 2 * INSET;

export function ToggleRow({
  value,
  onValueChange,
  style,
  children,
}: ToggleRowProps) {
  return (
    <Pressable
      onPress={() => onValueChange(!value)}
      accessibilityRole="switch"
      aria-checked={value}
      style={style}
    >
      {children}
      <View
        accessibilityElementsHidden
        importantForAccessibility="no-hide-descendants"
        style={STYLE.track}
      >
        <Motion
          to={{ opacity: value ? 1 : 0 }}
          transition={SNAP}
          style={STYLE.on}
        />
        <Motion
          to={{ translateX: value ? TRAVEL : 0 }}
          transition={SNAP}
          style={STYLE.thumb}
        />
      </View>
    </Pressable>
  );
}

const STYLE = StyleSheet.create({
  track: {
    ...TRACK,
    borderRadius: TRACK.height / 2,
    backgroundColor: COLOR.track,
    pointerEvents: "none",
  },
  on: {
    ...StyleSheet.absoluteFill,
    borderRadius: TRACK.height / 2,
    backgroundColor: COLOR.chosen,
  },
  thumb: {
    position: "absolute",
    top: INSET,
    left: INSET,
    ...THUMB,
    borderRadius: THUMB.height / 2,
    backgroundColor: COLOR.white,
    boxShadow: "0 2px 6px rgba(0, 0, 0, 0.15), 0 0 1px rgba(0, 0, 0, 0.1)",
  },
});
