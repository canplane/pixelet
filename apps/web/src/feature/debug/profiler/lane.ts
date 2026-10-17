// @pxl/web/feature/debug/profiler/lane.ts

/**
 * One table of lanes the report, panel and trace all read. A lane's unit is
 * its name's suffix (`_ms`, `_mb`, `_zooms`, `_name`), so a lane the kernel
 * adds needs no word here; only list lanes are spelled out.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { PHASES } from "@xpute/host/sched/tick.spec.ts";
import { CENSUS_TAGS, FRAME, FRAME_GROUPS, LIMITS, WAKES } from "./profiler.spec.ts";
import { EFFECT_NAMES } from "@/kernel/turn/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/kernel/render/gpu/span.spec.ts";
import { op } from "@xpute/gpu/api.spec.ts";
import { fixed } from "@/shared/fixed.ts";

/** The one megabyte every figure in bytes is shown in. */
export const MB = 1048576;

type ListLane = "phases" | "gpu_spans" | "census" | "zoom_tiles" | "gpu_objects";
type FrameLane = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarLane = Exclude<FrameLane, ListLane>;

export type Frame =
  & { [K in ScalarLane]: f64 }
  & { phases: f64[]; gpu_spans: f64[]; census: [f64, f64][]; zoom_tiles: f64[]; gpu_objects: f64[] };

/** The kernel counts by zoom: slot z, bit z or a value z is zoom z. */
export const zoomName = (z: number) => (Number.isNaN(z) ? "-" : `z${z}`);

const ZOOM_NAMES = Array.from({ length: LIMITS.ZOOMS }, (_, z) => zoomName(z));
/** Slot k is the objects op k made: its name, lower case. */
const OBJECT_NAMES = Array.from({ length: op.DESTROY }, (_, k) => Object.entries(op).find(([, v]) => v === k)?.[0].toLowerCase() ?? "-");
export const LIST_NAMES: Record<ListLane, readonly string[]> = {
  phases: PHASES,
  gpu_spans: GPU_SPAN_NAMES,
  census: CENSUS_TAGS,
  zoom_tiles: ZOOM_NAMES,
  gpu_objects: OBJECT_NAMES,
};

export const LANES: readonly { name: FrameLane; at: u32; width: u32 }[] = (() => {
  const at = (Object.entries(FRAME) as [FrameLane | "WORDS", u32][]).filter(([k]) => k !== "WORDS") as [FrameLane, u32][];
  at.sort((a, b) => a[1] - b[1]);
  return at.map(([name, a], i) => ({ name, at: a, width: (i + 1 < at.length ? at[i + 1][1] : FRAME.WORDS) - a }));
})();

export const isList = (lane: FrameLane): lane is ListLane => lane in LIST_NAMES;

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

export const n1 = (n: number) => fixed(n ?? 0, 1);

const UNIT = /_(ms|mb|bytes|px)$/;

/** A task phase's mean or total: bytes are counted, the rest are ms. */
export const phaseText = ([k, v]: [string, f64]) => `${k.replace(UNIT, "")} ${k === "bytes" ? round(v) : n1(v)}`;

export const pct = (part: number, whole: number) => (whole > 0 ? round((part / whole) * 100) : 0);

export const tris = (t: number) => (t >= 1000 ? `${round(t / 1000)}k` : String(round(t)));

/** A frame's draws and triangles: its census's sums, every pipeline a draw names. */
export const drawn = (f: Frame): [f64, f64] => f.census.reduce(([d, t], [cd, ct]) => [d + cd, t + ct], [0, 0]);

export function zoomsText(bits: f64): string {
  const out: string[] = [];
  for (let z = 0; z < 32; z++) if (bits & (1 << z)) out.push(zoomName(z));
  return out.length > 0 ? out.join(",") : "-";
}

const NAME_TABLES: Record<string, readonly string[]> = { worst_effect_name: EFFECT_NAMES, kind_name: FRAME_KINDS, wake_name: WAKES };

export function valueText(name: string, v: f64): string {
  if (name.endsWith("_name")) return NAME_TABLES[name]?.[v] ?? "-";
  if (name.endsWith("_ms")) return `${n1(v)}ms`;
  if (name.endsWith("_mb")) return `${+fixed(v, 1)}MB`;
  if (name.endsWith("_bytes")) return `${+fixed(v / MB, 1)}MB`;
  if (name.endsWith("_zooms")) return zoomsText(v);
  if (name === "triangles" || name.endsWith("_tris")) return tris(v);
  if (name === "px") return `${fixed(v / 1e6, 1)}Mpx`;
  return Number.isInteger(v) ? String(v) : n1(v);
}

function label(group: string, name: string): string {
  const bare = name.startsWith(`${group}_`) ? name.slice(group.length + 1) : name;
  return bare.replace(UNIT, "") || name;
}

function listText(lane: ListLane, f: Frame): string {
  const names = LIST_NAMES[lane];
  if (lane === "census") {
    const rows = f.census.map(([draws, t], i) => ({ tag: names[i] ?? String(i), draws, t })).filter((r) => r.draws > 0 || r.t > 0);
    rows.sort((a, b) => b.t - a.t || b.draws - a.draws);
    return rows.map((r) => `${r.tag} ${round(r.draws)}/${tris(r.t)}`).join(" ");
  }
  const unit = lane === "gpu_spans" || lane === "phases" ? "_ms" : "";
  return f[lane].map((v, i) => [names[i] ?? String(i), v] as const).filter(([, v]) => v > 0).map(([n, v]) => `${n} ${valueText(unit, v)}`).join(" ") || "-";
}

export function groupText(group: FrameGroup, f: Frame): string {
  return (FRAME_GROUPS[group] as readonly FrameLane[]).map((lane) => {
    const l = label(group, lane);
    return isList(lane) ? `${l} [${listText(lane, f)}]` : `${l} ${valueText(lane, f[lane])}`;
  }).join("  ");
}

export const fieldText = (name: string, v: f64) => `${name.replace(UNIT, "")} ${valueText(name, v)}`;
