// @pxl/client/ui/feature/debug/perf/gauge.tsx

import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { type FrameGroup, GROUPS, n1, valueText, zoomName } from "./lane.ts";

const bytes = (b: number) => valueText("_bytes", b);
import { failText } from "./report.ts";
import { fps, type Gauge } from "./view.ts";
import { alpha, COLOR } from "@/ui/shared/component/theme.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { HIT_KINDS } from "@/abi/perf.spec.ts";

const SPAN_ROWS = 3;

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <View style={GAUGE.row}>
      <Text style={[GAUGE.text, GAUGE.key]}>{props.k}</Text>
      <Text style={[GAUGE.text, props.warn && GAUGE.warn]}>{props.v}</Text>
    </View>
  );
}

const Note = ({ children }: { children: string }) => (
  <Text style={[GAUGE.text, GAUGE.key]}>{children}</Text>
);

export const Head = ({
  children,
  end,
}: {
  children: string;
  end?: ReactNode;
}) => (
  <View style={GAUGE.head}>
    <Text style={[GAUGE.text, GAUGE.headText]}>{children}</Text>
    <View style={GAUGE.headLine} />
    {end}
  </View>
);

const deg = (rad: number) => (rad * 180) / Math.PI;
const or = (x: number) => (Number.isNaN(x) ? "-" : n1(x));
const geo = (lat: number, lon: number) =>
  Number.isNaN(lat) ? "-" : `${fixed(lat, 5)}°, ${fixed(lon, 5)}°`;
const meters = (m: number) => (Number.isNaN(m) ? "-" : `${n1(m)} m`);

const HIT_KIND_TEXT: Record<(typeof HIT_KINDS)[number], string> = {
  ground: "ground",
  object: "cube",
  objects: "cubes, folded",
  decor: "decor",
};
const hitKindText = (k: number) => {
  const kind = HIT_KINDS[k];
  return kind === undefined ? "-" : HIT_KIND_TEXT[kind];
};

/** A coordinate in units, to three decimals: enough for an eighth of a unit,
 * the side of the finest cell (z27). Trailing zeros are dropped. */
const unit = (u: number) => fixed(u, 3).replace(/\.?0+$/, "");
const coordinate = (x: number, y: number, z: number) =>
  Number.isNaN(x) ? "-" : `${unit(x)}, ${unit(y)}, ${unit(z)}`;

/**
 * Where the camera's eye, its pivot and the pointer are at this moment, not
 * over the window. Each is a coordinate in units, and the pivot is also given
 * on the Earth. The pointer is shown twice: what it meets at the zooms the
 * eye draws, and where the tool in hand would strike there. When a hover
 * would do nothing, the two show where they part.
 */
export function PerfView({ v }: { v: Gauge["view"] }) {
  const picked = !Number.isNaN(v.hit_kind);
  const folded = v.folded === 1;
  return (
    <>
      <Head>Camera</Head>
      <Row k="coord" v={coordinate(v.eye_x, v.eye_y, v.eye_z)} />
      <Row k="distance" v={`${n1(v.dist)} u`} />
      <Row
        k="polar/azimuth"
        v={`${n1(deg(v.polar))}°/${n1(deg(v.azimuth))}°`}
      />
      <Head>Pivot</Head>
      <Row k="coord" v={coordinate(v.pivot_x, v.pivot_y, v.pivot_z)} />
      <Row k="lat/lon" v={geo(v.pivot_lat, v.pivot_lon)} />
      <Row k="ele" v={meters(v.pivot_ele)} />
      <Head>Pointer</Head>
      <Row k="coord" v={coordinate(v.hover_x, v.hover_y, v.hover_z)} />
      <Row k="drawn at" v={zoomName(v.cell_zoom)} />
      <Row
        k="hit"
        v={
          picked
            ? hitKindText(v.hit_kind)
            : Number.isNaN(v.hover_x)
              ? "-"
              : "nothing in reach"
        }
      />
      <Row
        k="hit/drawn at"
        v={
          picked
            ? `${zoomName(v.pick_zoom)}/${zoomName(v.ray_zoom)}${folded ? " folded" : ""}`
            : "-"
        }
        warn={folded}
      />
      <Row k="places at" v={coordinate(v.place_x, v.place_y, v.place_z)} />
      <Row
        k="places at zoom"
        v={
          Number.isNaN(v.place_zoom)
            ? "-"
            : `${zoomName(v.place_zoom)}${v.blocked === 1 ? " blocked" : ""}`
        }
        warn={v.blocked === 1}
      />
    </>
  );
}

