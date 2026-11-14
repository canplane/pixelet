// @pxl/client/ui/shared/component/segmented.tsx

import { Pressable, StyleSheet, Text, View } from "react-native";

import { PANEL_CONTROL } from "@/ui/shared/component/panel.tsx";
import {
  CHROME,
  COLOR,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
  WEIGHT,
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
                maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
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

const INSET = (PANEL_CONTROL - CHROME.selection) / 2;

const styles = StyleSheet.create({
  track: {
    flexDirection: "row",
    height: PANEL_CONTROL,
    borderRadius: PANEL_CONTROL / 2,
    backgroundColor: COLOR.faint,
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
    backgroundColor: COLOR.ground,
    boxShadow: "0 1px 2px rgba(0, 0, 0, 0.08)",
  },
  label: {
    ...TEXT_STYLE.footnote,
    fontWeight: WEIGHT.medium,
    color: LABEL.secondary,
  },
  label_on: { color: LABEL.primary },
});
