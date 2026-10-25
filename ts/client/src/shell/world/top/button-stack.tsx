// @pxl/client/shell/world/top/button-stack.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import { Backing, SYSTEM_BACKING } from "@/shell/ui/backing.tsx";
import type { GlyphName } from "@/shell/ui/icon.tsx";
import { LABEL } from "@/shell/ui/palette.ts";
import { BackingPlace } from "@/shell/ui/backing.tsx";
import { Glyph } from "@/shell/ui/glyph.tsx";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SNAP } from "@/shell/ui/motion/transition.ts";
import { chrome, color, disabledContent, surface } from "@/shell/ui/theme.ts";

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
