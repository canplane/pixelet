// @pxl/web/feature/dev/ledger/gauge.tsx

/**
 * The gauge half of the instrument in
 * issue/20260909-cooperative-scheduler-and-task-ledger.md: the kernel's
 * numbers over the last few seconds, painted as a corner panel. It reads and
 * never writes — nothing here feeds back into scheduling.
 *
 * What the report has and this does not is detail, not kind: per-unit costs,
 * retention counts, band reuse, mounts and the window peak stay in the report.
 * Everything here comes from the kernel's summary block, read in place
 * (view.ts) — the same block the report is built from: pacing against
 * the display's measured frame, the tick and its budget, throughput, LOD
 * waits, the GPU's time and what it drew, the device's and the heap's
 * footprint, queue pressure, task latency with its commonest failure, and the
 * LoAF CPU-vs-render split. A value reads as lane.ts says a lane does.
 */

import { round } from "@xpute/core/math/scalar.ts";

import { n1, tris, valueText } from "./lane.ts";
import { BOUND, failText } from "./report.ts";
import type { FrameSummary, Gauge, Spread } from "./view.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import { CORE_MEMORY } from "@/kernel/mem.ts";

/** Tags listed under the triangle total — the heaviest few carry the cost —
 * and spans under the GPU's time, and levels under the terrain's triangles. */
const CENSUS_ROWS = 5;
const SPAN_ROWS = 3;
const LEVEL_ROWS = 3;
/** Share of moving frames allowed to miss a vsync before the row warns. */
const MISSED_WARN_SHARE = 0.1;
/** LoAF's own threshold for a long frame. */
const LONG_FRAME_MS = 50;
/** The interaction phase runs unbudgeted and is meant to be O(1) — camera,
 * frame, puck, companion. Past this it is eating the budget the phases after
 * it receive. */
const INTERACTION_WARN_MS = 2;
/** A pending chunk waiting longer than this is being passed over rather than
 * queued. Ten seconds is above what a full zoom-out's queue takes to drain
 * (~7 s against one scan slot), so an ordinary backlog never trips it, and
 * below the sixteen-second wait that ageing exists to prevent: past it, ageing
 * is not doing its job. */
const STARVED_MS = 10_000;
/** A sheet larger than it starts, and this sparse, is one compaction should
 * already have halved. */
const ATLAS_INITIAL = 1024;
const ATLAS_SPARSE_PCT = 25;

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-white/45">{props.k}</span>
      <span className={props.warn ? "text-amber-400" : "text-white/90"}>{props.v}</span>
    </div>
  );
}

const Rule = () => <div className="my-1 border-t border-white/10" />;

const phase = (f: FrameSummary, name: string): Spread => f.phases.find((p) => p.name === name)?.spread ?? { p50: 0, p95: 0, max: 0 };

/** The panel over one read of the window; the overlay polls and hands it down,
 * since the start/stop buttons above the panel read the same `rec`. */
