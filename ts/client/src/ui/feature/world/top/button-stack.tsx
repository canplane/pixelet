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
  CHROME,
  COLOR,
  DISABLED_CONTENT,
  SURFACE,
} from "@/ui/shared/component/theme.ts";

interface StackItem {
  label: string;
  onPress: () => void;
  glyph?: GlyphName;
  face?: ReactNode;
  isActive?: boolean;
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
      fill={isActive ? SURFACE.chosen.fill : COLOR.ground}
      tint={isActive ? SURFACE.chosen.glass : undefined}
      place={BackingPlace.OVER_MAP}
      interactive
      style={[STYLE.capsule, isActive && STYLE.chosen]}
    >
      {items.map((item) => (
        <Pressable
          key={item.label}
          // A disabled button still takes the touch, so that a tap on it does
          // not reach the world.
          onPress={item.disabled ? undefined : item.onPress}
          accessibilityRole="button"
          accessibilityLabel={item.label}
          accessibilityState={{ disabled: item.disabled }}
          style={STYLE.item}
        >
          {({ pressed }) => (
            <Motion
              to={{
                opacity: item.disabled
                  ? DISABLED_CONTENT
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
                    size={CHROME.glyph}
                    color={item.isActive ? COLOR.action : LABEL.primary}
                  />
                ))}
            </Motion>
          )}
        </Pressable>
      ))}
    </Backing>
  );
}

const STYLE = StyleSheet.create({
  capsule: {
    width: CHROME.control,
    borderRadius: CHROME.control / 2,
    overflow: "hidden",
  },
  item: {
    width: CHROME.control,
    height: CHROME.control,
    alignItems: "center",
    justifyContent: "center",
  },
  chosen: { boxShadow: `0 0 0 2px ${SURFACE.chosen.edge}` },
});
