// @pxl/client/ui/feature/debug/perf/gauge.tsx

import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { type FrameGroup, GROUPS, n1, val_text, zoom_name } from "./column.ts";

const bytes = (b: number) => val_text("_bytes", b);
import { fail_text } from "./report.ts";
import { fps, type Gauge } from "./view.ts";
import { alpha, color } from "@/ui/shared/component/theme.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { HIT_KINDS } from "@/abi/perf.spec.ts";

const SPAN_ROWS = 3;

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

export const Head = ({
  children,
  end,
}: {
  children: string;
  end?: ReactNode;
}) => (
  <View style={gauge.head}>
    <Text style={[gauge.text, gauge.head_text]}>{children}</Text>
    <View style={gauge.head_line} />
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
const hit_kind_text = (k: number) => {
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
      <Row k="alt" v={meters(v.pivot_alt)} />
      <Head>Pointer</Head>
      <Row k="coord" v={coordinate(v.hover_x, v.hover_y, v.hover_z)} />
      <Row k="drawn at" v={zoom_name(v.cell_zoom)} />
      <Row
        k="hit"
        v={
          picked
            ? hit_kind_text(v.hit_kind)
            : Number.isNaN(v.hover_x)
              ? "-"
              : "nothing in reach"
        }
      />
      <Row
        k="hit/drawn at"
        v={
          picked
            ? `${zoom_name(v.pick_zoom)}/${zoom_name(v.ray_zoom)}${folded ? " folded" : ""}`
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
            : `${zoom_name(v.place_zoom)}${v.blocked === 1 ? " blocked" : ""}`
        }
        warn={v.blocked === 1}
      />
    </>
  );
}

export function PerfRec({ rec }: { rec: Gauge["rec"] }) {
  return (
    <View style={gauge.rec}>
      <View style={gauge.row}>
        <View style={gauge.rec_state}>
          <View style={[gauge.rec_dot, rec.recording && gauge.rec_dot_on]} />
          <Text style={[gauge.text, rec.recording && gauge.warn]}>
            {rec.recording ? "Recording" : "Stopped"}
          </Text>
        </View>
        <Text style={gauge.text}>{`${fixed(rec.elapsed_ms / 1000, 1)} s`}</Text>
      </View>
      <Row
        k="frames/tasks"
        v={`${rec.frame_cnt}/${rec.task_cnt}${rec.truncated ? " truncated" : ""}`}
        warn={rec.truncated}
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
  const failing = g.tasks.filter((t) => t.err > 0);

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
          v={f.gpu_timed > 0 ? `${n1(f.gpu_busy.p95)} ms` : "no timestamps"}
          warn={f.gpu_timed > 0 && f.gpu_busy.p95 > f.last.paced_ms}
        />
        {heaviest(f.gpu_spans, (s) => s.p95, SPAN_ROWS).map((s) => (
          <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.p95)} ms`} />
        ))}
        <Row
          k="scale/drawn"
          v={`${fixed(f.last.scale, 2)}/${
            f.gpu_px > 0 ? val_text("gpu_px", f.gpu_px) : "-"
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
        v={`${f.heap_used_delta_bytes >= 0 ? "+" : ""}${bytes(
          f.heap_used_delta_bytes,
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
              v={`${t.err} of ${t.n}${
                t.reasons.length > 0
                  ? ` ${fail_text(t.reasons[0].fail, t.reasons[0].detail)}`
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

export const gauge = StyleSheet.create({
  // Every digit takes the same width, so that a column of figures lines up.
  text: {
    fontSize: 10.5,
    lineHeight: 16,
    fontVariant: ["tabular-nums"],
    color: alpha(color.white, 0.9),
  },
  key: { color: alpha(color.white, 0.45) },
  warn: { color: color.perf_halt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  head: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    marginTop: 8,
    marginBottom: 2,
  },
  // 10 is macOS's smallest text style: smaller text does not render on a 1x
  // screen.
  head_text: { fontSize: 10, color: alpha(color.white, 0.3) },
  rec: { marginTop: 8 },
  rec_state: { flexDirection: "row", alignItems: "center", gap: 6 },
  rec_dot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: alpha(color.white, 0.25),
  },
  rec_dot_on: { backgroundColor: color.perf_halt },
  head_line: {
    flex: 1,
    borderTopWidth: 1,
    borderTopColor: alpha(color.white, 0.1),
  },
});
