// @pxl/web/feature/debug/gizmo-legend.tsx

/**
 * What each line of the world gizmo means, shown while the `world_gizmo`
 * setting is on. Colors and dashes are the spec's (`gizmo.spec.ts`), which
 * the kernel draws the lines in.
 */

import { StyleSheet, Text, View } from "react-native";

import { useKernel } from "@/kernel/shell.ts";
import { SETTINGS } from "@/feature/setting/schema.ts";
import { alpha, color } from "@/shared/ui/theme.ts";
import { gauge, Head } from "./profiler/gauge.tsx";
import { Fold } from "./fold.tsx";
import { coverage, frame } from "./gizmo.spec.ts";

interface Look {
  rgb: number;
  dashed: boolean;
}

const FRAME: Record<keyof typeof frame, string> = {
  focus: "focus and its cell",
  level: "level ring",
  forward: "upright ring, forward",
  north: "ring through north",
  up: "up",
};

// Highest first: a shared stretch is drawn in the highest.
const COVERAGE: Record<keyof typeof coverage, string> = {
  shard: "a drawn piece's shard",
  tile: "a drawn piece's tile",
  no_tile: "goal, no tile on its line",
  floor_only: "goal, only the floor's tile",
  unbuilt: "goal, mesh not built",
  drawn_from_floor: "drawn from the floor's tile",
  drawn: "drawn",
};

function Rows<K extends string>({ looks, labels }: { looks: Record<K, Look>; labels: Record<K, string> }) {
  return (Object.keys(labels) as K[]).map((k) => (
    <View key={k} style={styles.row}>
      <View style={styles.chip}>
        <View style={[styles.swatch, { borderColor: `#${looks[k].rgb.toString(16).padStart(6, "0")}`, borderStyle: looks[k].dashed ? "dashed" : "solid" }]} />
      </View>
      <Text style={gauge.text}>{labels[k]}</Text>
    </View>
  ));
}

export function GizmoLegend() {
  const show = useKernel((s) => s.settings[SETTINGS.world_gizmo.key] === 1);
  if (!show) return null;
  return (
    <Fold name="gizmo" title="World gizmo">
      <Head>Frame</Head>
      <Rows looks={frame} labels={FRAME} />
      <Head>Coverage</Head>
      <Rows looks={coverage} labels={COVERAGE} />
    </Fold>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
  // The lines are drawn over the world, so a swatch stands on a light ground, where black reads too.
  chip: { width: 28, height: 12, justifyContent: "center", paddingHorizontal: 3, borderRadius: 2, backgroundColor: alpha(color.white, 0.55) },
  swatch: { borderTopWidth: 2 },
});
