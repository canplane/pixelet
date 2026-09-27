// @pxl/web/shared/ui/circular-button.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import { Backing, SYSTEM_BACKING } from "@platform/backing.tsx";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { chrome, color, surface } from "@/shared/ui/theme.ts";

interface CircularButtonProps {
  children: ReactNode;
  onPress: () => void;
  /** What assistive technology reads it as. */
  label: string;
  isActive?: boolean;
  disabled?: boolean;
}

/** A round map control: a disc of the platform's glass, or its fill where it
 * has none, holding one glyph. Switched on, the disc takes the chosen coral,
 * as a tint on the glass or as the fill. A press shrinks the disc where there
 * is no glass; the glass answers a press itself. */
export function CircularButton({ children, onPress, label, isActive, disabled }: CircularButtonProps) {
  return (
    <Pressable
      // Disabled, it still takes a touch and does nothing with it: a
      // Pressable told it is disabled lets the touch through to the map under
      // it, where a tap on the dimmed compass left the heading's follow.
      onPress={disabled ? undefined : onPress}
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled }}
      style={styles.disc}
    >
      {({ pressed }) => (
        <Motion to={{ scale: pressed && !disabled && !SYSTEM_BACKING ? 0.95 : 1 }} transition={SNAP} style={styles.press}>
          <Backing
            fill={isActive ? surface.chosen.fill : color.ground}
            tint={isActive ? surface.chosen.glass : undefined}
            place={BackingPlace.OverMap}
            interactive={!disabled}
            // Disabled, it is dimmed as a presence of its glass, not an
            // opacity over it (shared/ui/motion/veil.ts).
            fade={disabled ? DISABLED : 1}
            style={styles.face}
          >
            {children}
          </Backing>
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
  press: { flex: 1 },
  face: { flex: 1, borderRadius: chrome.control / 2, alignItems: "center", justifyContent: "center" },
});
