// @pxl/client/shell/ui/segmented.tsx

/** A segmented control, drawn on a page and on Android; an iPhone's is UIKit's (segmented.ios.tsx). */

import { Pressable, StyleSheet, Text, View } from "react-native";

import { PANEL_CONTROL } from "@/shell/ui/panel.tsx";
import {
  chrome,
  color,
  textScale,
  textStyle,
  weight,
} from "@/shell/ui/theme.ts";
import { LABEL } from "@/shell/ui/palette.ts";

export interface SegmentedProps<T extends string | number> {
  segments: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}

export function Segmented<T extends string | number>(
  { segments, value, onChange }: SegmentedProps<T>,
) {
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
            {/* Always mounted, as dock/strip.tsx `ToolButton` is. */}
            <View collapsable={false} style={[styles.segment, on && styles.on]}>
              <Text
                maxFontSizeMultiplier={textScale.chrome}
                style={[styles.label, on && styles.labelOn]}
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
  // The segment's radius is the track's minus the inset, so the two are concentric.
  reach: { flex: 1, padding: INSET },
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
    ...textStyle.footnote,
    fontWeight: weight.medium,
    color: LABEL.secondary,
  },
  labelOn: { color: LABEL.primary },
});
