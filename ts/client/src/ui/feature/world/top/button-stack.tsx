// @pxl/client/ui/feature/world/top/button-stack.tsx

import type { ReactNode } from "react";

import { Pressable, StyleSheet } from "react-native";

import { Backing, SYSTEM_BACKING } from "@/ui/shared/component/backing.tsx";
import type { GlyphName } from "@/ui/shared/component/icon.tsx";
import { LABEL } from "@/ui/shared/component/palette.ts";
import { BackingPlace } from "@/ui/shared/component/backing.tsx";
import { Glyph } from "@/ui/shared/component/glyph.tsx";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SNAP } from "@/ui/shared/component/motion/transition.ts";
import {
  chrome,
  color,
  disabled_content,
  surface,
} from "@/ui/shared/component/theme.ts";

interface StackItem {
  label: string;
  on_press: () => void;
  glyph?: GlyphName;
  face?: ReactNode;
  is_active?: boolean;
  disabled?: boolean;
}

/** Round buttons grouped in one capsule, as the iPhone's Maps app groups its
 * controls. */
export function ButtonStack({
  items,
  isActive,
}: {
  items: StackItem[];
  isActive?: boolean;
}) {
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
          // A disabled button still takes the touch, so that a tap on it does
          // not reach the world.
          onPress={item.disabled ? undefined : item.on_press}
          accessibilityRole="button"
          accessibilityLabel={item.label}
          accessibilityState={{ disabled: item.disabled }}
          style={styles.item}
        >
          {({ pressed }) => (
            <Motion
              to={{
                opacity: item.disabled
                  ? disabled_content
                  : pressed && !SYSTEM_BACKING
                    ? 0.5
                    : 1,
              }}
              transition={SNAP}
            >
              {item.face ??
                (item.glyph && (
                  <Glyph
                    glyph={item.glyph}
                    size={chrome.glyph}
                    color={item.is_active ? color.action : LABEL.primary}
                  />
                ))}
            </Motion>
          )}
        </Pressable>
      ))}
    </Backing>
  );
}

const styles = StyleSheet.create({
  capsule: {
    width: chrome.control,
    borderRadius: chrome.control / 2,
    overflow: "hidden",
  },
  item: {
    width: chrome.control,
    height: chrome.control,
    alignItems: "center",
    justifyContent: "center",
  },
  chosen: { boxShadow: `0 0 0 2px ${surface.chosen.edge}` },
});
