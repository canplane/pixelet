// @pxl/web/feature/debug/profiler/gauge.tsx

/**
 * The gauge half of the instrument in
 * issue/20260909-cooperative-scheduler-and-task-ledger.md: the kernel's
 * numbers over the last few seconds, painted as a corner panel. It reads and
 * never writes — nothing here feeds back into scheduling.
 *
 * What the report has and this does not is detail, not kind: per-unit costs,
 * retention counts, band reuse, mounts and the window peak stay in the report.
 * Everything here comes from the kernel's summary block, read in place
 * (view.ts) — the same block the report is built from. It is grouped the way
 * the report's lanes are (profiler.json FRAME_GROUPS), in the order a reader
 * asks: **pace**, where the turn's time went against the display's measured
 * frame; **ground**, what the chunk scheduler is holding and what its
 * ceilings left behind; **draw**, the GPU's time and what it drew; **work**,
 * what is being built and fetched; **memory**, what all of it holds. Task
 * latency with its commonest failure and the LoAF CPU-vs-render split follow.
 * A value reads as lane.ts says a lane does.
 */

import { round } from "@xpute/core/math/scalar.ts";
import type { ReactNode } from "react";
import { StyleSheet, Text, View } from "react-native";

import { EFFECT_NAMES } from "./profiler.spec.ts";
import { n1, pct, tris, valueText } from "./lane.ts";
import { BOUND, failText } from "./report.ts";
import type { FrameSummary, Gauge, Spread } from "./view.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import { CORE } from "@/kernel/mem.ts";
import { alpha, color, font } from "@/shared/ui/theme.ts";

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
 * below the sixteen-second wait that aging exists to prevent: past it, aging
 * is not doing its job. */
const STARVED_MS = 10_000;

/** The levels a bitmask names, as the census packs them (world/chunk/table.rs). */
function levelBits(mask: number): string {
  const out: number[] = [];
  for (let k = 0; mask >>> k; k++) if (mask & (1 << k)) out.push(k);
  return out.length ? out.join("\u00b7") : "-";
}
/** A sheet larger than it starts, and this sparse, is one compaction should
 * already have halved. */
const ATLAS_INITIAL = 1024;
const ATLAS_SPARSE_PCT = 25;

function Row(props: { k: string; v: string; warn?: boolean }) {
  return (
    <View style={gauge.row}>
      <Text style={[gauge.text, gauge.key]}>{props.k}</Text>
      <Text style={[gauge.text, props.warn && gauge.warn]}>{props.v}</Text>
    </View>
  );
}

/** A note where a row's numbers would be. */
const Note = ({ children }: { children: string }) => <Text style={[gauge.text, gauge.key]}>{children}</Text>;

const Rule = () => <View style={gauge.rule} />;

/** A section's start: the rule that separated one from the next, with the word
 * it separates on it. The panel runs to forty rows and a reader is looking for
 * one of them; a bare rule says where a group ends and never what it is. */
export const Head = ({ children, end }: { children: string; end?: ReactNode }) => (
  <View style={gauge.head}>
    <Text style={[gauge.text, gauge.headText]}>{children}</Text>
    <View style={gauge.headLine} />
    {end}
  </View>
);

const phase = (f: FrameSummary, name: string): Spread => f.phases.find((p) => p.name === name)?.spread ?? { p50: 0, p95: 0, max: 0 };

/** The panel over one read of the window; the overlay polls and hands it down,
 * since the start/stop buttons above the panel read the same `rec`. */
