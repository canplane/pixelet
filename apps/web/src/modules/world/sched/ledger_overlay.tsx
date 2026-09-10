// @pxl/web/modules/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for sched/ledger.ts — the gauge half of the instrument in
 * discussions/20260909-cooperative-scheduler-and-task-ledger.md. Polls the
 * ledger rings on a timer and paints a corner panel; never feeds anything
 * back into scheduling. Gated by `import.meta.env.DEV` and debug_store's
 * showLedgerOverlay — off by default. The text half (live console stream,
 * pasteable report) is log.ts, toggled independently.
 *
 * This component also owns the dev-only lifecycle the two surfaces share:
 * installing the LoAF observer and hanging `__pxl_ledger_trace` /
 * `__pxl_ledger_log` on `window`, both regardless of whether the panel
 * itself is showing.
 *
 * Deliberately not the whole step-1 overlay: no stacked flame strip, no
 * per-frame CPU-*category* rollup (the ledger carries chunk_stage's three
 * phases, not per-task-category time). What's here is the decision-grade
 * minimum — frame pacing vs the 16.7ms line, the CPU split behind it, the
 * GPU ask, retained footprint, queue pressure, task latency, and the LoAF
 * CPU-vs-render split.
 */

import { useEffect, useState } from "react";

import { useDebugStore } from "@/app/settings/debug_store.ts";
import { occlusion_stats } from "@/render/layer/pipeline/chunk_mesher.ts";
import { poi_place_stats } from "@/modules/world/poi/poi_layer.tsx";
import { poi_index_stats } from "@/modules/world/poi/poi_index.ts";
import { category_info } from "./categories.ts";
import {
  type FrameSample,
  ledger_frames_snapshot,
  ledger_install_longtask_observer,
  ledger_install_observers,
  ledger_long_frames_snapshot,
  ledger_recording_start,
  ledger_recording_state,
  ledger_recording_stop,
  ledger_snapshot,
  type TaskRecord,
} from "./ledger.ts";
import { ledger_report_to_console, ledger_set_logging, pct } from "./log.ts";
import { ledger_trace_download } from "./trace.ts";

const FRAME_BUDGET_MS = 1000 / 60;
const POLL_MS = 500;
// one decoded terrain page, measured live (terrain/config.ts: ~4.03MB) — the
// multiplier on tile_pages, same idea as px × fx_passes.
const TILE_DECODED_MB = 4;

function r1(n: number): string {
  return n.toFixed(1);
}

function summarize_frames(frames: readonly FrameSample[]) {
  if (frames.length === 0) return null;
  const dt = frames.map((f) => f.dt_ms).sort((a, b) => a - b);
  const sched = frames.map((f) => f.sched_ms).sort((a, b) => a - b);
  const over = frames.filter((f) => f.dt_ms > FRAME_BUDGET_MS + 1).length;
  const last = frames[frames.length - 1];
  const heap_delta = last.heap_mb - frames[0].heap_mb;
  const p95 = (pick: (f: FrameSample) => number) => pct(frames.map(pick).sort((a, b) => a - b), 0.95);
  return {
    n: frames.length,
    dt_p50: pct(dt, 0.5),
    dt_p95: pct(dt, 0.95),
    dt_max: dt[dt.length - 1],
    over,
    sched_p95: pct(sched, 0.95),
    sched_max: sched[sched.length - 1],
    task_p95: p95((f) => f.task_ms),
    offthread_p95: p95((f) => Math.max(0, f.dt_ms - f.task_ms)),
    gpu_p95: p95((f) => f.gpu_ms),
    recheck_p95: p95((f) => f.cpu.recheck_ms),
    commit_p95: p95((f) => f.cpu.commit_ms),
    render_p95: p95((f) => f.cpu.render_ms),
    last,
    heap_delta,
  };
}

function summarize_tasks(tasks: readonly TaskRecord[]) {
  const by = new Map<string, { lat: number[]; ok: number; aborted: number; error: number }>();
  for (const rec of tasks) {
    let g = by.get(rec.category);
    if (!g) by.set(rec.category, g = { lat: [], ok: 0, aborted: 0, error: 0 });
    if (rec.outcome === "ok") g.ok++;
    else if (rec.outcome === "aborted") g.aborted++;
    else g.error++;
    if (rec.outcome === "ok") g.lat.push(rec.end_ms - rec.enqueue_ms);
  }
  return [...by.entries()].map(([cat, g]) => {
    const lat = g.lat.sort((a, b) => a - b);
    return { cat, n: g.ok + g.aborted + g.error, p50: pct(lat, 0.5), p95: pct(lat, 0.95), ok: g.ok, aborted: g.aborted, error: g.error };
  });
}

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <div className="flex justify-between gap-3">
      <span className="text-white/45">{props.k}</span>
      <span className={props.warn ? "text-amber-400" : "text-white/90"}>{props.v}</span>
    </div>
  );
}

