// @pxl/web/feature/debug/profiler/gauge.tsx

/**
 * The kernel's numbers over the last few seconds as a corner panel, read in
 * place from the summary block the report is built from. Read-only.
 */

import { round } from "@xpute/kit/math/scalar.ts";
import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { EFFECT_NAMES } from "@/kernel/turn/step.spec.ts";
import { STREAM_NAMES } from "@/kernel/io/demand.spec.ts";
import { drawn, type FrameGroup, GROUPS, LIST_NAMES, MB, n1, pct, tris, valueText, zoomName } from "./lane.ts";

const bytes = (b: number) => valueText("_bytes", b);
import { BOUND, failText } from "./report.ts";
import type { FrameSummary, Gauge } from "./view.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import { kernel_memory } from "@/kernel/shell.ts";
import { alpha, color, textStyle } from "@/shared/ui/theme.ts";
import { fixed } from "@/shared/fixed.ts";

const CENSUS_ROWS = 5;
const SPAN_ROWS = 3;
const MISSED_WARN_SHARE = 0.1;
/** LoAF's own threshold for a long frame. */
const LONG_FRAME_MS = 50;
/** The unbudgeted interaction phase is meant to be O(1). */
const INTERACTION_WARN_MS = 2;

const ATLAS_SPARSE_PCT = 25;

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <View style={gauge.row}>
      <Text style={[gauge.text, gauge.key]}>{props.k}</Text>
      <Text style={[gauge.text, props.warn && gauge.warn]}>{props.v}</Text>
    </View>
  );
}

const Note = ({ children }: { children: string }) => <Text style={[gauge.text, gauge.key]}>{children}</Text>;

export const Head = ({ children, end }: { children: string; end?: ReactNode }) => (
  <View style={gauge.head}>
    <Text style={[gauge.text, gauge.headText]}>{children}</Text>
    <View style={gauge.headLine} />
    {end}
  </View>
);

const deg = (rad: number) => (rad * 180) / Math.PI;
const or = (x: number) => (Number.isNaN(x) ? "-" : n1(x));
const geo = (lat: number, lon: number) => (Number.isNaN(lat) ? "-" : `${fixed(lat, 5)}°, ${fixed(lon, 5)}°`);
const metres = (m: number) => (Number.isNaN(m) ? "-" : `${n1(m)} m`);

const phase = (f: FrameSummary, name: string): number => f.phases.find((p) => p.name === name)?.p95 ?? 0;

/** Live: where the camera and the pointer are now, not over the window. */
export function ProfilerView({ v }: { v: Gauge["view"] }) {
  const hovering = !Number.isNaN(v.hover_x);
  return (
    <>
      <Head>Camera</Head>
      <Row k="distance" v={`${n1(v.dist)} u`} />
      <Row k="polar/azimuth" v={`${n1(deg(v.polar))}°/${n1(deg(v.azimuth))}°`} />
      <Row k="τ" v={or(v.tau)} />
      <Row k="center" v={geo(v.center_lat, v.center_lon)} />
      <Row k="center altitude" v={metres(v.center_z)} />
      <Head>Pointer</Head>
      <Row k="hover (u)" v={hovering ? `${Math.floor(v.hover_x)}, ${Math.floor(v.hover_y)}, ${Math.floor(v.hover_z)}` : "-"} />
      <Row k="hover" v={geo(v.hover_lat, v.hover_lon)} />
      <Row k="hover altitude" v={metres(v.hover_alt)} />
      <Row k="cell" v={hovering ? zoomName(v.cell_want) : "-"} />
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
          <Text style={[gauge.text, rec.recording && gauge.warn]}>{rec.recording ? "Recording" : "Stopped"}</Text>
        </View>
        <Text style={gauge.text}>{`${fixed(rec.elapsed_ms / 1000, 1)} s`}</Text>
      </View>
      <Row k="frames/tasks" v={`${rec.frames}/${rec.tasks}${rec.truncated ? " truncated" : ""}`} warn={rec.truncated} />
    </View>
  );
}

