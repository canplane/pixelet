// @pxl/client/shell/debug/profiler/lane.ts

/**
 * How the report, panel and trace show a frame's lanes, as abi/profiler.ts
 * decodes them. A lane's unit is its name's suffix (`_ms`, `_bytes`, `_px`,
 * `_name`, as profiler.json FRAME says), so a lane the kernel adds needs no
 * word here; only a list lane's names are spelled out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { PHASES } from "@xpute/host/sched/tick.spec.ts";
import { CENSUS_TAGS, FRAME_GROUP, LIMIT, WAKES } from "@/abi/profiler.spec.ts";
import type { Frame, ListLane } from "@/abi/profiler.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { op } from "@xpute/gpu/api.spec.ts";
import { STREAM_NAMES } from "@/abi/demand.spec.ts";
import { fixed } from "@/shell/fixed.ts";

/** The one unit every figure in bytes is shown in: a mebibyte. */
const MIB = 1048576;

/** The kernel counts by zoom: slot z, bit z or a value z is zoom z. */
export const zoomName = (z: number) => (Number.isNaN(z) ? "-" : `z${z}`);

const ZOOM_NAMES = Array.from({ length: LIMIT.ZOOMS }, (_, z) => zoomName(z));
/** Slot k is the objects op k + 1 made, as no op is 0: its name, lower case. */
const OBJECT_NAMES = Array.from(
  { length: op.DESTROY - 1 },
  (_, k) =>
    Object.entries(op).find(([, v]) => v === k + 1)?.[0].toLowerCase() ?? "-",
);
/** Each list lane's names, slot by slot. */
export const LIST_NAMES: Record<ListLane, readonly string[]> = {
  phases: PHASES,
  effects: EFFECT_NAMES,
  gpu_spans: GPU_SPAN_NAMES,
  census: CENSUS_TAGS,
  zoom_tiles: ZOOM_NAMES,
  gpu_objects: OBJECT_NAMES,
  waiting: STREAM_NAMES,
  out: STREAM_NAMES,
};

export type FrameGroup = Exclude<keyof typeof FRAME_GROUP, "$doc">;
export const GROUPS = Object.keys(FRAME_GROUP).filter((g) =>
  !g.startsWith("$")
) as FrameGroup[];

export const n1 = (n: number) => fixed(n ?? 0, 1);

const UNIT = /_(ms|bytes|px)$/;

/** A task phase's mean or total: bytes are counted, the rest are ms. */
export const phaseText = ([k, v]: [string, f64]) =>
  `${k.replace(UNIT, "")} ${k === "bytes" ? round(v) : n1(v)}`;

export const pct = (
  part: number,
  whole: number,
) => (whole > 0 ? round((part / whole) * 100) : 0);

export const tris = (
  t: number,
) => (t >= 1000 ? `${round(t / 1000)}k` : String(round(t)));

/** A frame's GPU time: its spans' sum (pxl-render renderer.rs `gpu_ms`). */
export const gpuMs = (f: Frame): f64 =>
  f.gpu_spans.reduce((n, ms) => n + ms, 0);

/** A frame's draws and triangles: its census's sums, every pipeline a draw names. */
export const drawn = (f: Frame): [f64, f64] =>
  f.census.reduce(([d, t], [cd, ct]) => [d + cd, t + ct], [0, 0]);

const NAME_TABLES: Record<string, readonly string[]> = {
  kind_name: FRAME_KINDS,
  wake_name: WAKES,
};

export function valueText(name: string, v: f64): string {
  if (name.endsWith("_name")) return NAME_TABLES[name]?.[v] ?? "-";
  if (name.endsWith("_ms")) return `${n1(v)} ms`;
  if (name.endsWith("_bytes")) return `${+fixed(v / MIB, 1)} MiB`;
  if (name.endsWith("_px")) return `${fixed(v / 1e6, 1)} Mpx`;
  return Number.isInteger(v) ? String(v) : n1(v);
}
