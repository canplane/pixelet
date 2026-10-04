// @pxl/web/shared/ui/circular-button.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import type { f64 } from "@xpute/core/abi/word.ts";
import { Backing, SYSTEM_BACKING } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { chrome, color, disabledContent, surface } from "@/shared/ui/theme.ts";

interface CircularButtonProps {
  /** Or `children` instead: a face of its own (the compass), or a glyph that moves. */
  glyph?: GlyphName;
  children?: ReactNode;
  size?: f64;
  place?: BackingPlace;
  onPress: () => void;
  label: string;
  isActive?: boolean;
  disabled?: boolean;
}

export function CircularButton(
  { glyph, children, size = chrome.control, place = BackingPlace.OverMap, onPress, label, isActive, disabled }: CircularButtonProps,
) {
  const round = { width: size, height: size, borderRadius: size / 2 };
  return (
    <Pressable
      // Disabled still takes the touch: a disabled Pressable let a tap on the
      // dimmed compass through to the map.
      onPress={disabled ? undefined : onPress}
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled }}
      style={round}
    >
      {({ pressed }) => (
        <Motion
          to={{ scale: pressed && !disabled && !SYSTEM_BACKING ? 0.95 : 1 }}
          transition={SNAP}
          style={[styles.press, { borderRadius: size / 2 }, isActive && styles.chosen]}
        >
          <Backing
            fill={isActive ? surface.chosen.fill : place === BackingPlace.OnSheet ? color.faint : color.ground}
            tint={isActive ? surface.chosen.glass : undefined}
            place={place}
            interactive={!disabled}
            style={[styles.face, { borderRadius: size / 2 }]}
          >
            <Motion to={{ opacity: disabled ? disabledContent : 1 }} transition={SNAP} style={styles.content}>
              {glyph ? <Glyph glyph={glyph} size={chrome.glyph} color={LABEL.primary} /> : children}
            </Motion>
          </Backing>
        </Motion>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  press: { flex: 1 },
  chosen: { boxShadow: `0 0 0 2px ${surface.chosen.edge}` },
  face: { flex: 1, alignItems: "center", justifyContent: "center" },
  content: { alignItems: "center", justifyContent: "center" },
});
