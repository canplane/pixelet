// @pxl/client/ui/debug/accordion.tsx

import { CaretDownIcon, CaretUpIcon } from "phosphor-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { create } from "zustand";

import { cornerPanel } from "@/ui/component/corner-card.ts";
import { alpha, color } from "@/ui/component/theme.ts";
import { gauge } from "./perf/gauge.tsx";
import type { PanelName } from "./window.tsx";

/** Opening a panel closes the one that was open, so the corner holds one open
 * panel at a time. */
const store = create<{ open: PanelName | null }>()(() => ({
  open: null,
}));

const toggle = (name: PanelName) =>
  store.setState(({ open }) => ({
    open: open === name ? null : name,
  }));

export const HINT_INK = alpha(color.white, 0.55);

/** `note` follows the title in regular weight, since it is a reading and not a
 * name. */
export function Accordion({
  name,
  title,
  note,
  end,
  children,
}: {
  name: PanelName;
  title: string;
  note?: string;
  end?: ReactNode;
  children?: ReactNode;
}) {
  const open = store((s) => s.open === name);
  return (
    <View style={styles.panel}>
      <Pressable
        onPress={() => toggle(name)}
        accessibilityRole="button"
        aria-expanded={open}
      >
        <View style={styles.title}>
          <Text style={[gauge.text, styles.titleText]}>
            {title}
            {note !== undefined && (
              <Text style={styles.noteText}>{` · ${note}`}</Text>
            )}
          </Text>
          <View style={styles.end}>
            {open && end}
            {open ? (
              <CaretUpIcon size={12} color={HINT_INK} />
            ) : (
              <CaretDownIcon size={12} color={HINT_INK} />
            )}
          </View>
        </View>
      </Pressable>
      {open && children}
    </View>
  );
}

const styles = StyleSheet.create({
  panel: {
    ...cornerPanel,
    flexShrink: 1,
    overflow: "hidden",
    pointerEvents: "box-none",
  },
  title: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  titleText: {
    fontSize: 11,
    fontWeight: "600",
    letterSpacing: 0.6,
    textTransform: "uppercase",
    color: alpha(color.white, 0.75),
  },
  noteText: { fontWeight: "400" },
  end: { flexDirection: "row", alignItems: "center", gap: 6 },
});
