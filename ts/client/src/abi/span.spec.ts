// @pxl/client/abi/span.spec.ts
//
// GENERATED from spec/client/render/gpu/span.json (xpute-gen data.rs)
//
// What the renderer reports of a frame (pxl-render renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

export const GPU_SPAN_NAMES = [
  {
    name: "air",
    per_px: false,
  },
  {
    name: "exposure",
    per_px: false,
  },
  {
    name: "ao",
    per_px: true,
  },
  {
    name: "world",
    per_px: true,
  },
  {
    name: "world_rest",
    per_px: true,
  },
  {
    name: "resolve",
    per_px: true,
  },
  {
    name: "finish",
    per_px: false,
  },
  {
    name: "marker",
    per_px: false,
  },
  {
    name: "debug",
    per_px: false,
  },
  {
    name: "character",
    per_px: false,
  },
  {
    name: "cast",
    per_px: false,
  },
  {
    name: "beam_hit",
    per_px: false,
  },
  {
    name: "beam",
    per_px: false,
  },
  {
    name: "grid_cell",
    per_px: false,
  },
  {
    name: "sample_hit",
    per_px: false,
  },
  {
    name: "sample",
    per_px: false,
  },
  {
    name: "raster_box",
    per_px: false,
  },
  {
    name: "sraster",
    per_px: true,
  },
  {
    name: "px",
    per_px: true,
  },
  {
    name: "hole",
    per_px: true,
  },
  {
    name: "light_world_receivers",
    per_px: true,
  },
  {
    name: "light_draws",
    per_px: false,
  },
  {
    name: "light_slide",
    per_px: false,
  },
  {
    name: "light_propagate",
    per_px: false,
  },
  {
    name: "light_damage",
    per_px: false,
  },
  {
    name: "light_beam_hit",
    per_px: false,
  },
  {
    name: "light_beam",
    per_px: false,
  },
  {
    name: "light_sample",
    per_px: false,
  },
  {
    name: "light_raster",
    per_px: false,
  },
  {
    name: "light_slot",
    per_px: false,
  },
  {
    name: "light_hit",
    per_px: false,
  },
  {
    name: "light_walk",
    per_px: false,
  },
  {
    name: "generate",
    per_px: false,
  },
  {
    name: "shadow",
    per_px: false,
  },
  {
    name: "face",
    per_px: false,
  },
  {
    name: "damage",
    per_px: true,
  },
  {
    name: "shadow_damage",
    per_px: false,
  },
  {
    name: "propagate",
    per_px: true,
  },
] as const;

export const GpuSpanName = {
  air: 0,
  exposure: 1,
  ao: 2,
  world: 3,
  world_rest: 4,
  resolve: 5,
  finish: 6,
  marker: 7,
  debug: 8,
  character: 9,
  cast: 10,
  beam_hit: 11,
  beam: 12,
  grid_cell: 13,
  sample_hit: 14,
  sample: 15,
  raster_box: 16,
  sraster: 17,
  px: 18,
  hole: 19,
  light_world_receivers: 20,
  light_draws: 21,
  light_slide: 22,
  light_propagate: 23,
  light_damage: 24,
  light_beam_hit: 25,
  light_beam: 26,
  light_sample: 27,
  light_raster: 28,
  light_slot: 29,
  light_hit: 30,
  light_walk: 31,
  generate: 32,
  shadow: 33,
  face: 34,
  damage: 35,
  shadow_damage: 36,
  propagate: 37,
} as const;
export type GpuSpanName = (typeof GpuSpanName)[keyof typeof GpuSpanName];

/** The name of the `GpuSpanName` member numbered `n`, or undefined for a number
 * no member has. */
export function gpu_span_name_name(
  n: number,
): keyof typeof GpuSpanName | undefined {
  for (const k in GpuSpanName)
    if (GpuSpanName[k as keyof typeof GpuSpanName] === n)
      return k as keyof typeof GpuSpanName;
  return undefined;
}

/** The `GpuSpanName` member `n` is, or null for a number no member has. */
export function gpu_span_name_of(n: number): GpuSpanName | null {
  return gpu_span_name_name(n) === undefined ? null : (n as GpuSpanName);
}

export const LIGHT_CHANGED = [
  "content",
  "slide",
  "receivers",
  "footprint",
  "world",
] as const;

export const LightChanged = {
  content: 0,
  slide: 1,
  receivers: 2,
  footprint: 3,
  world: 4,
} as const;
export type LightChanged = (typeof LightChanged)[keyof typeof LightChanged];

/** The name of the `LightChanged` member numbered `n`, or undefined for a number
 * no member has. */
export function light_changed_name(
  n: number,
): keyof typeof LightChanged | undefined {
  for (const k in LightChanged)
    if (LightChanged[k as keyof typeof LightChanged] === n)
      return k as keyof typeof LightChanged;
  return undefined;
}

/** The `LightChanged` member `n` is, or null for a number no member has. */
export function light_changed_of(n: number): LightChanged | null {
  return light_changed_name(n) === undefined ? null : (n as LightChanged);
}

export const FRAME_KINDS = [
  "none",
  "unready",
  "dropped",
  "plain",
  "still",
  "cast",
  "unchanged",
  "px",
  "masked",
] as const;

export const FrameKind = {
  none: 0,
  unready: 1,
  dropped: 2,
  plain: 3,
  still: 4,
  cast: 5,
  unchanged: 6,
  px: 7,
  masked: 8,
} as const;
export type FrameKind = (typeof FrameKind)[keyof typeof FrameKind];

/** The name of the `FrameKind` member numbered `n`, or undefined for a number
 * no member has. */
export function frame_kind_name(n: number): keyof typeof FrameKind | undefined {
  for (const k in FrameKind)
    if (FrameKind[k as keyof typeof FrameKind] === n)
      return k as keyof typeof FrameKind;
  return undefined;
}

/** The `FrameKind` member `n` is, or null for a number no member has. */
export function frame_kind_of(n: number): FrameKind | null {
  return frame_kind_name(n) === undefined ? null : (n as FrameKind);
}
