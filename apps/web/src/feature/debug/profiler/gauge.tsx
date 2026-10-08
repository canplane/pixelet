// @pxl/web/feature/debug/profiler/gauge.tsx

/**
 * The kernel's numbers over the last few seconds as a corner panel, read in
 * place from the summary block the report is built from. Read-only.
 */

import { round } from "@xpute/core/math/scalar.ts";
import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { EFFECT_NAMES } from "@/kernel/turn/step.spec.ts";
import { n1, pct, STARVED_MS, tris, valueText, zoomName, zoomsText } from "./lane.ts";
import { BOUND, failText } from "./report.ts";
import type { FrameSummary, Gauge, Spread } from "./view.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import { kernel_memory } from "@/kernel/shell.ts";
import { alpha, color, font } from "@/shared/ui/theme.ts";

const CENSUS_ROWS = 5;
const SPAN_ROWS = 3;
const ZOOM_ROWS = 3;
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

/** One of the panel's two groups, above its sections. */
export const Title = ({ children, end, first }: { children: string; end?: ReactNode; first?: boolean }) => (
  <View style={[gauge.title, first && gauge.titleFirst]}>
    <Text style={[gauge.text, gauge.titleText]}>{children}</Text>
    {end}
  </View>
);

const Head = ({ children, end }: { children: string; end?: ReactNode }) => (
  <View style={gauge.head}>
    <Text style={[gauge.text, gauge.headText]}>{children}</Text>
    <View style={gauge.headLine} />
    {end}
  </View>
);

const deg = (rad: number) => (rad * 180) / Math.PI;
const or = (x: number) => (Number.isNaN(x) ? "-" : n1(x));
const geo = (lat: number, lon: number) => (Number.isNaN(lat) ? "-" : `${lat.toFixed(5)}°, ${lon.toFixed(5)}°`);
const metres = (m: number) => (Number.isNaN(m) ? "-" : `${n1(m)} m`);

const phase = (f: FrameSummary, name: string): Spread => f.phases.find((p) => p.name === name)?.spread ?? { p50: 0, p95: 0, max: 0 };

/** Live: where the camera and the pointer are now, not over the window. */
export function ProfilerView({ v }: { v: Gauge["view"] }) {
  const hovering = !Number.isNaN(v.hover_x);
  return (
    <>
      <Head>camera</Head>
      <Row k="distance" v={`${n1(v.dist)} u`} />
      <Row k="polar/azimuth" v={`${n1(deg(v.polar))}°/${n1(deg(v.azimuth))}°`} />
      <Row k="τ" v={or(v.tau)} />
      <Row k="center" v={geo(v.center_lat, v.center_lon)} />
      <Row k="center altitude" v={metres(v.center_z)} />
      <Head>pointer</Head>
      <Row k="hover (u)" v={hovering ? `${Math.floor(v.hover_x)}, ${Math.floor(v.hover_y)}, ${Math.floor(v.hover_z)}` : "-"} />
      <Row k="hover" v={geo(v.hover_lat, v.hover_lon)} />
      <Row k="hover altitude" v={metres(v.hover_alt)} />
      <Row k="cells wanted/drawn" v={hovering ? `${zoomName(v.cell_want)}/${zoomName(v.cell_drawn)}` : "-"} warn={v.cell_drawn < v.cell_want} />
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
          <Text style={[gauge.text, rec.recording && gauge.warn]}>{rec.recording ? "recording" : "stopped"}</Text>
        </View>
        <Text style={gauge.text}>{`${(rec.elapsed_ms / 1000).toFixed(1)} s`}</Text>
      </View>
      <Row k="frames/tasks" v={`${rec.frames}/${rec.tasks}${rec.truncated ? " truncated" : ""}`} warn={rec.truncated} />
    </View>
  );
}

