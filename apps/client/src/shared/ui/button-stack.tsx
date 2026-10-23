// @pxl/client/shared/ui/button-stack.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import { Backing, SYSTEM_BACKING } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { BackingPlace } from "@/shared/ui/backing.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { chrome, color, disabledContent, surface } from "@/shared/ui/theme.ts";

interface StackItem {
  label: string;
  onPress: () => void;
  glyph?: GlyphName;
  face?: ReactNode;
  isActive?: boolean;
  disabled?: boolean;
}

/** iPhone Maps' grouped controls. */
export function ButtonStack({ items, isActive }: { items: StackItem[]; isActive?: boolean }) {
  return (
    <Backing
      fill={isActive ? surface.chosen.fill : color.ground}
      tint={isActive ? surface.chosen.glass : undefined}
      place={BackingPlace.OverMap}
      interactive
      style={[styles.capsule, isActive && styles.chosen]}
    >
      {items.map((item) => (
        <Pressable
          key={item.label}
          // Disabled still takes the touch: no tap through to the map.
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
              {item.face ?? (item.glyph && <Glyph glyph={item.glyph} size={chrome.glyph} color={item.isActive ? color.action : LABEL.primary} />)}
            </Motion>
          )}
        </Pressable>
      ))}
    </Backing>
  );
}

const styles = StyleSheet.create({
  capsule: { width: chrome.control, borderRadius: chrome.control / 2, overflow: "hidden" },
  item: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
  chosen: { boxShadow: `0 0 0 2px ${surface.chosen.edge}` },
});
