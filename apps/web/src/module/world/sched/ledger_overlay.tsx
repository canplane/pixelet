// @pxl/web/module/world/sched/ledger_overlay.tsx

/**
 * Dev overlay for sched/ledger.ts — the gauge half of the instrument in
 * issue/20260909-cooperative-scheduler-and-task-ledger.md. Polls the
 * ledger rings on a timer and paints a corner panel; never feeds anything
 * back into scheduling. Gated by `import.meta.env.DEV` and the `show_ledger`
 * setting — off by default. The text half is log.ts: the pasteable report
 * and the live console stream are both buttons on the panel, the stream a
 * toggle that outlives the panel (it is the ledger's sink, not the panel's
 * state), so closing the panel does not silence a stream being watched.
 *
 * This component also owns the dev-only lifecycle the surfaces share:
 * installing the LoAF observer and hanging `__pxl_ledger_trace` /
 * `__pxl_ledger_log` on `window`, both regardless of whether the panel
 * itself is showing.
 *
 * What the report has and this does not is detail, not kind: per-unit
 * costs, retention counts, band reuse, mounts and the window peak stay in
 * the report. Everything here is read through summary.ts, the same numbers
 * the report prints — pacing against the display's measured frame, the tick
 * and its budget, throughput, LOD waits, the GPU ask, retained footprint,
 * queue pressure, task latency with its commonest failure, and the LoAF
 * CPU-vs-render split.
 */

import { useEffect, useState } from "react";

import { useSettingsStore } from "@/app/setting/store.ts";
import { poi_place_stats } from "@/module/world/poi/poi_layer.tsx";
import { poi_index_stats } from "@/module/world/poi/poi_index.ts";
import { badge_atlas } from "@/module/world/poi/poi_badge.ts";
import { CORE_MEMORY, core_pool_stat } from "@/render/layer/pipeline/core_pool.ts";
import { category_info } from "./category.ts";
import {
  ledger_frames_since,
  ledger_install_longtask_observer,
  ledger_install_observers,
  ledger_long_frames_since,
  ledger_recording_state,
  ledger_recording_stop,
  ledger_tasks_since,
} from "./ledger.ts";
import { ledger_logging, ledger_report_to_console, ledger_set_logging, ledger_window_start, window_counters } from "./log.ts";
import { STARVED_MS, summarize_frames, summarize_tasks, TILE_DECODED_MB, tris_text } from "./summary.ts";
import { ledger_trace_download } from "./trace.ts";
import { alloc_probe_install, alloc_probe_report, alloc_probe_reset } from "./alloc_probe.ts";

const POLL_MS = 500;
/** Tags listed under the triangle total — the heaviest few carry the cost. */
const CENSUS_ROWS = 5;
/** The panel reads the recent past, not the whole recording — frames, tasks
 * and long frames alike. Reducing every frame since load on each repaint
 * grew into a 60 ms task every half second two minutes in — the panel was
 * the stall it was showing — and the task and long-frame rows kept doing
 * the same after the frame rows stopped. The report still covers the whole
 * window. */
const LIVE_WINDOW_MS = 10_000;
/** Share of moving frames allowed to miss a vsync before the row warns. */
const MISSED_WARN_SHARE = 0.1;
/** LoAF's own threshold for a long frame. */
const LONG_FRAME_MS = 50;
/** A placement job spanning this many frames is one the content phase is
 * starving — a quarter second at 60 Hz, longer than a settle. */
const PLACE_FRAMES_WARN = 15;

function r1(n: number): string {
  return n.toFixed(1);
}

/** The interaction phase runs unbudgeted and is meant to be O(1) — camera,
 * frame, puck, companion. Past this, it is eating the budget the phases after
 * it receive. */
const INTERACTION_WARN_MS = 2;

/** A sheet larger than it starts, and this sparse, is one compaction should
 * already have halved — billboard/atlas.ts's `compact`. */
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

type LedgerGlobals = typeof globalThis & {
  __pxl_ledger_trace?: () => string;
  __pxl_ledger_log?: () => Promise<string>;
  __pxl_alloc_report?: () => string;
  __pxl_alloc_reset?: () => void;
};

