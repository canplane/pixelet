// @pxl/web/feature/debug/gizmo-legend.tsx

/**
 * What each line of the world gizmo means, shown while the `world_gizmo`
 * setting is on. Colors and dashes are the spec's (`gizmo.spec.ts`), which
 * the kernel draws the lines in.
 */

import { StyleSheet, Text, View } from "react-native";

import { useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { gauge, Head } from "./profiler/gauge.tsx";
import { Fold } from "./fold.tsx";
import { frame } from "./gizmo.spec.ts";

interface Look {
  rgb: number;
  alpha?: number;
  dashed?: boolean;
}

const FRAME: Record<keyof typeof frame, string> = {
  focus: "focus and its cell",
  level: "level ring",
  forward: "upright ring, forward",
  north: "ring through north",
  up: "up",
};

const hex = (rgb: number) => `#${rgb.toString(16).padStart(6, "0")}`;

/** In the spec's order, or `reversed` to list the one drawn over the rest first. */
function Rows<K extends string>({ looks, labels, reversed }: { looks: Record<K, Look>; labels: Record<K, string>; reversed?: boolean }) {
  const keys = Object.keys(looks) as K[];
  return (reversed ? keys.reverse() : keys).map((k) => (
    <View key={k} style={styles.row}>
      <View style={styles.chip}>
        <View style={[styles.swatch, { borderColor: hex(looks[k].rgb), borderStyle: looks[k].dashed ? "dashed" : "solid", opacity: looks[k].alpha ?? 1 }]} />
      </View>
      <Text style={gauge.text}>{labels[k]}</Text>
    </View>
  ));
}

export function GizmoLegend() {
  const show = useKernel((s) => s.settings[SETTINGS.world_gizmo.key] === 1);
  if (!show) return null;
  return (
    <Fold name="world_gizmo" title="World Gizmo">
      <Head>Frame</Head>
      <Rows looks={frame} labels={FRAME} />
    </Fold>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
  // A mid gray, where both the black marks and the faint white one read.
  chip: { width: 28, height: 12, justifyContent: "center", paddingHorizontal: 3, borderRadius: 2, backgroundColor: "#808080" },
  swatch: { borderTopWidth: 2 },
});
