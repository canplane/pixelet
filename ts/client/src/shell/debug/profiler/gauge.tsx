// @pxl/client/shell/debug/profiler/gauge.tsx

/**
 * The kernel's numbers over the last few seconds as a corner panel, read in
 * place from the summary the report is built from. Read-only.
 */

import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { type FrameGroup, GROUPS, n1, valueText, zoomName } from "./lane.ts";

const bytes = (b: number) => valueText("_bytes", b);
import { failText } from "./report.ts";
import { fps, type Gauge } from "./view.ts";
import { alpha, color, textStyle } from "@/shell/ui/theme.ts";
import { fixed } from "@/shell/fixed.ts";

const SPAN_ROWS = 3;
const MISSED_WARN_SHARE = 0.1;

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <View style={gauge.row}>
      <Text style={[gauge.text, gauge.key]}>{props.k}</Text>
      <Text style={[gauge.text, props.warn && gauge.warn]}>{props.v}</Text>
    </View>
  );
}

const Note = ({ children }: { children: string }) => (
  <Text style={[gauge.text, gauge.key]}>{children}</Text>
);

export const Head = (
  { children, end }: { children: string; end?: ReactNode },
) => (
  <View style={gauge.head}>
    <Text style={[gauge.text, gauge.headText]}>{children}</Text>
    <View style={gauge.headLine} />
    {end}
  </View>
);

const deg = (rad: number) => (rad * 180) / Math.PI;
const or = (x: number) => (Number.isNaN(x) ? "-" : n1(x));
const geo = (
  lat: number,
  lon: number,
) => (Number.isNaN(lat) ? "-" : `${fixed(lat, 5)}°, ${fixed(lon, 5)}°`);
const metres = (m: number) => (Number.isNaN(m) ? "-" : `${n1(m)} m`);

/** What the pick met, by the kernel's code (profiler.json `SUMMARY.view`). */
const STRUCK = ["ground", "cube", "cubes, folded", "decor"];

const units3 = (
  x: number,
  y: number,
  z: number,
) => (Number.isNaN(x)
  ? "-"
  : `${Math.floor(x)}, ${Math.floor(y)}, ${Math.floor(z)}`);

/**
 * Live: where the camera and the pointer are now, not over the window. The
 * pointer twice: what it meets at the eye's zooms, and what the tool held strikes
 * there, so a hover that takes nothing shows where the two part.
 */
export function ProfilerView({ v }: { v: Gauge["view"] }) {
  const picked = !Number.isNaN(v.struck);
  const folded = v.folded === 1;
  return (
    <>
      <Head>Camera</Head>
      <Row k="distance (units)" v={n1(v.dist)} />
      <Row
        k="polar/azimuth"
        v={`${n1(deg(v.polar))}°/${n1(deg(v.azimuth))}°`}
      />
      <Row k="center lat/lon" v={geo(v.center_lat, v.center_lon)} />
      <Row k="center altitude" v={metres(v.center_z)} />
      <Head>Under the pointer, as drawn</Head>
      <Row k="x/y/z (units)" v={units3(v.hover_x, v.hover_y, v.hover_z)} />
      <Row k="lat/lon" v={geo(v.hover_lat, v.hover_lon)} />
      <Row k="altitude" v={metres(v.hover_alt)} />
      <Row k="drawn at" v={zoomName(v.cell)} />
      <Head>What the tool strikes</Head>
      <Row
        k="struck"
        v={picked
          ? (STRUCK[v.struck] ?? "-")
          : Number.isNaN(v.hover_x)
          ? "-"
          : "nothing in reach"}
      />
      <Row
        k="held/drawn at"
        v={picked
          ? `${zoomName(v.held)}/${zoomName(v.drawn)}${folded ? " folded" : ""}`
          : "-"}
        warn={folded}
      />
      <Row
        k="places at x/y/z (units)"
        v={units3(v.place_x, v.place_y, v.place_z)}
      />
      <Row
        k="places a cube of"
        v={Number.isNaN(v.place_zoom)
          ? "-"
          : `${zoomName(v.place_zoom)}${v.blocked === 1 ? " blocked" : ""}`}
        warn={v.blocked === 1}
      />
    </>
  );
}