export function ProfilerGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <Note>no window open</Note>;

  const f = g.frames;
  const sheetPct = pct(g.sheet.used_px, g.sheet.size * g.sheet.size);
  const heaviest = <T,>(rows: T[], by: (r: T) => number, n: number) => [...rows].sort((a, b) => by(b) - by(a)).slice(0, n).filter((r) => by(r) > 0);
  // The terrain's tiles, read off its zooms.
  const terrain = f === null ? 0 : f.last.zoom_tiles.reduce((n, z) => n + z, 0);
  const worstLong = g.longs.reduce<LongFrame | null>((w, l) => (!w || l.durationMs > w.durationMs ? l : w), null);

  // A section a group of profiler.json FRAME_GROUPS, in its order: the panel
  // reads its heads where the report does. Each holds what reads its lanes.
  const sections: Record<FrameGroup, ReactNode> = {
    time: f !== null && (
      <>
        <Row k="interval p50/p95" v={`${n1(f.interval.p50)}/${n1(f.interval.p95)} ms`} />
        <Row k="interval max" v={`${n1(f.interval.max)} ms`} warn={f.interval.max > LONG_FRAME_MS} />
        <Row k="missed/moving" v={`${f.missed}/${f.moving}`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
        <Row k="budget/display" v={`${n1(f.last.budget_ms)}/${n1(f.last.display_ms)} ms${f.last.settled ? " still" : ""}`} />
        {/* The interval's turns, rise to fall, and what they left of it: past it is a long frame. */}
        <Row k="busy/off-turn p95" v={`${n1(f.busy.p95)}/${n1(f.off_turn.p95)} ms`} />
        <Row k="long frames" v={`${f.long} of ${f.n}`} warn={f.long > 0} />
        {!loafSupported() && <Row k="browser long frames" v="unsupported" />}
        {loafSupported() && (
          <Row
            k="worst browser frame"
            v={worstLong ? `${n1(worstLong.durationMs)} ms s${n1(worstLong.scriptMs)} r${n1(worstLong.renderMs)}` : `none of ${g.longs.length}`}
            warn={!!worstLong && worstLong.durationMs > 100}
          />
        )}
      </>
    ),
    turn: f !== null && (
      <>
        <Row k="tick p95/max" v={`${n1(f.tick.p95)}/${n1(f.tick.max)} ms`} warn={f.overran > 0} />
        <Row
          k="phase i/v/c/s p95"
          v={`${n1(phase(f, "interaction"))}/${n1(phase(f, "visible"))}/${n1(phase(f, "content"))}/${n1(phase(f, "cosmetic"))}`}
          warn={phase(f, "interaction") > INTERACTION_WARN_MS}
        />
        <Row k="tasks p95" v={`${n1(f.tasks.p95)} ms`} />
        <Row k="effects/stage" v={`${n1(f.last.effects_ms)}/${n1(f.last.stage_ms)} ms`} warn={f.last.effects_ms > 4} />
        <Row k="worst effect" v={`${n1(f.last.worst_effect_ms)} ms ${EFFECT_NAMES[f.last.worst_effect_name] ?? "-"}`} warn={f.last.worst_effect_ms > 4} />
        <Row k="open turns/busy" v={`${g.open.turns}/${n1(g.open.busy_ms)} ms`} />
      </>
    ),
    frame: f !== null && (
      <>
        <Row k="kinds" v={f.kinds.filter((k) => k.n > 0).map((k) => `${k.name} ${k.n}`).join(" ") || "-"} />
        <Row k="wakes" v={f.wakes.filter((w) => w.n > 0).map((w) => `${w.name} ${w.n}`).join(" ") || "-"} />
        {/* A settled view asks no timer; one asked says how soon. */}
        <Row k="last wake" v={`${valueText("wake_name", f.last.wake_name)}${f.last.wake_in_ms > 0 ? ` in ${n1(f.last.wake_in_ms)} ms` : ""}`} />
      </>
    ),
    gpu: f !== null && (
      <>
        <Row k="gpu p95" v={f.gpu.p95 > 0 ? `${n1(f.gpu.p95)} ms` : "n/a"} warn={f.gpu.p95 > f.last.display_ms} />
        {heaviest(f.gpu_spans, (s) => s.p95, SPAN_ROWS).map((s) => <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.p95)} ms`} />)}
        <Row k="draws/triangles" v={`${drawn(f.last)[0]}/${tris(drawn(f.last)[1])}`} />
        {f.census.slice(0, CENSUS_ROWS).map((r) => <Row key={r.tag} k={`  ${r.tag} draws/tris`} v={`${round(r.draws)}/${tris(r.tris)}`} />)}
        <Row k="world surface" v={valueText("world_px", f.last.world_px)} />
      </>
    ),
    cap: f !== null && <Row k="terrain refused" v={`${round(f.last.cap_terrain_refused)}`} warn={f.last.cap_terrain_refused > 0} />,
    requests: (
      <>
        {f !== null && STREAM_NAMES.map((name, s) =>
          (f.last.waiting[s] > 0 || f.last.out[s] > 0) && <Row key={name} k={`${name} waiting/out`} v={`${f.last.waiting[s]}/${f.last.out[s]}`} warn={f.last.waiting[s] > 12} />
        )}
        {g.tasks.length === 0 && <Note>none in the window</Note>}
        {g.tasks.map((t) => (
          <View key={t.stream}>
            <Row k={`${t.stream}${BOUND[t.stream] ? ` (${BOUND[t.stream]})` : ""} p50/p95`} v={`${n1(t.lat.p50)}/${n1(t.lat.p95)} ms`} />
            <Row k="  ok/aborted/failed" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
            {t.reasons.length > 0 && <Row k="  top failure" v={`${failText(t.reasons[0].fail, t.reasons[0].detail)} ×${t.reasons[0].n}`} warn />}
          </View>
        ))}
      </>
    ),
    tiles: f !== null && (
      <>
        <Row
          k="terrain/place"
          v={`${terrain}/${f.last.tiles_place}${terrain > 0 ? ` ${zoomName(f.last.zoom_tiles.findLastIndex((n) => n > 0))}..${zoomName(f.last.zoom_tiles.findIndex((n) => n > 0))}` : ""}`}
        />
        {/* Since the window began: a settled view moves none of them. */}
        <Row k="terrain admitted/evicted" v={`${g.counters.terrain_admitted}/${g.counters.terrain_evicted}`} />
      </>
    ),
    device: f !== null && (
      <>
        <Row k="blocks" v={`${f.last.blocks} ${valueText("_mb", f.last.blocks_mb)}`} />
        <Row k="depth/layers" v={`${f.last.tex_depth_mb}/${f.last.tex_layers_mb} MB`} />
        {/* The leak bar's figure: flat over a pan loop and across a lost device. */}
        <Row k="gpu objects" v={`${f.last.gpu_objects.reduce((n, o) => n + o, 0)}`} />
        <Row
          k="  by op"
          v={f.last.gpu_objects.map((n, k) => [LIST_NAMES.gpu_objects[k], n] as const).filter(([, n]) => n > 0).map(([name, n]) => `${name} ${n}`).join(" ") || "-"}
        />
        {g.sheet.entries > 0 && <Row k="sheet" v={`${g.sheet.size}px ${g.sheet.entries} ${sheetPct}%`} warn={g.sheet.size > g.sheet.initial && sheetPct < ATLAS_SPARSE_PCT} />}
        <Row k="sheet repacks/uploads" v={`${g.counters.sheet_repacks}/${g.counters.sheet_uploads}`} />
      </>
    ),
    heap: f !== null && (
      <>
        <Row k="used change" v={`${f.heap_used_delta_bytes >= 0 ? "+" : ""}${bytes(f.heap_used_delta_bytes)}`} />
        {/* `live` counts a carved run whole; `slot` is what slots hold. The shares are computed here, not separate lanes. */}
        <Row
          k="used/live of"
          v={`${bytes(f.last.heap_used_bytes)}/${bytes(f.last.heap_live_bytes)} of ${bytes(f.last.heap_range_bytes)} ${pct(f.last.heap_live_bytes, f.last.heap_range_bytes)}%`}
          warn={f.last.heap_live_bytes >= f.last.heap_range_bytes * 0.8}
        />
        <Row
          k="slab runs/slots"
          v={`${bytes(f.last.heap_run_bytes)}/${bytes(f.last.heap_slot_bytes)} ${pct(f.last.heap_slot_bytes, f.last.heap_run_bytes)}%`}
          warn={f.last.heap_run_bytes > MB && f.last.heap_slot_bytes < f.last.heap_run_bytes / 2}
        />
        <Row k="refused/largest free" v={`${f.last.heap_refusals}/${bytes(f.last.heap_largest_free_bytes)}`} warn={f.last.heap_refusals > 0} />
        <Row k="releases" v={`${f.last.heap_releases}`} />
        <Row k="kernel memory" v={bytes(kernel_memory().byteLength)} />
      </>
    ),
    ring: f !== null && <Row k="answers/held peak" v={`${f.last.ring_cq_peak}/${f.last.ring_held_peak}`} warn={f.last.ring_held_peak > 0} />,
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
  text: { fontFamily: textStyle.caption.fontFamily, fontSize: 10.5, lineHeight: 16, fontVariant: ["tabular-nums"], color: alpha(color.white, 0.9) },
  key: { color: alpha(color.white, 0.45) },
  warn: { color: color.profilerHalt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  head: { flexDirection: "row", alignItems: "center", gap: 8, marginTop: 8, marginBottom: 2 },
  // macOS's smallest text style; smaller does not render on a 1x screen.
  headText: { fontSize: 10, color: alpha(color.white, 0.3) },
  rec: { marginTop: 8 },
  recState: { flexDirection: "row", alignItems: "center", gap: 6 },
  recDot: { width: 6, height: 6, borderRadius: 3, backgroundColor: alpha(color.white, 0.25) },
  recDotOn: { backgroundColor: color.profilerHalt },
  headLine: { flex: 1, borderTopWidth: 1, borderTopColor: alpha(color.white, 0.1) },
});
