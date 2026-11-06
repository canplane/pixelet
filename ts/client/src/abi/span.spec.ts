// @pxl/client/abi/span.spec.ts
//
// GENERATED from spec/client/render/gpu/span.json
//
// What the renderer reports of a frame (pxl-render gpu/renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

export const GPU_SPAN_NAMES = [
  "air",
  "exposure",
  "ao",
  "world",
  "world_rest",
  "resolve",
  "finish",
  "marker",
  "debug",
  "preview",
  "fill",
  "cast",
  "beam_hit",
  "beam",
  "cell",
  "sample_hit",
  "sample",
  "sample_box",
  "sraster",
  "px",
  "miss",
  "light_receivers",
  "light_draws",
  "light_slide",
  "light_propagate",
  "light_claim",
  "light_beam_hit",
  "light_beam",
  "light_sample",
  "light_raster",
  "light_slot",
  "light_hit",
  "light_walk",
  "generate",
  "shadow",
  "face",
  "mark",
  "light_mark",
  "propagate",
] as const;

export const LIGHT_MOVED = [
  "content",
  "slide",
  "receivers",
  "footprint",
  "world",
  "owed",
] as const;

export const FRAME_KINDS = [
  "none",
  "unready",
  "held",
  "dropped",
  "plain",
  "standing",
  "cast",
  "stood",
  "px",
  "gated",
] as const;