/** Whether the window is recording, under the buttons that start and stop it. */
export function ProfilerRec({ rec }: { rec: Gauge["rec"] }) {
  return (
    <View style={gauge.rec}>
      <View style={gauge.row}>
        <View style={gauge.recState}>
          <View style={[gauge.recDot, rec.recording && gauge.recDotOn]} />
          <Text style={[gauge.text, rec.recording && gauge.warn]}>
            {rec.recording ? "Recording" : "Stopped"}
          </Text>
        </View>
        <Text style={gauge.text}>{`${fixed(rec.elapsed_ms / 1000, 1)} s`}</Text>
      </View>
      <Row
        k="frames/tasks"
        v={`${rec.frames}/${rec.tasks}${rec.truncated ? " truncated" : ""}`}
        warn={rec.truncated}
      />
    </View>
  );
}

export function ProfilerGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <Note>no window open</Note>;

  const f = g.frames;
  const heaviest = <T,>(rows: T[], by: (r: T) => number, n: number) =>
    [...rows].sort((a, b) => by(b) - by(a)).slice(0, n).filter((r) =>
      by(r) > 0
    );
  const failing = g.tasks.filter((t) => t.error > 0);

  // A glance: how smooth, where the frame's time goes, what it is drawn at,
  // and whether anything leaks or fails. The report holds the rest.
  const sections: Record<FrameGroup, ReactNode> = {
    time: f !== null && (
      <>
        {/* Frames a second over the window, and at the median interval. */}
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
          k="missed/moving"
          v={`${f.missed}/${f.moving}`}
          warn={f.missed > f.moving * MISSED_WARN_SHARE}
        />
        <Row
          k="refresh/paced"
          v={`${n1(f.last.refresh_ms)}/${n1(f.last.paced_ms)} ms`}
        />
        <Row
          k="cpu busy p95"
          v={`${n1(f.busy.p95)} ms`}
          warn={f.busy.p95 > f.last.paced_ms}
        />
      </>
    ),
    turn: null,
    frame: null,
    gpu: f !== null && (
      <>
        {/* Over the frames a GPU read timed; none timed is no measure, not 0. */}
        <Row
          k="gpu p95"
          v={f.gpu_read > 0 ? `${n1(f.gpu.p95)} ms` : "no timestamps"}
          warn={f.gpu_read > 0 && f.gpu.p95 > f.last.paced_ms}
        />
        {heaviest(f.gpu_spans, (s) => s.p95, SPAN_ROWS).map((s) => (
          <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.p95)} ms`} />
        ))}
        <Row
          k="scale/drawn"
          v={`${fixed(f.last.scale, 2)}/${
            f.last.gpu_read ? valueText("gpu_px", f.last.gpu_px) : "-"
          }`}
          warn={f.last.scale < 1}
        />
      </>
    ),
    cap: null,
    rays: null,
    eye: null,
    sun: null,
    requests: failing.length > 0 && (
      <>
        {failing.map((t) => (
          <Row
            key={t.stream}
            k={`${t.stream} failed`}
            v={`${t.error} of ${t.n}${
              t.reasons.length > 0
                ? ` ${failText(t.reasons[0].fail, t.reasons[0].detail)}`
                : ""
            }`}
            warn
          />
        ))}
      </>
    ),
    tiles: null,
    device: null,
    heap: f !== null && (
      <Row
        k="heap used change"
        v={`${f.heap_used_delta_bytes >= 0 ? "+" : ""}${
          bytes(f.heap_used_delta_bytes)
        }`}
      />
    ),
    ring: null,
    body: null,
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
    </>
  );
}

export const gauge = StyleSheet.create({
  // Figures a fixed width, so a column of them lines up.
  text: {
    fontFamily: textStyle.caption.fontFamily,
    fontSize: 10.5,
    lineHeight: 16,
    fontVariant: ["tabular-nums"],
    color: alpha(color.white, 0.9),
  },
  key: { color: alpha(color.white, 0.45) },
  warn: { color: color.profilerHalt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  head: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    marginTop: 8,
    marginBottom: 2,
  },
  // macOS's smallest text style; smaller does not render on a 1x screen.
  headText: { fontSize: 10, color: alpha(color.white, 0.3) },
  rec: { marginTop: 8 },
  recState: { flexDirection: "row", alignItems: "center", gap: 6 },
  recDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: alpha(color.white, 0.25),
  },
  recDotOn: { backgroundColor: color.profilerHalt },
  headLine: {
    flex: 1,
    borderTopWidth: 1,
    borderTopColor: alpha(color.white, 0.1),
  },
});
