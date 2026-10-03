// @pxl/web/platform/web/segmented.tsx

import { Pressable, StyleSheet, Text, View } from "react-native";

import { PANEL_CONTROL } from "@/shared/ui/panel.tsx";
import { chrome, color, textStyle, weight } from "@/shared/ui/theme.ts";
import { LABEL } from "@platform/palette.ts";

export interface Segment<T extends string | number> {
  value: T;
  label: string;
}

export function Segmented<T extends string | number>({ segments, value, onChange }: { segments: readonly Segment<T>[]; value: T; onChange: (value: T) => void }) {
  return (
    <View accessibilityRole="tablist" style={styles.track}>
      {segments.map((s) => {
        const on = s.value === value;
        return (
          <Pressable
            key={s.value}
            onPress={() => onChange(s.value)}
            accessibilityRole="tab"
            aria-selected={on}
            style={styles.reach}
          >
            {/* Always mounted, as dock.tsx `ToolButton` is. */}
            <View collapsable={false} style={[styles.segment, on && styles.on]}>
              <Text style={[styles.label, on && styles.labelOn]}>{s.label}</Text>
            </View>
          </Pressable>
        );
      })}
    </View>
  );
}

const INSET = (PANEL_CONTROL - chrome.selection) / 2;

const styles = StyleSheet.create({
  track: { flexDirection: "row", height: PANEL_CONTROL, borderRadius: PANEL_CONTROL / 2, backgroundColor: color.faint },
  // The segment's radius is the track's minus the inset, so the two are concentric.
  reach: { flex: 1, padding: INSET },
  segment: { flex: 1, alignItems: "center", justifyContent: "center", borderRadius: PANEL_CONTROL / 2 - INSET },
  on: { backgroundColor: color.ground, boxShadow: "0 1px 2px rgba(0, 0, 0, 0.08)" },
  label: { ...textStyle.footnote, fontWeight: weight.medium, color: LABEL.secondary },
  labelOn: { color: LABEL.primary },
});
