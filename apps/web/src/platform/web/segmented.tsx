// @pxl/web/platform/web/segmented.tsx

import { Pressable, StyleSheet, Text, View } from "react-native";

import { PANEL_CONTROL } from "@/shared/ui/panel.tsx";
import { color, textStyle, weight } from "@/shared/ui/theme.ts";

export interface Segment<T extends string | number> {
  value: T;
  label: string;
}

/** One of a few views of the same panel, picked from a row of segments. */
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
            style={[styles.segment, on && styles.on]}
          >
            <Text style={[styles.label, on && styles.labelOn]}>{s.label}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

/** Gap between the track and the selected segment. */
const INSET = 3;

const styles = StyleSheet.create({
  // Track and segment are both capsules; the segment's radius is the track's
  // minus the inset, so the two are concentric.
  track: { flexDirection: "row", height: PANEL_CONTROL, padding: INSET, borderRadius: PANEL_CONTROL / 2, backgroundColor: color.faint },
  segment: { flex: 1, alignItems: "center", justifyContent: "center", borderRadius: PANEL_CONTROL / 2 - INSET },
  on: { backgroundColor: color.paper, boxShadow: "0 1px 2px rgba(0, 0, 0, 0.08)" },
  label: { ...textStyle.footnote, fontWeight: weight.medium, color: color.textMuted },
  labelOn: { color: color.text },
});