type LedgerGlobals = typeof globalThis & {
  __pxl_ledger_trace?: () => string;
  __pxl_ledger_log?: () => Promise<string>;
};

export function LedgerOverlay() {
  const show = useDebugStore((s) => s.showLedgerOverlay);
  const log = useDebugStore((s) => s.logLedger);
  const [, force] = useState(0);

  // Shared dev lifecycle — runs whether or not the panel is showing, so the
  // long-frame ring is already warm when it is opened and the console
  // handles are always reachable.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    ledger_install_observers();
    ledger_install_longtask_observer();
    const w = globalThis as LedgerGlobals;
    w.__pxl_ledger_trace = ledger_trace_download;
    w.__pxl_ledger_log = ledger_report_to_console;
    return () => {
      delete w.__pxl_ledger_trace;
      delete w.__pxl_ledger_log;
    };
  }, []);

  // Live console stream, independent of the panel.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    ledger_set_logging(log);
    return () => ledger_set_logging(false);
  }, [log]);

  // Panel repaint only while the panel is up.
  useEffect(() => {
    if (!import.meta.env.DEV || !show) return;
    const id = setInterval(() => force((n) => n + 1), POLL_MS);
    return () => clearInterval(id);
  }, [show]);

  if (!import.meta.env.DEV || !show) return null;

  const f = summarize_frames(ledger_frames_snapshot());
  const tasks = summarize_tasks(ledger_snapshot());
  const longs = ledger_long_frames_snapshot();
  const rec = ledger_recording_state();
  const worst_long = longs.reduce<(typeof longs)[number] | null>((w, l) => (!w || l.duration_ms > w.duration_ms ? l : w), null);
  const loaf_supported = typeof PerformanceObserver !== "undefined" &&
    !!PerformanceObserver.supportedEntryTypes?.includes("long-animation-frames");

  return (
    <div className="pointer-events-none fixed left-2 top-[84px] z-50 w-[228px] rounded-lg bg-black/72 p-2.5 font-mono text-[10.5px] leading-[1.5] text-white/90 backdrop-blur-sm">
      <div className="mb-1 font-semibold tracking-wide text-white/55">sched/ledger</div>

      {!f && <div className="text-white/45">no frames sampled yet</div>}
      {f && (
        <>
          <Row k="dt p50/p95" v={`${r1(f.dt_p50)}/${r1(f.dt_p95)}ms`} warn={f.dt_p95 > FRAME_BUDGET_MS + 1} />
          <Row k="dt max" v={`${r1(f.dt_max)}ms`} warn={f.dt_max > FRAME_BUDGET_MS * 2} />
          <Row k="over budget" v={`${f.over}/${f.n}`} warn={f.over > f.n * 0.1} />
          <Row
            k="task/offthd p95"
            v={`${r1(f.task_p95)}/${r1(f.offthread_p95)}ms`}
            warn={f.offthread_p95 > FRAME_BUDGET_MS}
          />
          <Row k="gpu p95" v={f.gpu_p95 > 0 ? `${r1(f.gpu_p95)}ms` : "n/a"} warn={f.gpu_p95 > FRAME_BUDGET_MS} />
          <Row k="sched p95/max" v={`${r1(f.sched_p95)}/${r1(f.sched_max)}ms`} warn={f.sched_max > FRAME_BUDGET_MS} />
          <Row
            k="rck/cmt/rnd p95"
            v={`${r1(f.recheck_p95)}/${r1(f.commit_p95)}/${r1(f.render_p95)}`}
            warn={Math.max(f.recheck_p95, f.commit_p95, f.render_p95) > FRAME_BUDGET_MS}
          />
          <div className="my-1 border-t border-white/10" />
          <Row
            k="lod c/f/p"
            v={`${f.last.lod.chunks}/${f.last.lod.in_frustum}/${f.last.lod.pending}`}
            warn={f.last.lod.pending > 32}
          />
          <div className="my-1 border-t border-white/10" />
          <Row k="draws" v={String(f.last.draw_calls)} />
          <Row k="tris" v={`${(f.last.triangles / 1000).toFixed(0)}k`} />
          <Row k="textures" v={String(f.last.textures)} />
          <Row
            k="fill Mpx×n"
            v={`${(f.last.px / 1e6).toFixed(1)}×${f.last.fx_passes}`}
            warn={(f.last.px * f.last.fx_passes) / 1e6 > 12}
          />
          <div className="my-1 border-t border-white/10" />
          <Row k="heap" v={f.last.heap_mb ? `${f.last.heap_mb}MB Δ${f.heap_delta >= 0 ? "+" : ""}${f.heap_delta}` : "n/a"} />
          <Row
            k="tiles ~mb / poi"
            v={`${f.last.tile_pages} ~${f.last.tile_pages * TILE_DECODED_MB} / ${f.last.poi_count}`}
          />
          <Row
            k="pool live/idle hit"
            v={`${f.last.pool.live_mb}/${f.last.pool.idle_mb}MB ${f.last.pool.hit_pct}%`}
            warn={f.last.pool.hit_pct < 80}
          />
          <Row
            k="faces kept"
            v={`${occlusion_stats.faces_possible > 0 ? Math.round(occlusion_stats.faces_kept / occlusion_stats.faces_possible * 100) : 0}%`}
          />
          <Row
            k="poi place ms"
            v={`${r1(poi_place_stats.ms)} max${r1(poi_place_stats.max_ms)} hit${poi_place_stats.hits}`}
            warn={poi_place_stats.ms > 8}
          />
          <Row
            k="  cand/kept idx"
            v={`${poi_place_stats.candidates}/${poi_place_stats.kept} ${poi_index_stats().indexed}${poi_index_stats().dropped > 0 ? ` -${poi_index_stats().dropped}` : ""}`}
            warn={poi_index_stats().dropped > 0}
          />
          <Row k="scan/bake" v={`${f.last.work.scans}/${f.last.work.bakes}`} warn={f.last.work.scans > 0} />
          <Row k="queue t/p" v={`${f.last.q_terrain}/${f.last.q_poi}`} warn={f.last.q_terrain + f.last.q_poi > 12} />
        </>
      )}

      {tasks.length > 0 && <div className="my-1 border-t border-white/10" />}
      {tasks.map((t) => {
        const bound = category_info(t.cat)?.bound;
        return (
          <div key={t.cat}>
            <Row
              k={`${t.cat}${bound ? ` (${bound})` : ""} p50/p95`}
              v={`${r1(t.p50)}/${r1(t.p95)}ms`}
            />
            <Row k="  ok/ab/err" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
          </div>
        );
      })}

      <div className="my-1 border-t border-white/10" />
      {!loaf_supported && <Row k="LoAF" v="unsupported" />}
      {loaf_supported && (
        <Row
          k="LoAF worst"
          v={worst_long ? `${r1(worst_long.duration_ms)}ms s${r1(worst_long.script_ms)}/r${r1(worst_long.render_ms)}` : `0 (${longs.length})`}
          warn={!!worst_long && worst_long.duration_ms > 100}
        />
      )}

      <div className="my-1 border-t border-white/10" />
      <Row
        k={rec.recording ? "REC" : "stopped"}
        v={`${(rec.elapsed_ms / 1000).toFixed(1)}s ${rec.frames}f/${rec.tasks}t${rec.truncated ? " TRUNC" : ""}`}
        warn={rec.recording || rec.truncated}
      />

      {
        /* start and stop are both always present rather than one toggle.
          Recording auto-begins at load so a cold-load stall is never missed,
          which means a toggle would only ever show "stop" until you had
          already stopped once — and then "start" is the button you cannot
          find when you want to begin a clean window. */
      }
      <div className="mt-1.5 flex gap-1">
        <button
          type="button"
          onClick={() => {
            ledger_recording_start();
            force((n) => n + 1);
          }}
          title="clear everything and begin a fresh window"
          className={`pointer-events-auto flex-1 rounded border py-1 transition-colors ${
            rec.recording ? "border-white/15 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white/90" : "border-emerald-400/40 bg-emerald-400/10 text-emerald-300 hover:bg-emerald-400/20"
          }`}
        >
          start
        </button>
        <button
          type="button"
          onClick={() => {
            ledger_recording_stop();
            force((n) => n + 1);
          }}
          disabled={!rec.recording}
          title="freeze the window so exports are stable while you read them"
          className={`pointer-events-auto flex-1 rounded border py-1 transition-colors ${
            rec.recording ? "border-amber-400/40 bg-amber-400/10 text-amber-300 hover:bg-amber-400/20" : "border-white/10 bg-white/5 text-white/25"
          }`}
        >
          stop
        </button>
      </div>
      <div className="mt-1 flex gap-1">
        <button
          type="button"
          onClick={() => void ledger_report_to_console()}
          title="print + copy a text report"
          className="pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90"
        >
          report
        </button>
        <button
          type="button"
          onClick={() => ledger_trace_download()}
          title="save a Chrome-trace file for Perfetto"
          className="pointer-events-auto flex-1 rounded border border-white/15 bg-white/5 py-1 text-white/60 transition-colors hover:bg-white/10 hover:text-white/90"
        >
          trace
        </button>
      </div>
    </div>
  );
}
