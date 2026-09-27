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
  /** The glyph it holds, drawn in the label color at every control's glyph
   * size; or what it holds instead, a face of its own (the compass) or a
   * glyph that moves. */
  glyph?: GlyphName;
  children?: ReactNode;
  /** Its side: a map control's by default. */
  size?: f64;
  /** Where it stands, which is what its backing is: over the map by default,
   * or on a sheet, where the glass is clear and the fill faint. */
  place?: BackingPlace;
  onPress: () => void;
  /** What assistive technology reads it as. */
  label: string;
  isActive?: boolean;
  disabled?: boolean;
}

/** A round control: a disc of the platform's glass, or its fill where it has
 * none, holding one glyph. Switched on, the disc takes the chosen coral, as a
 * tint on the glass or as the fill, ringed in it. A press shrinks the disc where there is
 * no glass; the glass answers a press itself. */
export function CircularButton(
  { glyph, children, size = chrome.control, place = BackingPlace.OverMap, onPress, label, isActive, disabled }: CircularButtonProps,
) {
  const round = { width: size, height: size, borderRadius: size / 2 };
  return (
    <Pressable
      // Disabled, it still takes a touch and does nothing with it: a
      // Pressable told it is disabled lets the touch through to the map under
      // it, where a tap on the dimmed compass left the heading's follow.
      onPress={disabled ? undefined : onPress}
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled }}
      style={round}
    >
      {({ pressed }) => (
        // The ring stands on what a press shrinks, so the two shrink as one.
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
