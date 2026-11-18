// @pxl/client/ui/feature/debug/perf/column.ts

/**
 * How the report, the panel and the trace show a frame's columns, as
 * service/core/perf.ts decodes them. A column's name ends in its unit: `_ms`,
 * `_bytes`, `_px`, or `_name` for an index into a list of names, as `FRAME` in
 * `perf.json5` defines. So a column the core adds needs nothing here. Only the
 * names of a list column's slots are spelled out.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { PHASES } from "@/abi/phase.spec.ts";
import { FRAME_GROUP } from "@/abi/perf.spec.ts";
import { BASE_KINDS } from "@/abi/publication.spec.ts";
import type { Frame, FrameColumn, ListColumn } from "@/service/core/perf.ts";
import {
  INVALIDATION_REASONS,
  EFFECT_NAMES,
  STAGE_PART_NAMES,
} from "@/abi/step.spec.ts";
import {
  FRAME_KINDS,
  GPU_SPAN_NAMES,
  LIGHT_CHANGEDS,
} from "@/abi/span.spec.ts";
import { fixed } from "@/ui/shared/fixed.ts";

const MIB = 1048576;

export const zoomName = (z: number) => (Number.isNaN(z) ? "-" : `z${z}`);

/** The names of each list column's slots, in slot order. */
export const LIST_NAMES: Record<ListColumn, readonly string[]> = {
  phases: PHASES,
  stage_parts: STAGE_PART_NAMES,
  effects: EFFECT_NAMES,
  invalidations: INVALIDATION_REASONS,
  gpu_spans: GPU_SPAN_NAMES,
  loads: BASE_KINDS,
};

/** The names of the bits of each column that is a mask, bit k the k-th. */
export const BIT_NAMES: { readonly [K in FrameColumn]?: readonly string[] } = {
  light_changed: LIGHT_CHANGEDS,
};

/** The names of the bits set in `mask`, of `names`. */
export const bitsOf = (names: readonly string[], mask: f64) =>
  names.filter((_, k) => (mask >> k) & 1);

const sum = (ms: f64[]) => ms.reduce((a, b) => a + b, 0);
/** A frame's time in its effects and in its stage, each the sum of its
 * parts'. */
export const effectsMs = (f: Frame) => sum(f.effects);
export const stageMs = (f: Frame) => sum(f.stage_parts);

export type FrameGroup = keyof typeof FRAME_GROUP;
export const GROUPS = Object.keys(FRAME_GROUP) as FrameGroup[];

export const n1 = (n: number) => fixed(n ?? 0, 1);

const UNIT = /_(ms|bytes|px)$/;

/** A phase's name without its unit. */
export const phaseName = (k: string) => k.replace(UNIT, "");

/** The `bytes` phase is a count, and every other phase is in milliseconds. */
export const phaseText = ([k, v]: [string, f64]) =>
  `${phaseName(k)} ${k === "bytes" ? round(v) : n1(v)}`;

export const tris = (t: number) =>
  t >= 1000 ? `${round(t / 1000)}k` : String(round(t));

/** The type requires a table for every `_name` column of `FRAME`. */
type NameColumn = Extract<FrameColumn, `${string}_name`>;
const NAME_TABLES: { readonly [K in NameColumn]: readonly string[] } = {
  kind_name: FRAME_KINDS,
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