export function ProfilerGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <Note>no window open</Note>;

  const f = g.frames;
  const sheetPct = pct(g.sheet.used_px, g.sheet.size * g.sheet.size);
  const [kept, possible] = [g.counters.faces_kept, g.counters.faces_possible];
  const heaviest = <T,>(rows: T[], by: (r: T) => number, n: number) => [...rows].sort((a, b) => by(b) - by(a)).slice(0, n).filter((r) => by(r) > 0);
  const worstLong = g.longs.reduce<LongFrame | null>((w, l) => (!w || l.durationMs > w.durationMs ? l : w), null);

  return (
    <>
      {f === null && <Note>no frames sampled yet</Note>}
      {f !== null && (
        <>
          <Head>frame/gpu</Head>
          <Row k="frame p50/p95" v={`${n1(f.dt.p50)}/${n1(f.dt.p95)} ms`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="frame max" v={`${n1(f.dt.max)} ms`} warn={f.dt.max > LONG_FRAME_MS} />
          <Row k="missed/moving" v={`${f.missed}/${f.moving}`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="budget/frame" v={`${n1(f.last.budget_ms)}/${n1(f.last.frame_ms)} ms${f.last.settled ? " still" : ""}`} />
          <Row k="task/off-thread p95" v={`${n1(f.task.p95)}/${n1(f.offthread.p95)} ms`} warn={f.offthread.p95 > f.last.frame_ms} />
          <Row k="tick p95/max" v={`${n1(f.sched.p95)}/${n1(f.sched.max)} ms`} warn={f.overran > 0} />
          <Row
            k="phase i/v/c/s p95"
            v={`${n1(phase(f, "interaction").p95)}/${n1(phase(f, "visible").p95)}/${n1(phase(f, "content").p95)}/${n1(phase(f, "cosmetic").p95)}`}
            warn={phase(f, "interaction").p95 > INTERACTION_WARN_MS}
          />
          <Row k="chunks p95" v={`${n1(f.chunks.p95)} ms`} />
          {
            /* `task` less the three parts is the unmeasured region, where a fold
                          once took 70 ms of a turn. */
          }
          <Row k="effects/stage" v={`${n1(f.last.turn_effects_ms)}/${n1(f.last.turn_stage_ms)} ms`} warn={f.last.turn_effects_ms > 4} />
          <Row k="worst effect" v={`${n1(f.last.turn_step_ms)} ms ${EFFECT_NAMES[f.last.turn_step_name] ?? "-"}`} warn={f.last.turn_step_ms > 4} />
          <Row k="gpu p95" v={f.gpu.p95 > 0 ? `${n1(f.gpu.p95)} ms` : "n/a"} warn={f.gpu.p95 > f.last.frame_ms} />
          {heaviest(f.gpu_spans, (s) => s.spread.p95, SPAN_ROWS).map((s) => <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.spread.p95)} ms`} />)}
          {!loafSupported() && <Row k="long frames" v="unsupported" />}
          {loafSupported() && (
            <Row
              k="worst long frame"
              v={worstLong ? `${n1(worstLong.durationMs)} ms s${n1(worstLong.scriptMs)} r${n1(worstLong.renderMs)}` : `none of ${g.longs.length}`}
              warn={!!worstLong && worstLong.durationMs > 100}
            />
          )}

          <Head>draw/lod</Head>
          <Row k="chunks/in view/pending" v={`${f.last.lod_chunks}/${f.last.lod_in_frustum}/${f.last.lod_pending}`} warn={f.last.lod_pending > 32} />
          <Row
            k="off-view · oldest off/in"
            v={`${f.last.lod_offscreen_pending} · ${round(f.last.lod_oldest_release_ms)}/${round(f.last.lod_oldest_visible_ms)} ms`}
            warn={Math.max(f.last.lod_oldest_release_ms, f.last.lod_oldest_visible_ms) > STARVED_MS}
          />
          <Row
            k="moves hidden/uncovered/unready"
            v={`${round(f.last.stale_hidden)}/${round(f.last.stale_uncovered)}/${round(f.last.stale_unready)}`}
          />
          <Row
            k="slots waiting/chained"
            v={`${round(f.last.slot_waits)}/${round(f.last.slot_chained)} ${zoomsText(f.last.slot_wait_zooms)}`}
            warn={f.last.slot_waits > 0}
          />
          {
            /* `full`: the mount pass stopped on a full table rather than its own
                          cap, i.e. a number is too small. */
          }
          <Row
            k="room commit/mount/page"
            v={`${round(f.last.cap_commit_left)}/${round(f.last.cap_mount_left)}${f.last.cap_mount_full ? " full" : ""}/${round(f.last.cap_record_refused)}`}
            warn={f.last.cap_mount_full > 0 || f.last.cap_record_refused > 0}
          />
          <Row k="draws/triangles" v={`${f.last.draw_calls}/${tris(f.last.triangles)}`} />
          {f.census.slice(0, CENSUS_ROWS).map((r) => <Row key={r.tag} k={`  ${r.tag} draws/tris`} v={`${round(r.draws)}/${tris(r.tris)}`} />)}
          {heaviest(f.zoom_tris.map((t, k) => ({ k, t })), (r) => r.t, ZOOM_ROWS).map((r) => <Row key={r.k} k={`  ${zoomName(r.k)} triangles`} v={tris(r.t)} />)}
          <Row k="fill" v={valueText("px", f.last.px)} warn={f.last.px > 12e6} />
          <Row k="bakes/runs per s" v={`${round(f.backlogPerS[1])}/${round(f.backlogPerS[0])}`} />
          <Row k="faces kept" v={`${pct(kept, possible)}%`} />
        </>
      )}

      <Head>requests</Head>
      {f !== null && <Row k="queued terrain/place" v={`${f.last.q_terrain}/${f.last.q_place}`} warn={f.last.q_terrain + f.last.q_place > 12} />}
      {f !== null && <Row k="ring answers/held peak" v={`${f.last.ring_cq_peak}/${f.last.ring_held_peak}`} warn={f.last.ring_held_peak > 0} />}
      {g.tasks.length === 0 && <Note>none in the window</Note>}
      {g.tasks.map((t) => (
        <View key={t.category}>
          <Row k={`${t.category}${BOUND[t.category] ? ` (${BOUND[t.category]})` : ""} p50/p95`} v={`${n1(t.lat.p50)}/${n1(t.lat.p95)} ms`} />
          <Row k="  ok/aborted/failed" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
          {t.reasons.length > 0 && <Row k="  top failure" v={`${failText(t.reasons[0].fail, t.reasons[0].detail)} ×${t.reasons[0].n}`} warn />}
        </View>
      ))}

      {f !== null && (
        <>
          <Head>memory</Head>
          <Row k="heap change" v={`${f.used_delta_mb >= 0 ? "+" : ""}${f.used_delta_mb} MB`} />
          {
            /* `live` counts a carved run whole; `slot` is what is held. The shares
                          are computed here, not separate lanes. */
          }
          <Row
            k="heap used/live of"
            v={`${f.last.heap_used_mb}/${f.last.heap_live_mb} of ${f.last.heap_range_mb} MB ${pct(f.last.heap_live_mb, f.last.heap_range_mb)}%`}
            warn={f.last.heap_live_mb >= f.last.heap_range_mb * 0.8}
          />
          <Row
            k="slab runs/slots"
            v={`${n1(f.last.heap_run_mb)}/${n1(f.last.heap_slot_mb)} MB ${pct(f.last.heap_slot_mb, f.last.heap_run_mb)}%`}
            warn={f.last.heap_run_mb > 1 && f.last.heap_slot_mb < f.last.heap_run_mb / 2}
          />
          <Row k="refused/largest free" v={`${f.last.heap_refusals}/${n1(f.last.heap_largest_free_mb)} MB`} warn={f.last.heap_refusals > 0} />
          <Row
            k="tiles"
            v={`${f.last.tiles_total}${f.last.tiles_total > 0 ? ` ${zoomName(f.last.zoom_tiles.findLastIndex((n) => n > 0))}..${zoomName(f.last.zoom_tiles.findIndex((n) => n > 0))}` : ""}`}
          />
          <Row k="kernel memory" v={`${round(kernel_memory().byteLength / 1048576)} MB`} />
          <Row k="buffers/arena" v={`${f.last.buffers} ${valueText("_mb", f.last.buffer_mb)}/${valueText("_mb", f.last.arena_mb)}`} />
          <Row k="depth/shadow/layers" v={`${f.last.tex_depth_mb}/${f.last.tex_shadow_mb}/${f.last.tex_layers_mb} MB`} />
          {g.sheet.entries > 0 && <Row k="sheet" v={`${g.sheet.size}px ${g.sheet.entries} ${sheetPct}%`} warn={g.sheet.size > g.sheet.initial && sheetPct < ATLAS_SPARSE_PCT} />}
        </>
      )}
    </>
  );
}

export const gauge = StyleSheet.create({
  text: { fontFamily: font.mono, fontSize: 10.5, lineHeight: 16, color: alpha(color.white, 0.9) },
  key: { color: alpha(color.white, 0.45) },
  warn: { color: color.profilerHalt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  head: { flexDirection: "row", alignItems: "center", gap: 8, marginTop: 8, marginBottom: 2 },
  // macOS's smallest text style; smaller does not render on a 1x screen.
  headText: { fontSize: 10, letterSpacing: 0.4, textTransform: "uppercase", color: alpha(color.white, 0.3) },
  rec: { marginTop: 8 },
  recState: { flexDirection: "row", alignItems: "center", gap: 6 },
  recDot: { width: 6, height: 6, borderRadius: 3, backgroundColor: alpha(color.white, 0.25) },
  recDotOn: { backgroundColor: color.profilerHalt },
  title: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", marginTop: 10, paddingBottom: 3, borderBottomWidth: 1, borderBottomColor: alpha(color.white, 0.2) },
  titleFirst: { marginTop: 0 },
  titleText: { fontSize: 11, fontWeight: "600", letterSpacing: 0.6, textTransform: "uppercase", color: alpha(color.white, 0.75) },
  headLine: { flex: 1, borderTopWidth: 1, borderTopColor: alpha(color.white, 0.1) },
});
