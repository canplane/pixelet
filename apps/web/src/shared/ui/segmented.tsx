// @pxl/web/shared/ui/segmented.tsx

import type { ReactNode } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";

import { alpha, color, textStyle, weight } from "@/shared/ui/theme.ts";

interface Segment<T extends string | number> {
  value: T;
  label: string;
  icon?: ReactNode;
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
            {s.icon}
            <Text style={[styles.label, on && styles.labelOn]}>{s.label}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  // Capsules, the track and the segment in it alike.
  track: { flexDirection: "row", height: 36, padding: 3, borderRadius: 18, backgroundColor: alpha(color.wash, 0.6) },
  segment: { flex: 1, flexDirection: "row", alignItems: "center", justifyContent: "center", gap: 6, borderRadius: 15 },
  on: { backgroundColor: color.paper, boxShadow: "0 1px 2px rgba(0, 0, 0, 0.08)" },
  label: { ...textStyle.footnote, fontWeight: weight.medium, color: color.textMuted },
  labelOn: { color: color.text },
});
