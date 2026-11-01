// @pxl/client/shell/debug/perf/lane.ts

/**
 * How the report, panel and trace show a frame's lanes, as abi/perf.ts
 * decodes them. A lane's unit is its name's suffix (`_ms`, `_bytes`, `_px`,
 * `_name`, as perf.json FRAME says), so a lane the kernel adds needs no
 * word here; only a list lane's names are spelled out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { PHASES } from "@/abi/phase.spec.ts";
import { FRAME_GROUP, WAKES } from "@/abi/perf.spec.ts";
import type { ListLane } from "@/port/perf.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { fixed } from "@/shell/fixed.ts";

/** The one unit every figure in bytes is shown in: a mebibyte. */
const MIB = 1048576;

/** The kernel gives a zoom as its number. */
export const zoomName = (z: number) => (Number.isNaN(z) ? "-" : `z${z}`);

/** Each list lane's names, slot by slot. */
export const LIST_NAMES: Record<ListLane, readonly string[]> = {
  phases: PHASES,
  effects: EFFECT_NAMES,
  gpu_spans: GPU_SPAN_NAMES,
};

export type FrameGroup = Exclude<keyof typeof FRAME_GROUP, "$doc">;
export const GROUPS = Object.keys(FRAME_GROUP).filter(
  (g) => !g.startsWith("$"),
) as FrameGroup[];

export const n1 = (n: number) => fixed(n ?? 0, 1);

const UNIT = /_(ms|bytes|px)$/;

/** A task phase's mean or total: bytes are counted, the rest are ms. */
export const phaseText = ([k, v]: [string, f64]) =>
  `${k.replace(UNIT, "")} ${k === "bytes" ? round(v) : n1(v)}`;

export const pct = (part: number, whole: number) =>
  whole > 0 ? round((part / whole) * 100) : 0;

export const tris = (t: number) =>
  t >= 1000 ? `${round(t / 1000)}k` : String(round(t));

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
