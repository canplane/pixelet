// @pxl/web/shared/ui/button-stack.tsx

import { Pressable, StyleSheet, View } from "react-native";

import { Backing, SYSTEM_BACKING } from "@platform/backing.tsx";
import type { GlyphName } from "@platform/icon.tsx";
import { LABEL } from "@platform/palette.ts";
import { BackingPlace } from "@/shared/ui/backing-place.ts";
import { Glyph } from "@/shared/ui/glyph.tsx";
import { Motion } from "@platform/motion.tsx";
import { SNAP } from "@/shared/ui/motion/transition.ts";
import { chrome, color } from "@/shared/ui/theme.ts";

export interface StackItem {
  glyph: GlyphName;
  label: string;
  onPress: () => void;
  /** An on/off item that is on: its glyph takes the action color. */
  isActive?: boolean;
}

/** Buttons of one kind in one capsule over the map, split by a hairline, as
 * iPhone Maps groups its map controls. */
export function ButtonStack({ items }: { items: StackItem[] }) {
  return (
    <Backing fill={color.ground} place={BackingPlace.OverMap} interactive style={styles.capsule}>
      {items.map((item, i) => (
        <View key={item.label} style={i > 0 && styles.split}>
          <Pressable onPress={item.onPress} accessibilityRole="button" accessibilityLabel={item.label} style={styles.item}>
            {({ pressed }) => (
              <Motion to={{ opacity: pressed && !SYSTEM_BACKING ? 0.5 : 1 }} transition={SNAP}>
                <Glyph glyph={item.glyph} size={chrome.glyph} color={item.isActive ? color.action : LABEL.primary} />
              </Motion>
            )}
          </Pressable>
        </View>
      ))}
    </Backing>
  );
}

const styles = StyleSheet.create({
  capsule: { width: chrome.control, borderRadius: chrome.control / 2, overflow: "hidden" },
  item: { width: chrome.control, height: chrome.control, alignItems: "center", justifyContent: "center" },
  split: { borderTopWidth: StyleSheet.hairlineWidth, borderTopColor: LABEL.tertiary },
});
