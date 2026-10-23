// @pxl/client/feature/debug/fold.tsx

/**
 * The debug panels' folds: each closed until opened, and opening one closes
 * the one that was open, so the corner holds one at a time.
 */

import { CaretDown, CaretUp } from "phosphor-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { create } from "zustand";

import { cornerPanel } from "@/shared/ui/corner-card.ts";
import { alpha, color } from "@/shared/ui/theme.ts";
import { gauge } from "./profiler/gauge.tsx";

export type FoldName = "view" | "performance" | "world_gizmo";

export const useOpenFold = create<{ open: FoldName | null }>(() => ({ open: null }));

const toggle = (name: FoldName) => useOpenFold.setState(({ open }) => ({ open: open === name ? null : name }));

export const HINT_INK = alpha(color.white, 0.55);

/** A panel whose title opens and closes it; `end` sits by the caret. */
export function Fold({ name, title, end, children }: { name: FoldName; title: string; end?: ReactNode; children?: ReactNode }) {
  const open = useOpenFold((s) => s.open === name);
  return (
    <View style={styles.panel}>
      <Pressable onPress={() => toggle(name)} accessibilityRole="button" aria-expanded={open}>
        <View style={styles.title}>
          <Text style={[gauge.text, styles.titleText]}>{title}</Text>
          <View style={styles.end}>
            {open && end}
            {open ? <CaretUp size={12} color={HINT_INK} /> : <CaretDown size={12} color={HINT_INK} />}
          </View>
        </View>
      </Pressable>
      {open && children}
    </View>
  );
}

const styles = StyleSheet.create({
  panel: { ...cornerPanel, flexShrink: 1, overflow: "hidden", pointerEvents: "box-none" },
  title: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  titleText: { fontSize: 11, fontWeight: "600", letterSpacing: 0.6, textTransform: "uppercase", color: alpha(color.white, 0.75) },
  end: { flexDirection: "row", alignItems: "center", gap: 6 },
});
