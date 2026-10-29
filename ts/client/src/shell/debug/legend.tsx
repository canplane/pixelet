// @pxl/client/shell/debug/legend.tsx

/**
 * What each debug view's marks mean, each legend shown while its setting is
 * on. Colors, dashes and bounds are the spec's, which the views are drawn in:
 * the world gizmo's lines the kernel's (`gizmo.spec.ts`), the camera gizmo's
 * and the ray steps' the renderer's (`debug_look.spec.ts`).
 */

import { StyleSheet, Text, View } from "react-native";
import { Defs, LinearGradient, Rect, Stop, Svg } from "react-native-svg";

import { SETTINGS } from "@/shell/setting/schema.ts";
import { gauge, Head } from "./profiler/gauge.tsx";
import { Fold } from "./fold.tsx";
import { coverage, frame } from "@/abi/gizmo.spec.ts";
import { grid, share, steps } from "@/abi/debug_look.spec.ts";
import { useKernel } from "@/shell/store/watch.ts";

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

const SHARE: Record<keyof typeof share, string> = {
  finer: `finer, up to ${share.finer.most} grid cell`,
  within: `${share.finer.most} to ${share.within.most} grid cells`,
  coarser: `coarser, past ${share.within.most}`,
};

const hex = (rgb: number) => `#${rgb.toString(16).padStart(6, "0")}`;

const useShown = (name: "world_gizmo" | "camera_gizmo" | "ray_steps") =>
  useKernel((s) => s.settings[SETTINGS[name].key] === 1);

/**
 * In the spec's order, or `reversed` to list the one drawn over the rest
 * first; a line's look as a stroke, or `filled` a tint's as a patch.
 */
function Rows<K extends string>(
  { looks, labels, reversed, filled }: {
    looks: Record<K, Look>;
    labels: Record<K, string>;
    reversed?: boolean;
    filled?: boolean;
  },
) {
  const keys = Object.keys(looks) as K[];
  return (reversed ? keys.reverse() : keys).map((k) => (
    <Row key={k} look={looks[k]} filled={filled}>{labels[k]}</Row>
  ));
}

function Row(
  { look, filled, children }: {
    look: Look;
    filled?: boolean;
    children: string;
  },
) {
  return (
    <View style={styles.row}>
      <View style={styles.chip}>
        <View
          style={filled
            ? [styles.patch, { backgroundColor: hex(look.rgb) }]
            : [styles.stroke, {
              borderColor: hex(look.rgb),
              borderStyle: look.dashed ? "dashed" : "solid",
              opacity: look.alpha ?? 1,
            }]}
        />
      </View>
      <Text style={gauge.text}>{children}</Text>
    </View>
  );
}

export function WorldGizmoLegend() {
  if (!useShown("world_gizmo")) return null;
  return (
    <Fold name="world_gizmo" title="World Gizmo">
      <Head>Frame</Head>
      <Rows looks={frame} labels={FRAME} />
      <Head>Coverage</Head>
      <Rows looks={coverage} labels={COVERAGE} reversed />
    </Fold>
  );
}

export function CameraGizmoLegend() {
  if (!useShown("camera_gizmo")) return null;
  return (
    <Fold name="camera_gizmo" title="Camera Gizmo">
      <Head>Grid</Head>
      <Row look={grid}>a cell of one solid angle</Row>
      <Head>Share</Head>
      <Rows looks={share} labels={SHARE} filled />
    </Fold>
  );
}

/** Where `n` steps fall along the ramp, which is log-scaled to `most`. */
const along = (n: number) => Math.log2(1 + n) / Math.log2(1 + steps.most);
const TICKS = [1, Math.sqrt(steps.most), steps.most];

export function RayStepsLegend() {
  if (!useShown("ray_steps")) return null;
  const last = steps.ramp.length - 1;
  return (
    <Fold name="ray_steps" title="Ray Steps">
      <Head>Steps</Head>
      <View style={styles.ramp}>
        <Svg width="100%" height={8}>
          <Defs>
            <LinearGradient id="steps-ramp" x1="0" y1="0" x2="1" y2="0">
              {steps.ramp.map((rgb, i) => (
                <Stop key={i} offset={i / last} stopColor={hex(rgb)} />
              ))}
            </LinearGradient>
          </Defs>
          <Rect width="100%" height="100%" fill="url(#steps-ramp)" />
        </Svg>
        <View style={styles.ticks}>
          {TICKS.map((n) => (
            <Text
              key={n}
              style={[gauge.text, styles.tick, { left: `${along(n) * 100}%` }]}
            >
              {n}
            </Text>
          ))}
        </View>
      </View>
      <Row look={{ rgb: steps.none }} filled>none walked</Row>
    </Fold>
  );
}

const TICK_WIDTH = 32;

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
  // Inset by half a tick's label, so the ends' labels stay inside.
  ramp: { marginHorizontal: TICK_WIDTH / 2, marginVertical: 4 },
  ticks: { height: 16 },
  tick: {
    position: "absolute",
    width: TICK_WIDTH,
    marginLeft: -TICK_WIDTH / 2,
    textAlign: "center",
  },
});