export function LedgerOverlay() {
  const show = useSettingsStore((s) => s.show_ledger);
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
    alloc_probe_install();
    w.__pxl_alloc_report = alloc_probe_report;
    w.__pxl_alloc_reset = alloc_probe_reset;
    return () => {
      delete w.__pxl_ledger_trace;
      delete w.__pxl_ledger_log;
      delete w.__pxl_alloc_report;
      delete w.__pxl_alloc_reset;
    };
  }, []);

  // Panel repaint only while the panel is up.
  useEffect(() => {
    if (!import.meta.env.DEV || !show) return;
    const id = setInterval(() => force((n) => n + 1), POLL_MS);
    return () => clearInterval(id);
  }, [show]);

  if (!import.meta.env.DEV || !show) return null;

  const since = performance.now() - LIVE_WINDOW_MS;
  const f = summarize_frames(ledger_frames_since(since));
  const c = window_counters();
  const atlas = badge_atlas.stats();
  const atlas_pct = Math.round((atlas.used_px / (atlas.size * atlas.size)) * 100);
  const index = poi_index_stats();
  const tasks = summarize_tasks(ledger_tasks_since(since));
  const longs = ledger_long_frames_since(since);
  const rec = ledger_recording_state();
  const logging = ledger_logging();
  const worst_long = longs.reduce<(typeof longs)[number] | null>((w, l) => (!w || l.duration_ms > w.duration_ms ? l : w), null);
  const loaf_supported = typeof PerformanceObserver !== "undefined" &&
    !!PerformanceObserver.supportedEntryTypes?.includes("long-animation-frames");

  return (
    <div className="pointer-events-none fixed left-2 top-[84px] z-50 max-h-[calc(100dvh-96px)] w-[228px] overflow-hidden rounded-lg bg-black/72 p-2.5 font-mono text-[10.5px] leading-[1.5] text-white/90 backdrop-blur-sm">
      <div className="mb-1 font-semibold tracking-wide text-white/55">sched/ledger, last {LIVE_WINDOW_MS / 1000}s</div>

      {
        /* The recording and its buttons come first: on a phone the panel
          runs past the bottom of the screen, and what must stay reachable
          is start/stop and the exports, not the last rows of numbers. The
          body below is clipped rather than scrolled — the panel lets
          gestures through to the map, and a scrolling body would not. */
      }
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
      <div className="mt-1 flex gap-1">
        <button
          type="button"
          onClick={() => {
            ledger_window_start();
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
        <button
          type="button"
          onClick={() => {
            ledger_set_logging(!logging);
            force((n) => n + 1);
          }}
          title="stream each settled task, missed frame and long frame to the console"
          className={`pointer-events-auto flex-1 rounded border py-1 transition-colors ${
            logging ? "border-sky-400/40 bg-sky-400/10 text-sky-300 hover:bg-sky-400/20" : "border-white/15 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white/90"
          }`}
        >
          log
        </button>
      </div>
      <div className="my-1 border-t border-white/10" />

      {!f && <div className="text-white/45">no frames sampled yet</div>}
      {f && (
        <>
          <Row k="dt p50/p95 moving" v={`${r1(f.dt.p50)}/${r1(f.dt.p95)}ms`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="dt max" v={`${r1(f.dt.max)}ms`} warn={f.dt.max > LONG_FRAME_MS} />
          <Row k="missed/moving" v={`${f.missed}/${f.moving}`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row
            k="task/offthd p95"
            v={`${r1(f.task.p95)}/${r1(f.offthread.p95)}ms`}
            warn={f.offthread.p95 > f.last.frame_ms}
          />
          <Row k="gpu p95" v={f.gpu.p95 > 0 ? `${r1(f.gpu.p95)}ms` : "n/a"} warn={f.gpu.p95 > f.last.frame_ms} />
          <Row k="sched p95/max" v={`${r1(f.sched.p95)}/${r1(f.sched.max)}ms`} warn={f.overran > 0} />
          <Row
            k="budget/frame"
            v={`${r1(f.last.budget_ms)}/${r1(f.last.frame_ms)}ms${f.last.settled ? " still" : ""}`}
          />
          <Row k="rck/cmt p95" v={`${r1(f.recheck.p95)}/${r1(f.commit.p95)}`} />
          <Row
            k="phase p95 i/v/c/s"
            v={`${r1(f.phases.interaction.p95)}/${r1(f.phases.visible.p95)}/${r1(f.phases.content.p95)}/${r1(f.phases.cosmetic.p95)}`}
            warn={f.phases.interaction.p95 > INTERACTION_WARN_MS}
          />
          <Row k="bake/scan per s busy" v={`${Math.round(f.backlog_per_s.bakes)}/${Math.round(f.backlog_per_s.scans)}`} />
          <div className="my-1 border-t border-white/10" />
          <Row
            k="lod c/f/p"
            v={`${f.last.lod.chunks}/${f.last.lod.in_frustum}/${f.last.lod.pending}`}
            warn={f.last.lod.pending > 32}
          />
          <Row
            k="wait rel/vis"
            v={`${f.last.lod.offscreen_pending} ${Math.round(f.last.lod.oldest_release_ms)}/${Math.round(f.last.lod.oldest_visible_ms)}ms`}
            warn={Math.max(f.last.lod.oldest_release_ms, f.last.lod.oldest_visible_ms) > STARVED_MS}
          />
          <div className="my-1 border-t border-white/10" />
          <Row k="draws" v={String(f.last.draw_calls)} />
          <Row k="tris" v={`${(f.last.triangles / 1000).toFixed(0)}k`} />
          {f.census.slice(0, CENSUS_ROWS).map(([tag, d, t]) => <Row key={tag} k={`  ${tag} d/tris`} v={`${Math.round(d)}/${tris_text(t)}`} />)}
          <Row k="buffers" v={`${f.last.buffers} ${f.last.buffer_mb}MB`} />
          <Row k="fill Mpx" v={(f.last.px / 1e6).toFixed(1)} warn={f.last.px / 1e6 > 12} />
          <Row k="targets MB" v={`d${f.last.textures_mb.depth} m${f.last.textures_mb.msaa} s${f.last.textures_mb.shadow} l${f.last.textures_mb.layers}`} />
          <div className="my-1 border-t border-white/10" />
          <Row k="heap" v={f.last.heap_mb ? `${f.last.heap_mb}MB Δ${f.heap_delta >= 0 ? "+" : ""}${f.heap_delta}` : "n/a"} />
          <Row
            k="tiles ~mb / poi"
            v={`${f.last.tile_pages} ~${f.last.tile_pages * TILE_DECODED_MB} / ${f.last.poi_count}`}
          />
          <Row
            k="pool live/idle hit"
            v={`${f.last.pool.live_mb}/${f.last.pool.idle_mb}MB ${f.pool_hit_pct ?? "-"}%`}
            warn={f.pool_hit_pct !== null && f.pool_hit_pct < 80}
          />
          <Row
            k="  reserved/core"
            v={`${Math.round(core_pool_stat().reserved_bytes / 1048576)}/${Math.round((CORE_MEMORY?.buffer.byteLength ?? 0) / 1048576)}MB`}
            warn={core_pool_stat().reserved_bytes > (CORE_MEMORY?.buffer.byteLength ?? 0)}
          />
          <Row
            k="faces kept"
            v={`${c.faces_possible > 0 ? Math.round(c.faces_kept / c.faces_possible * 100) : 0}%`}
          />
          {
            /* Latency of a placement job, not a frame's cost — that is the step
              table's. Warns when one spans more frames than a camera settle
              would hide. */
          }
          <Row
            k="poi place ms/f"
            v={`${r1(poi_place_stats.ms)}/${poi_place_stats.frames} max${poi_place_stats.max_frames}f hit${poi_place_stats.hits}`}
            warn={poi_place_stats.max_frames > PLACE_FRAMES_WARN}
          />
          <Row
            k="  cand/kept idx"
            v={`${poi_place_stats.candidates}/${poi_place_stats.kept} ${index.indexed}${index.dropped > 0 ? ` -${index.dropped}` : ""}`}
            warn={index.dropped > 0}
          />
          <Row
            k="atlas"
            v={`${atlas.size}px ${atlas.entries} ${atlas_pct}%`}
            warn={atlas.size > ATLAS_INITIAL && atlas_pct < ATLAS_SPARSE_PCT}
          />
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
              v={`${r1(t.lat.p50)}/${r1(t.lat.p95)}ms`}
            />
            <Row k="  ok/ab/err" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
            {t.reasons.length > 0 && <Row k="  top err" v={`${t.reasons[0][0]} x${t.reasons[0][1]}`} warn />}
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
    </div>
  );
}