export function PerfRecording({
  recordingState,
}: {
  recordingState: Gauge["recordingState"];
}) {
  return (
    <View style={GAUGE.recording}>
      <View style={GAUGE.row}>
        <View style={GAUGE.recordingStatus}>
          <View
            style={[
              GAUGE.recordingDot,
              recordingState.recording && GAUGE.recordingDotOn,
            ]}
          />
          <Text style={[GAUGE.text, recordingState.recording && GAUGE.warn]}>
            {recordingState.recording ? "Recording" : "Stopped"}
          </Text>
        </View>
        <Text
          style={GAUGE.text}
        >{`${fixed(recordingState.elapsedMs / 1000, 1)} s`}</Text>
      </View>
      <Row
        k="frames/tasks"
        v={`${recordingState.frameCount}/${recordingState.taskCount}${recordingState.truncated ? " truncated" : ""}`}
        warn={recordingState.truncated}
      />
    </View>
  );
}

export function PerfGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <Note>no window open</Note>;

  const f = g.frames;
  const heaviest = <T,>(rows: T[], by: (r: T) => number, n: number) =>
    [...rows]
      .sort((a, b) => by(b) - by(a))
      .slice(0, n)
      .filter((r) => by(r) > 0);
  const failing = g.tasks.filter((t) => t.failed > 0);

  // The gauge is a glance: how smooth the frames are, where a frame's time
  // goes, the scale it is drawn at, and whether anything leaks or fails. The
  // report holds the rest, so the other groups show nothing here.
  const sections: Record<FrameGroup, ReactNode> = {
    time: f !== null && (
      <>
        <Row
          k="fps window/p50"
          v={`${or(fps(f) ?? NaN)}/${
            f.interval.p50 > 0 ? n1(1000 / f.interval.p50) : "-"
          }`}
        />
        <Row
          k="interval p50/p95"
          v={`${n1(f.interval.p50)}/${n1(f.interval.p95)} ms`}
          warn={f.interval.p95 > f.last.paced_ms + f.last.refresh_ms / 2}
        />
        <Row
          k="refresh/paced"
          v={`${n1(f.last.refresh_ms)}/${n1(f.last.paced_ms)} ms`}
        />
      </>
    ),
    cpu: null,
    frame: null,
    gpu: f !== null && (
      <>
        {/* GPU time is known only for the frames a GPU read timed. When none
            was timed there is no measure, and it is shown as such, not as 0. */}
        <Row
          k="gpu busy p95"
          v={f.gpuTimed > 0 ? `${n1(f.gpuBusy.p95)} ms` : "no timestamps"}
          warn={f.gpuTimed > 0 && f.gpuBusy.p95 > f.last.paced_ms}
        />
        {heaviest(f.gpuSpans, (s) => s.p95, SPAN_ROWS).map((s) => (
          <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.p95)} ms`} />
        ))}
        <Row
          k="scale/drawn"
          v={`${fixed(f.last.scale, 2)}/${
            f.gpuPx > 0 ? valueText("gpu_px", f.gpuPx) : "-"
          }`}
          warn={f.last.scale < 1}
        />
      </>
    ),
    rays: null,
    tiles: null,
    device: null,
    heap: f !== null && (
      <Row
        k="heap used change"
        v={`${f.heapUsedDeltaBytes >= 0 ? "+" : ""}${bytes(
          f.heapUsedDeltaBytes,
        )}`}
      />
    ),
  };

  return (
    <>
      {f === null && <Note>no frames sampled yet</Note>}
      {GROUPS.filter((group) => sections[group]).map((group) => (
        <View key={group}>
          <Head>{group}</Head>
          {sections[group]}
        </View>
      ))}
      {failing.length > 0 && (
        <View>
          <Head>requests</Head>
          {failing.map((t) => (
            <Row
              key={t.stream}
              k={`${t.stream} failed`}
              v={`${t.failed} of ${t.n}${
                t.reasons.length > 0
                  ? ` ${failText(t.reasons[0].fail, t.reasons[0].detail)}`
                  : ""
              }`}
              warn
            />
          ))}
        </View>
      )}
    </>
  );
}

export const GAUGE = StyleSheet.create({
  // Every digit takes the same width, so that a column of figures lines up.
  text: {
    fontSize: 10.5,
    lineHeight: 16,
    fontVariant: ["tabular-nums"],
    color: alpha(COLOR.white, 0.9),
  },
  key: { color: alpha(COLOR.white, 0.45) },
  warn: { color: COLOR.perfHalt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  head: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    marginTop: 8,
    marginBottom: 2,
  },
  // 10 is the least text size Apple's guidelines give macOS.
  headText: { fontSize: 10, color: alpha(COLOR.white, 0.3) },
  recording: { marginTop: 8 },
  recordingStatus: { flexDirection: "row", alignItems: "center", gap: 6 },
  recordingDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: alpha(COLOR.white, 0.25),
  },
  recordingDotOn: { backgroundColor: COLOR.perfHalt },
  headLine: {
    flex: 1,
    borderTopWidth: 1,
    borderTopColor: alpha(COLOR.white, 0.1),
  },
});
