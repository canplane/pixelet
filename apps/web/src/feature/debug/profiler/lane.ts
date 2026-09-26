// @pxl/web/feature/debug/profiler/lane.ts

/**
 * The profiler's words, in one table the report, the panel and the trace all
 * read: where each of a frame's lanes lies, which line of the report prints
 * it, and what its number reads as.
 *
 * The kernel names a lane and nothing more (profiler.json's FRAME), and the
 * name carries its unit — `_ms`, `_mb`, `_levels`, `_name` — so a lane it
 * adds is read, grouped and printed here without a word written for it. Only
 * a lane that is a list of other names' values — phases, spans, pipelines,
 * levels — is spelled out, once, below, as is the table a `_name` indexes.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";

import { CENSUS_TAGS, EFFECT_NAMES, FRAME, FRAME_GROUPS, GPU_SPAN_NAMES, LIMITS, PHASE_NAMES } from "./profiler.spec.ts";

const MB = 1048576;

// ---- the lanes ----

/** The lanes that are lists, each of the names its words are in order of. */
type ListLane = "phases" | "gpu_spans" | "census" | "level_tris" | "pages_by_level";
type FrameLane = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarLane = Exclude<FrameLane, ListLane>;

/** A frame as the shell holds it once read: a number a lane, a list a list lane. */
export type Frame =
  & { [K in ScalarLane]: f64 }
  & { phases: f64[]; gpu_spans: f64[]; census: [f64, f64][]; level_tris: f64[]; pages_by_level: f64[] };

/** What a list lane's words are named by, in order; a level is its number. */
const LEVEL_NAMES = Array.from({ length: LIMITS.LEVELS }, (_, k) => `L${k}`);
export const LIST_NAMES: Record<ListLane, readonly string[]> = {
  phases: PHASE_NAMES,
  gpu_spans: GPU_SPAN_NAMES,
  census: CENSUS_TAGS,
  level_tris: LEVEL_NAMES,
  pages_by_level: LEVEL_NAMES,
};

/** Every lane in the order it lies, with the words it spans. */
export const LANES: readonly { name: FrameLane; at: u32; width: u32 }[] = (() => {
  const at = (Object.entries(FRAME) as [FrameLane | "WORDS", u32][]).filter(([k]) => k !== "WORDS") as [FrameLane, u32][];
  at.sort((a, b) => a[1] - b[1]);
  return at.map(([name, a], i) => ({ name, at: a, width: (i + 1 < at.length ? at[i + 1][1] : FRAME.WORDS) - a }));
})();

export const isList = (lane: FrameLane): lane is ListLane => lane in LIST_NAMES;

/** The frame in the `words` at `o`, laid out as FRAME says. */
export function frameAt(words: ArrayLike<f64>, o: u32): Frame {
  const f: Record<string, f64 | f64[] | [f64, f64][]> = {};
  for (const { name, at, width } of LANES) {
    const w = (k: u32) => words[o + at + k];
    if (name === "census") f[name] = CENSUS_TAGS.map((_, k) => [w(2 * k), w(2 * k + 1)] as [f64, f64]);
    else if (isList(name)) f[name] = Array.from({ length: width }, (_, k) => w(k));
    else f[name] = w(0);
  }
  return f as Frame;
}

type FrameGroup = Exclude<keyof typeof FRAME_GROUPS, "$doc">;
export const GROUPS = Object.keys(FRAME_GROUPS).filter((g) => !g.startsWith("$")) as FrameGroup[];

// ---- the words ----

export const n1 = (n: number) => (n ?? 0).toFixed(1);

/** `part` of `whole` as a whole percent, 0 where there is no whole. */
export const pct = (part: number, whole: number) => (whole > 0 ? round((part / whole) * 100) : 0);

/** Triangles read as thousands past a thousand. */
export const tris = (t: number) => (t >= 1000 ? `${round(t / 1000)}k` : String(round(t)));

/** The levels a bit set names, as `0,3,5`, or `-` for none. */
function levelsText(bits: f64): string {
  const out: u32[] = [];
  for (let k = 0; k < 32; k++) if (bits & (1 << k)) out.push(k);
  return out.length > 0 ? out.join(",") : "-";
}

/** Which name table a `_name` lane indexes. The kernel records the number and
 * the words are the shell's, the way an errno's are libc's, so the table is
 * named here rather than sent. */
const NAME_TABLES: Record<string, readonly string[]> = { turn_step_name: EFFECT_NAMES };

/** A number as a lane named `name` reads: its unit is its name's. */
export function valueText(name: string, v: f64): string {
  if (name.endsWith("_name")) return NAME_TABLES[name]?.[v] ?? "-";
  if (name.endsWith("_ms")) return `${n1(v)}ms`;
  if (name.endsWith("_mb")) return `${+v.toFixed(1)}MB`;
  if (name.endsWith("_bytes")) return `${+(v / MB).toFixed(1)}MB`;
  if (name.endsWith("_levels")) return levelsText(v);
  if (name === "triangles" || name.endsWith("_tris")) return tris(v);
  if (name === "px") return `${(v / 1e6).toFixed(1)}Mpx`;
  return Number.isInteger(v) ? String(v) : n1(v);
}

/** A lane's name as a line of `group` prints it: the group's prefix and the
 * unit's suffix off, which the value says instead. */
export function label(group: string, name: string): string {
  const bare = name.startsWith(`${group}_`) ? name.slice(group.length + 1) : name;
  return bare.replace(/_(ms|mb|bytes|px)$/, "") || name;
}

/** A list lane's words, `name value` each, the zeros left out. */
function listText(lane: ListLane, f: Frame): string {
  const names = LIST_NAMES[lane];
  if (lane === "census") {
    const rows = f.census.map(([draws, t], i) => ({ tag: names[i] ?? String(i), draws, t })).filter((r) => r.draws > 0 || r.t > 0);
    rows.sort((a, b) => b.t - a.t || b.draws - a.draws);
    return rows.map((r) => `${r.tag} ${round(r.draws)}/${tris(r.t)}`).join(" ");
  }
  const unit = lane === "gpu_spans" || lane === "phases" ? "_ms" : lane === "level_tris" ? "_tris" : "";
  return f[lane].map((v, i) => [names[i] ?? String(i), v] as const).filter(([, v]) => v > 0).map(([n, v]) => `${n} ${valueText(unit, v)}`).join(" ") || "-";
}

/** One group of a frame as a line of the report: `label value` a lane. */
export function groupText(group: FrameGroup, f: Frame): string {
  return (FRAME_GROUPS[group] as readonly FrameLane[]).map((lane) => {
    const l = label(group, lane);
    return isList(lane) ? `${l} [${listText(lane, f)}]` : `${l} ${valueText(lane, f[lane])}`;
  }).join("  ");
}

/** A summary section's field as the report prints it, by its name. */
export const fieldText = (name: string, v: f64) => `${name.replace(/_(ms|mb|bytes)$/, "")} ${valueText(name, v)}`;
