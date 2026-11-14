// @pxl/client/ui/shared/component/segmented.tsx

import { Pressable, StyleSheet, Text, View } from "react-native";

import { PANEL_CONTROL } from "@/ui/shared/component/panel.tsx";
import {
  chrome,
  color,
  text_scale,
  text_style,
  weight,
} from "@/ui/shared/component/theme.ts";
import { LABEL } from "@/ui/shared/component/palette.ts";

export interface SegmentedProps<T extends string | number> {
  segments: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}

export function Segmented<T extends string | number>({
  segments,
  value,
  onChange,
}: SegmentedProps<T>) {
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
            style={styles.target}
          >
            {/* The segment stays mounted and only its color changes. On
                Android, a view created with its background already set is
                drawn without its rounded corners. */}
            <View collapsable={false} style={[styles.segment, on && styles.on]}>
              <Text
                maxFontSizeMultiplier={text_scale.chrome}
                style={[styles.label, on && styles.label_on]}
              >
                {s.label}
              </Text>
            </View>
          </Pressable>
        );
      })}
    </View>
  );
}

const INSET = (PANEL_CONTROL - chrome.selection) / 2;

const styles = StyleSheet.create({
  track: {
    flexDirection: "row",
    height: PANEL_CONTROL,
    borderRadius: PANEL_CONTROL / 2,
    backgroundColor: color.faint,
  },
  target: { flex: 1, padding: INSET },
  // The segment's corners are concentric with the track's.
  segment: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: PANEL_CONTROL / 2 - INSET,
  },
  on: {
    backgroundColor: color.ground,
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.08)",
  },
  label: {
    ...text_style.footnote,
    fontWeight: weight.medium,
    color: LABEL.secondary,
  },
  label_on: { color: LABEL.primary },
});
