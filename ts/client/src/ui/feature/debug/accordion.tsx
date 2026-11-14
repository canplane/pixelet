// @pxl/client/ui/feature/debug/accordion.tsx

import { CaretDownIcon, CaretUpIcon } from "phosphor-react-native";
import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { create } from "zustand";

import { CORNER_PANEL } from "@/ui/shared/component/corner-card.ts";
import { alpha, COLOR } from "@/ui/shared/component/theme.ts";
import { GAUGE } from "./perf/gauge.tsx";
import type { SettingName } from "@/ui/shared/setting.ts";

/** Opening a panel closes the one that was open, so the corner holds one open
 * panel at a time. A panel is named by the setting that shows it. */
const store = create<{ open: SettingName | null }>()(() => ({
  open: null,
}));

const toggle = (name: SettingName) =>
  store.setState(({ open }) => ({
    open: open === name ? null : name,
  }));

export const HINT_INK = alpha(COLOR.white, 0.55);

/** `note` follows the title in regular weight, since it is a reading and not a
 * name. */
export function Accordion({
  name,
  title,
  note,
  end,
  children,
}: {
  name: SettingName;
  title: string;
  note?: string;
  end?: ReactNode;
  children?: ReactNode;
}) {
  const open = store((s) => s.open === name);
  return (
    <View style={STYLE.panel}>
      <Pressable
        onPress={() => toggle(name)}
        accessibilityRole="button"
        aria-expanded={open}
      >
        <View style={STYLE.title}>
          <Text style={[GAUGE.text, STYLE.title_text]}>
            {title}
            {note !== undefined && (
              <Text style={STYLE.note_text}>{` · ${note}`}</Text>
            )}
          </Text>
          <View style={STYLE.end}>
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

const STYLE = StyleSheet.create({
  panel: {
    ...CORNER_PANEL,
    flexShrink: 1,
    overflow: "hidden",
    pointerEvents: "box-none",
  },
  title: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title_text: {
    fontSize: 11,
    fontWeight: "600",
    letterSpacing: 0.6,
    textTransform: "uppercase",
    color: alpha(COLOR.white, 0.75),
  },
  note_text: { fontWeight: "400" },
  end: { flexDirection: "row", alignItems: "center", gap: 6 },
});
