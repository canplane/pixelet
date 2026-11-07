// @pxl/client/ui/debug/legend.tsx

/**
 * What the world gizmo's marks mean, shown while its setting is on. Colors and
 * dashes are the spec's, which its lines are drawn in (`gizmo.spec.ts`).
 */

import { StyleSheet, Text, View } from "react-native";

import { SETTING } from "@/ui/setting/schema.ts";
import { gauge, Head } from "./perf/gauge.tsx";
import { Accordion } from "./accordion.tsx";
import { coverage, frame } from "@/abi/gizmo.spec.ts";
import { useCore } from "@/ui/store/watch.ts";

interface Look {
  rgb: number;
  alpha?: number;
  dashed?: boolean;
}

const FRAME: Record<keyof typeof frame, string> = {
  pivot: "pivot and its cell",
  level: "level ring",
  forward: "upright ring, forward",
  north: "ring through north",
  up: "up",
};

const COVERAGE: Record<keyof typeof coverage, string> = {
  drawn: "drawn",
  drawn_from_floor: "drawn from the floor's tile",
  unbuilt: "goal, mesh not built",
  floor_only: "goal, only the floor's tile",
  no_tile: "goal, no tile on its line",
  tile: "a drawn piece's tile",
  shard: "a drawn piece's shard",
};

const hex = (rgb: number) => `#${rgb.toString(16).padStart(6, "0")}`;

const useShown = (name: "world_gizmo") =>
  useCore((s) => s.settings[SETTING[name].key] === 1);

/**
 * In the spec's order, or `reversed` to list the one drawn over the rest
 * first; a line's look as a stroke, or `filled` a tint's as a patch.
 */
function Rows<K extends string>({
  looks,
  labels,
  reversed,
  filled,
}: {
  looks: Record<K, Look>;
  labels: Record<K, string>;
  reversed?: boolean;
  filled?: boolean;
}) {
  const keys = Object.keys(looks) as K[];
  return (reversed ? keys.reverse() : keys).map((k) => (
    <Row key={k} look={looks[k]} filled={filled}>
      {labels[k]}
    </Row>
  ));
}

function Row({
  look,
  filled,
  children,
}: {
  look: Look;
  filled?: boolean;
  children: string;
}) {
  return (
    <View style={styles.row}>
      <View style={styles.chip}>
        <View
          style={
            filled
              ? [styles.patch, { backgroundColor: hex(look.rgb) }]
              : [
                  styles.stroke,
                  {
                    borderColor: hex(look.rgb),
                    borderStyle: look.dashed ? "dashed" : "solid",
                    opacity: look.alpha ?? 1,
                  },
                ]
          }
        />
      </View>
      <Text style={gauge.text}>{children}</Text>
    </View>
  );
}

export function WorldGizmoLegend() {
  if (!useShown("world_gizmo")) return null;
  return (
    <Accordion name="world_gizmo" title="World Gizmo">
      <Head>Frame</Head>
      <Rows looks={frame} labels={FRAME} />
      <Head>Coverage</Head>
      <Rows looks={coverage} labels={COVERAGE} reversed />
    </Accordion>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 8 },
  // A mid gray, where both the black marks and the faint white one read.
  chip: {
    width: 28,
    height: 12,
    justifyContent: "center",
    paddingHorizontal: 3,
    borderRadius: 2,
    backgroundColor: "#808080",
  },
  stroke: { borderTopWidth: 2 },
  patch: { height: 8, borderRadius: 1 },
});
