// @pxl/client/ui/debug/perf/column.ts

/**
 * How the report, panel and trace show a frame's columns, as abi/perf.ts
 * decodes them. A column's unit is its name's suffix (`_ms`, `_bytes`, `_px`,
 * `_name`, as perf.json FRAME says), so a column the core adds needs no
 * word here; only a list column's names are spelled out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { PHASES } from "@/abi/phase.spec.ts";
import { FRAME_GROUP, WAKES } from "@/abi/perf.spec.ts";
import type { FrameColumn, ListColumn } from "@/port/perf.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { fixed } from "@/ui/fixed.ts";

/** The one unit every figure in bytes is shown in: a mebibyte. */
const MIB = 1048576;

/** The core gives a zoom as its number. */
export const zoomName = (z: number) => (Number.isNaN(z) ? "-" : `z${z}`);

/** Each list column's names, slot by slot. */
export const LIST_NAMES: Record<ListColumn, readonly string[]> = {
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

/** The names each `_name` column indexes, every one of FRAME's. */
type NameColumn = Extract<FrameColumn, `${string}_name`>;
const NAME_TABLES: { readonly [K in NameColumn]: readonly string[] } = {
  kind_name: FRAME_KINDS,
  wake_name: WAKES,
};
const isNameColumn = (name: string): name is NameColumn =>
  Object.hasOwn(NAME_TABLES, name);

export function valueText(name: string, v: f64): string {
  if (isNameColumn(name)) return NAME_TABLES[name][v] ?? "-";
  if (name.endsWith("_ms")) return `${n1(v)} ms`;
  if (name.endsWith("_bytes")) return `${+fixed(v / MIB, 1)} MiB`;
  if (name.endsWith("_px")) return `${fixed(v / 1e6, 1)} Mpx`;
  return Number.isInteger(v) ? String(v) : n1(v);
}
