// @pxl/web/shared/ui/circular-button.tsx

import type { ReactNode } from "react";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Pressable, type StyleProp, StyleSheet, type ViewStyle } from "react-native";

import { Glass } from "@platform/glass.tsx";
import { Motion } from "@platform/motion.tsx";
import { SNAP, type Transition } from "@/shared/ui/motion/transition.ts";
import { alpha, chrome, color } from "@/shared/ui/theme.ts";

interface CircularButtonProps {
  children: ReactNode;
  onPress: () => void;
  /** What assistive technology reads it as. */
  label: string;
  isActive?: boolean;
  disabled?: boolean;
  /** How present it is, what it mounts at, and how a change is moved
   * through (shared/ui/motion/veil.ts). */
  fade?: f64;
  fadeFrom?: f64;
  fadeTransition?: Transition;
  /** A lift off the map, for a control that stands on its own. */
  shadow?: boolean;
  style?: StyleProp<ViewStyle>;
}

/** A round map control: a disc of the platform's glass, or cream where it
 * has none, holding one glyph. */
export function CircularButton({ children, onPress, label, isActive, disabled, fade = 1, fadeFrom, fadeTransition, shadow = false, style }: CircularButtonProps) {
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      accessibilityRole="button"
      accessibilityLabel={label}
      style={[styles.disc, shadow && !disabled && styles.lifted, isActive && styles.active, style]}
    >
      {({ pressed }) => (
        <Motion to={{ scale: pressed ? 0.95 : 1 }} transition={SNAP} style={styles.ground}>
          <Glass
            fill={isActive ? color.lit : color.cream}
            tint={isActive ? alpha(color.glow, 0.25) : undefined}
            interactive={!disabled}
            // Disabled, it is dimmed as a presence of its glass, not an
            // opacity over it (shared/ui/motion/veil.ts).
            fade={disabled ? fade * DISABLED : fade}
            fadeFrom={fadeFrom}
            transition={fadeTransition}
            style={styles.face}
          >
            {children}
          </Glass>
        </Motion>
      )}
    </Pressable>
  );
}

const DISABLED = 0.4;

const styles = StyleSheet.create({
  disc: {
    width: chrome.control,
    height: chrome.control,
    borderRadius: chrome.control / 2,
  },
  ground: { flex: 1 },
  face: { flex: 1, borderRadius: chrome.control / 2, alignItems: "center", justifyContent: "center" },
  lifted: { boxShadow: "0 6px 20px rgba(0, 0, 0, 0.25)" },
  active: { boxShadow: `0 0 0 2px ${alpha(color.glow, 0.6)}` },
});
