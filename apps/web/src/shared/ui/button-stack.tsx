// @pxl/web/shared/ui/button-stack.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import { Backing, SYSTEM_BACKING } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { chrome, color, disabledContent } from "@/shared/ui/theme.ts";

export interface StackItem {
  label: string;
  onPress: () => void;
  glyph?: GlyphName;
  /** Instead of `glyph`: a face of its own, drawn in the tint given. */
  face?: (tint: string) => ReactNode;
  /** Its mode is on: drawn in the action color. */
  isActive?: boolean;
  disabled?: boolean;
}

/** Buttons of one kind in one capsule over the map, as iPhone Maps groups its map controls. */
export function ButtonStack({ items }: { items: StackItem[] }) {
  return (
    <Backing fill={color.ground} place={BackingPlace.OverMap} interactive style={styles.capsule}>
      {items.map((item) => {
        const tint = item.isActive ? color.action : LABEL.primary;
        return (
          <Pressable
            key={item.label}
            // Disabled still takes the touch, so a tap does not fall through to the map.
            onPress={item.disabled ? undefined : item.onPress}
            accessibilityRole="button"
            accessibilityLabel={item.label}
            accessibilityState={{ disabled: item.disabled }}
            style={styles.item}
          >
            {({ pressed }) => (
              <Motion
                to={{ opacity: item.disabled ? disabledContent : pressed && !SYSTEM_BACKING ? 0.5 : 1 }}
                transition={SNAP}
              >
                {item.face ? item.face(tint) : item.glyph && <Glyph glyph={item.glyph} size={chrome.glyph} color={tint} />}
              </Motion>
            )}
          </Pressable>
        );
      })}
    </Backing>
  );
}

const styles = StyleSheet.create({
  capsule: { width: chrome.control, borderRadius: chrome.control / 2, overflow: "hidden" },
  item: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
});