export function ProfilerGauge({ g }: { g: Gauge | null }) {
  if (g === null) return <Note>no window open</Note>;

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

      {f === null && <Note>no frames sampled yet</Note>}
      {f !== null && (
        <>
          <Head>pace</Head>
          <Row k="dt p50/p95 moving" v={`${n1(f.dt.p50)}/${n1(f.dt.p95)}ms`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="dt max" v={`${n1(f.dt.max)}ms`} warn={f.dt.max > LONG_FRAME_MS} />
          <Row k="missed/moving" v={`${f.missed}/${f.moving}`} warn={f.missed > f.moving * MISSED_WARN_SHARE} />
          <Row k="task/offthd p95" v={`${n1(f.task.p95)}/${n1(f.offthread.p95)}ms`} warn={f.offthread.p95 > f.last.frame_ms} />
          <Row k="sched p95/max" v={`${n1(f.sched.p95)}/${n1(f.sched.max)}ms`} warn={f.overran > 0} />
          <Row k="budget/frame" v={`${n1(f.last.budget_ms)}/${n1(f.last.frame_ms)}ms${f.last.settled ? " still" : ""}`} />
          <Row
            k="phase i/v/c/s"
            v={`${n1(phase(f, "interaction").p95)}/${n1(phase(f, "visible").p95)}/${n1(phase(f, "content").p95)}/${n1(phase(f, "cosmetic").p95)}`}
            warn={phase(f, "interaction").p95 > INTERACTION_WARN_MS}
          />
          <Row k="chunks p95" v={n1(f.chunks.p95)} />
          {
            /* The turn in parts: the effects, the stage, and the longest
              single effect with its name. The tick's own numbers are above,
              so what none of the three accounts for is `task` less them —
              the region nobody has measured, which is where a fold once took
              seventy milliseconds of a turn whose tick was one and a half.
              Nothing in the effects may be work; they decide what is wanted
              and are not budgeted. */
          }
          <Row k="turn eff/stage" v={`${n1(f.last.turn_effects_ms)}/${n1(f.last.turn_stage_ms)}ms`} warn={f.last.turn_effects_ms > 4} />
          <Row k="worst effect" v={`${n1(f.last.turn_step_ms)}ms ${EFFECT_NAMES[f.last.turn_step_name] ?? "-"}`} warn={f.last.turn_step_ms > 4} />

          <Head>ground</Head>
          <Row k="lod c/f/p" v={`${f.last.lod_chunks}/${f.last.lod_in_frustum}/${f.last.lod_pending}`} warn={f.last.lod_pending > 32} />
          <Row
            k="wait rel/vis"
            v={`${f.last.lod_offscreen_pending} ${round(f.last.lod_oldest_release_ms)}/${round(f.last.lod_oldest_visible_ms)}ms`}
            warn={Math.max(f.last.lod_oldest_release_ms, f.last.lod_oldest_visible_ms) > STARVED_MS}
          />
          {
            /* The stale population, and how much of it is doing anything: one
              that is on screen with a layout is holding ground up, one that is
              not is only waiting to be let go. `hid` is the wanted chunks that
              have a layout a drawn stale chunk is standing over. */
          }
          <Row
            k="stale n/drawn/hid"
            v={`${round(f.last.stale_chunks)}/${round(f.last.stale_drawn)}/${round(f.last.stale_hidden)}`}
          />
          {
            /* Stale ground kept because nothing covers it yet, and of those the
              ones whose cover is itself waiting on a slot — a wait that closes
              on itself. */
          }
          <Row
            k="stale unc/unrdy/wait"
            v={`${round(f.last.stale_uncovered)}/${round(f.last.stale_unready)}/${round(f.last.stale_on_waiter)}`}
            warn={f.last.stale_on_waiter > 0}
          />
          {
            /* A finished mesh thrown away because every slot of its pool is
              held by a chunk drawing from it, and the tiers baked a step
              coarser than asked because they had more boxes than a slot holds:
              the levels are the waiters' and the holders'
              (world/chunk/mesh_slot.rs). */
          }
          <Row
            k="slot wait/coarser"
            v={`${round(f.last.slot_waits)}/${round(f.last.slot_coarser)} L${levelBits(f.last.slot_wait_levels)} on ${levelBits(f.last.slot_holder_levels)}`}
            warn={f.last.slot_waits > 0}
          />
          {
            /* What the frame's ceilings left behind: chunks on screen wanting a
              layout the commit pass did not reach, chunks the selection wants
              that are not mounted, and pages that found every record carrying
              one worth more. `full` where the mount pass stopped because the
              table is full rather than because its own cap was spent — the one
              of the three that says a number is too small rather than that a
              frame was short. None of those numbers was derived; this is what
              says whether one of them is holding the screen back. */
          }
          <Row
            k="cap commit/mount/page"
            v={`${round(f.last.cap_commit_left)}/${round(f.last.cap_mount_left)}${f.last.cap_mount_full ? " full" : ""}/${round(f.last.cap_record_refused)}`}
            warn={f.last.cap_mount_full > 0 || f.last.cap_record_refused > 0}
          />

          <Head>draw</Head>
          <Row k="gpu p95" v={f.gpu.p95 > 0 ? `${n1(f.gpu.p95)}ms` : "n/a"} warn={f.gpu.p95 > f.last.frame_ms} />
          {heaviest(f.gpu_spans, (s) => s.spread.p95, SPAN_ROWS).map((s) => <Row key={s.name} k={`${s.name} p95`} v={`${n1(s.spread.p95)}ms`} />)}
          <Row k="draws" v={String(f.last.draw_calls)} />
          <Row k="tris" v={tris(f.last.triangles)} />
          {f.census.slice(0, CENSUS_ROWS).map((r) => <Row key={r.tag} k={`${r.tag} d/tris`} v={`${round(r.draws)}/${tris(r.tris)}`} />)}
          {heaviest(f.level_tris.map((t, k) => ({ k, t })), (r) => r.t, LEVEL_ROWS).map((r) => <Row key={r.k} k={`L${r.k} tris`} v={tris(r.t)} />)}
          <Row k="fill Mpx" v={(f.last.px / 1e6).toFixed(1)} warn={f.last.px / 1e6 > 12} />

          <Head>work</Head>
          <Row k="bake/scan per s busy" v={`${round(f.backlogPerS[1])}/${round(f.backlogPerS[0])}`} />
          <Row k="faces kept" v={`${possible > 0 ? round((kept / possible) * 100) : 0}%`} />
          <Row k="queue t/p" v={`${f.last.q_terrain}/${f.last.q_poi}`} warn={f.last.q_terrain + f.last.q_poi > 12} />
          <Row k="rings cq/spill" v={`${f.last.ring_cq_peak}/${f.last.ring_spill_peak}`} warn={f.last.ring_spill_peak > 0} />

          <Head>memory</Head>
          <Row k="heap Δ" v={`${f.used_delta_mb >= 0 ? "+" : ""}${f.used_delta_mb}MB`} />
          {
            /* How far the allocator has ever reached into its range, what is
              on loan now, the range, what a run holds against what is out in
              slots of it, what it refused and the largest block it has free.
              All come from the kernel's own seam to it (host.rs) — the shell
              never decodes the allocator's bytes, which is what let the last
              probe read a deleted structure's row for two commits without
              saying so. The two shares are the reader's arithmetic on lanes
              beside them and not lanes of their own: a lane that restates
              two others is the one that goes stale unnoticed. `live` counts a
              carved run whole, so it is what the heap has committed; `slot`
              is what anything actually holds, and the gap between the two is
              what the carving costs. */
          }
          <Row
            k="used/live of"
            v={`${f.last.heap_used_mb}/${f.last.heap_live_mb} of ${f.last.heap_range_mb}MB ${pct(f.last.heap_live_mb, f.last.heap_range_mb)}%`}
            warn={f.last.heap_live_mb >= f.last.heap_range_mb * 0.8}
          />
          <Row
            k="slab run/slot"
            v={`${n1(f.last.heap_run_mb)}/${n1(f.last.heap_slot_mb)}MB ${pct(f.last.heap_slot_mb, f.last.heap_run_mb)}%`}
            warn={f.last.heap_run_mb > 1 && f.last.heap_slot_mb < f.last.heap_run_mb / 2}
          />
          <Row k="refused / free" v={`${f.last.heap_refusals} / ${n1(f.last.heap_largest_free_mb)}MB`} warn={f.last.heap_refusals > 0} />
          {
            /* Pages are records in `.fixed`, a fixed number of them a level,
              so the count is what the window holds and not a cost in bytes. */
          }
          <Row
            k="pages"
            v={`${f.last.pages_total}${f.last.pages_total > 0 ? ` in L${f.last.pages_by_level.findIndex((n) => n > 0)}..L${f.last.pages_by_level.findLastIndex((n) => n > 0)}` : ""}`}
          />
          {
            /* The shell's own number: the reservation is the central scheduler's,
              and the kernel is handed a range rather than told how big the page
              thinks it is. */
          }
          <Row k="core MB" v={`${round(CORE.buffer.byteLength / 1048576)}MB`} />
          <Row k="buffers / arena" v={`${f.last.buffers} ${valueText("_mb", f.last.buffer_mb)} / ${valueText("_mb", f.last.arena_mb)}`} />
          <Row k="targets MB" v={`d${f.last.tex_depth_mb} s${f.last.tex_shadow_mb} l${f.last.tex_layers_mb}`} />
          {g.atlas.entries > 0 && <Row k="atlas" v={`${g.atlas.size}px ${g.atlas.entries} ${atlasPct}%`} warn={g.atlas.size > ATLAS_INITIAL && atlasPct < ATLAS_SPARSE_PCT} />}
        </>
      )}

      {g.tasks.length > 0 && <Rule />}
      {g.tasks.map((t) => (
        <View key={t.category}>
          <Row k={`${t.category}${BOUND[t.category] ? ` (${BOUND[t.category].bound})` : ""} p50/p95`} v={`${n1(t.lat.p50)}/${n1(t.lat.p95)}ms`} />
          <Row k="ok/ab/err" v={`${t.ok}/${t.aborted}/${t.error}`} warn={t.error > 0} />
          {t.reasons.length > 0 && <Row k="top err" v={`${failText(t.reasons[0].fail, t.reasons[0].detail)} x${t.reasons[0].n}`} warn />}
        </View>
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

/** The panel's type: a small monospace, which the numbers line up in. */
export const gauge = StyleSheet.create({
  text: { fontFamily: font.mono, fontSize: 10.5, lineHeight: 16, color: alpha(color.white, 0.9) },
  key: { color: alpha(color.white, 0.45) },
  warn: { color: color.profilerHalt },
  row: { flexDirection: "row", justifyContent: "space-between", gap: 12 },
  rule: { marginVertical: 4, borderTopWidth: 1, borderTopColor: alpha(color.white, 0.1) },
  head: { flexDirection: "row", alignItems: "center", gap: 8, marginTop: 8, marginBottom: 2 },
  // Ten, macOS's smallest text style: the panel is dense on purpose, and
  // what is smaller than any scale sets does not render on a 1x screen.
  headText: { fontSize: 10, letterSpacing: 0.4, textTransform: "uppercase", color: alpha(color.white, 0.3) },
  headLine: { flex: 1, borderTopWidth: 1, borderTopColor: alpha(color.white, 0.1) },
});