export function LedgerGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <div className="text-white/45">no window open</div>;

  const { rec, frames: f } = g;
  const atlasPct = round((g.atlas.used_px / (g.atlas.size * g.atlas.size)) * 100);
  const [kept, possible] = [g.counters.faces_kept, g.counters.faces_possible];
  const heaviest = <T,>(rows: T[], by: (r: T) => number, n: number) => [...rows].sort((a, b) => by(b) - by(a)).slice(0, n).filter((r) => by(r) > 0);
  const worstLong = g.longs.reduce<LongFrame | null>((w, l) => (!w || l.durationMs > w.durationMs ? l : w), null);

  return (
    <>
      <Row
        k={rec.recording ? "REC" : "stopped"}
        v={`${(rec.elapsed_ms / 1000).toFixed(1)}s ${rec.frames}f/${rec.tasks}t${rec.truncated ? " TRUNC" : ""}`}
        warn={rec.recording || rec.truncated}
      />
      <Rule />

      {f === null && <div className="text-white/45">no frames sampled yet</div>}
      {f !== null && (
        <>
          <Row k="dt p50/p95 moving" v={`${n1(f.dt.p50)}/${n1(f.dt.p95)}ms`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="dt max" v={`${n1(f.dt.max)}ms`} warn={f.dt.max > LONG_FRAME_MS} />
          <Row k="missed/moving" v={`${f.missed}/${f.moving}`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="task/offthd p95" v={`${n1(f.task.p95)}/${n1(f.offthread.p95)}ms`} warn={f.offthread.p95 > f.last.frame_ms} />
          <Row k="gpu p95" v={f.gpu.p95 > 0 ? `${n1(f.gpu.p95)}ms` : "n/a"} warn={f.gpu.p95 > f.last.frame_ms} />
          {heaviest(f.gpu_spans, (s) => s.spread.p95, SPAN_ROWS).map((s) => <Row key={s.name} k={`  ${s.name} p95`} v={`${n1(s.spread.p95)}ms`} />)}
          <Row k="sched p95/max" v={`${n1(f.sched.p95)}/${n1(f.sched.max)}ms`} warn={f.overran > 0} />
          <Row k="budget/frame" v={`${n1(f.last.budget_ms)}/${n1(f.last.frame_ms)}ms${f.last.settled ? " still" : ""}`} />
          <Row k="tick p95" v={n1(f.tick.p95)} />
          <Row
            k="phase p95 i/v/c/s"
            v={`${n1(phase(f, "interaction").p95)}/${n1(phase(f, "visible").p95)}/${n1(phase(f, "content").p95)}/${n1(phase(f, "cosmetic").p95)}`}
            warn={phase(f, "interaction").p95 > INTERACTION_WARN_MS}
          />
          <Row k="bake/scan per s busy" v={`${round(f.backlogPerS[1])}/${round(f.backlogPerS[0])}`} />
          <Rule />
          <Row k="lod c/f/p" v={`${f.last.lod_chunks}/${f.last.lod_in_frustum}/${f.last.lod_pending}`} warn={f.last.lod_pending > 32} />
          <Row
            k="wait rel/vis"
            v={`${f.last.lod_offscreen_pending} ${round(f.last.lod_oldest_release_ms)}/${round(f.last.lod_oldest_visible_ms)}ms`}
            warn={Math.max(f.last.lod_oldest_release_ms, f.last.lod_oldest_visible_ms) > STARVED_MS}
          />
          <Rule />
          <Row k="draws" v={String(f.last.draw_calls)} />
          <Row k="tris" v={tris(f.last.triangles)} />
          {f.census.slice(0, CENSUS_ROWS).map((r) => <Row key={r.tag} k={`  ${r.tag} d/tris`} v={`${round(r.draws)}/${tris(r.tris)}`} />)}
          {heaviest(f.level_tris.map((t, k) => ({ k, t })), (r) => r.t, LEVEL_ROWS).map((r) => <Row key={r.k} k={`  L${r.k} tris`} v={tris(r.t)} />)}
          <Row k="buffers / arena" v={`${f.last.buffers} ${valueText("_mb", f.last.buffer_mb)} / ${valueText("_mb", f.last.arena_mb)}`} />
          <Row k="fill Mpx" v={(f.last.px / 1e6).toFixed(1)} warn={f.last.px / 1e6 > 12} />
          <Row k="targets MB" v={`d${f.last.tex_depth_mb} m${f.last.tex_msaa_mb} s${f.last.tex_shadow_mb} l${f.last.tex_layers_mb}`} />
          <Rule />
          <Row k="heap Δ" v={`${f.used_delta_mb >= 0 ? "+" : ""}${f.used_delta_mb}MB`} />
          {
            /* Facts and no derived word: how far the allocator has ever reached
              into its range, what is on loan now, the range, what it refused
              and the largest block it has free. All come from the kernel's own
              seam to it (host.rs) — the shell never decodes the allocator's
              bytes, which is what let the last probe read a deleted
              structure's row for two commits without saying so. */
          }
          <Row k="  used/live of" v={`${f.last.heap_used_mb}/${f.last.heap_live_mb} of ${f.last.heap_range_mb}MB`} warn={f.last.heap_used_mb >= f.last.heap_range_mb} />
          <Row k="  refused / free" v={`${f.last.heap_refusals} / ${n1(f.last.heap_largest_free_mb)}MB`} warn={f.last.heap_refusals > 0} />
          <Row k="rings cq/spill" v={`${f.last.ring_cq_peak}/${f.last.ring_spill_peak}`} warn={f.last.ring_spill_peak > 0} />
          {
            /* Pages are records in `.fixed`, a fixed number of them a level,
              so the count is what the window holds and not a cost in bytes. */
          }
          <Row k="pages" v={`${f.last.pages}${f.last.pages > 0 ? ` in L${f.last.level_pages.findIndex((n) => n > 0)}..L${f.last.level_pages.findLastIndex((n) => n > 0)}` : ""}`} />
          {
            /* The shell's own number: the reservation is the central scheduler's,
              and the kernel is handed a range rather than told how big the page
              thinks it is. */
          }
          <Row k="  core MB" v={`${round(CORE_MEMORY.buffer.byteLength / 1048576)}MB`} />
          <Row k="faces kept" v={`${possible > 0 ? round((kept / possible) * 100) : 0}%`} />
          {g.atlas.entries > 0 && <Row k="atlas" v={`${g.atlas.size}px ${g.atlas.entries} ${atlasPct}%`} warn={g.atlas.size > ATLAS_INITIAL && atlasPct < ATLAS_SPARSE_PCT} />}
          <Row k="queue t/p" v={`${f.last.q_terrain}/${f.last.q_poi}`} warn={f.last.q_terrain + f.last.q_poi > 12} />
        </>
      )}

      {g.tasks.length > 0 && <Rule />}
      {g.tasks.map((t) => (
        <div key={t.category}>
          <Row k={`${t.category}${BOUND[t.category] ? ` (${BOUND[t.category].bound})` : ""} p50/p95`} v={`${n1(t.lat.p50)}/${n1(t.lat.p95)}ms`} />
          <Row k="  ok/ab/err" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
          {t.reasons.length > 0 && <Row k="  top err" v={`${failText(t.reasons[0].fail, t.reasons[0].status)} x${t.reasons[0].n}`} warn />}
        </div>
      ))}

      <Rule />
      {!loafSupported() && <Row k="LoAF" v="unsupported" />}
      {loafSupported() && (
        <Row
          k="LoAF worst"
          v={worstLong ? `${n1(worstLong.durationMs)}ms s${n1(worstLong.scriptMs)}/r${n1(worstLong.renderMs)}` : `0 (${g.longs.length})`}
          warn={!!worstLong && worstLong.durationMs > 100}
        />
      )}
    </>
  );
}
