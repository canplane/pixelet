// @pxl/web/feature/debug/fold.tsx

/**
 * The debug panels' folds: each closed until opened, and opening one closes
 * the one that was open, so the corner holds one at a time.
 */

import { CaretDown, CaretUp } from "phosphor-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, View } from "react-native";
import { create } from "zustand";

import { alpha, color } from "@/shared/ui/theme.ts";
import { Title } from "./profiler/gauge.tsx";

export type FoldName = "gizmo" | "view" | "performance";

export const useOpenFold = create<{ open: FoldName | null }>(() => ({ open: null }));

const toggle = (name: FoldName) => useOpenFold.setState(({ open }) => ({ open: open === name ? null : name }));

export const HINT_INK = alpha(color.white, 0.55);

/** A panel whose title opens and closes it; `end` sits by the caret. */
export function Fold({ name, title, end, children }: { name: FoldName; title: string; end?: ReactNode; children: ReactNode }) {
  const open = useOpenFold((s) => s.open === name);
  return (
    <View style={styles.panel}>
      <Pressable onPress={() => toggle(name)} accessibilityRole="button" aria-expanded={open}>
        <Title
          first
          end={
            <View style={styles.end}>
              {open && end}
              {open ? <CaretUp size={12} color={HINT_INK} /> : <CaretDown size={12} color={HINT_INK} />}
            </View>
          }
        >
          {title}
        </Title>
      </Pressable>
      {open && children}
    </View>
  );
}

const styles = StyleSheet.create({
  panel: {
    flexShrink: 1,
    width: 300,
    overflow: "hidden",
    marginBottom: 8,
    borderRadius: 8,
    padding: 10,
    backgroundColor: alpha(color.black, 0.72),
    pointerEvents: "box-none",
  },
  end: { flexDirection: "row", alignItems: "center", gap: 6 },
});
