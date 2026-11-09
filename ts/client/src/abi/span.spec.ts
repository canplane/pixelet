// @pxl/client/abi/span.spec.ts
//
// GENERATED from spec/client/render/gpu/span.json
//
// What the renderer reports of a frame (pxl-render renderer.rs): the GPU spans it is timed in,
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
  "character",
  "cast",
  "beam_hit",
  "beam",
  "grid_cell",
  "sample_hit",
  "sample",
  "raster_box",
  "sraster",
  "px",
  "hole",
  "light_world_receivers",
  "light_draws",
  "light_slide",
  "light_propagate",
  "light_damage",
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
  "damage",
  "shadow_damage",
  "propagate",
] as const;

export const LIGHT_CHANGED = [
  "content",
  "slide",
  "receivers",
  "footprint",
  "world",
] as const;

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
