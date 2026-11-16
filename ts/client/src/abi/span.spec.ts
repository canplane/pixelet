// @pxl/client/abi/span.spec.ts
//
// GENERATED from spec/client/render/gpu/span.json5 (xpute-gen data.rs)
//
// What the renderer reports of a frame (pxl-render renderer.rs): the GPU spans it is timed in,
// in the order the renderer encodes them, and the kind of frame it made; the profiler records and reports
// them by these names.

/** `world` is the world's pass as it lays the opaque world, and `world_rest` the pass that goes on after the occlusion is found between them (`ao`), drawing over it the occlusion's map, the sky and what the core animates; where the occlusion is not found again the one pass draws it all, timed as `world`, and `world_rest` runs none. */
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

export type GpuSpanName =
  | typeof GPU_SPAN_NAME_AIR
  | typeof GPU_SPAN_NAME_EXPOSURE
  | typeof GPU_SPAN_NAME_AO
  | typeof GPU_SPAN_NAME_WORLD
  | typeof GPU_SPAN_NAME_WORLD_REST
  | typeof GPU_SPAN_NAME_RESOLVE
  | typeof GPU_SPAN_NAME_FINISH
  | typeof GPU_SPAN_NAME_MARKER
  | typeof GPU_SPAN_NAME_DEBUG
  | typeof GPU_SPAN_NAME_CHARACTER
  | typeof GPU_SPAN_NAME_CAST
  | typeof GPU_SPAN_NAME_BEAM_HIT
  | typeof GPU_SPAN_NAME_BEAM
  | typeof GPU_SPAN_NAME_GRID_CELL
  | typeof GPU_SPAN_NAME_SAMPLE_HIT
  | typeof GPU_SPAN_NAME_SAMPLE
  | typeof GPU_SPAN_NAME_RASTER_BOX
  | typeof GPU_SPAN_NAME_SRASTER
  | typeof GPU_SPAN_NAME_PX
  | typeof GPU_SPAN_NAME_HOLE
  | typeof GPU_SPAN_NAME_LIGHT_WORLD_RECEIVERS
  | typeof GPU_SPAN_NAME_LIGHT_DRAWS
  | typeof GPU_SPAN_NAME_LIGHT_SLIDE
  | typeof GPU_SPAN_NAME_LIGHT_PROPAGATE
  | typeof GPU_SPAN_NAME_LIGHT_DAMAGE
  | typeof GPU_SPAN_NAME_LIGHT_BEAM_HIT
  | typeof GPU_SPAN_NAME_LIGHT_BEAM
  | typeof GPU_SPAN_NAME_LIGHT_SAMPLE
  | typeof GPU_SPAN_NAME_LIGHT_RASTER
  | typeof GPU_SPAN_NAME_LIGHT_SLOT
  | typeof GPU_SPAN_NAME_LIGHT_HIT
  | typeof GPU_SPAN_NAME_LIGHT_WALK
  | typeof GPU_SPAN_NAME_GENERATE
  | typeof GPU_SPAN_NAME_SHADOW
  | typeof GPU_SPAN_NAME_FACE
  | typeof GPU_SPAN_NAME_DAMAGE
  | typeof GPU_SPAN_NAME_SHADOW_DAMAGE
  | typeof GPU_SPAN_NAME_PROPAGATE;
export const GPU_SPAN_NAME_AIR = 0;
export const GPU_SPAN_NAME_EXPOSURE = 1;
export const GPU_SPAN_NAME_AO = 2;
export const GPU_SPAN_NAME_WORLD = 3;
export const GPU_SPAN_NAME_WORLD_REST = 4;
export const GPU_SPAN_NAME_RESOLVE = 5;
export const GPU_SPAN_NAME_FINISH = 6;
export const GPU_SPAN_NAME_MARKER = 7;
export const GPU_SPAN_NAME_DEBUG = 8;
export const GPU_SPAN_NAME_CHARACTER = 9;
export const GPU_SPAN_NAME_CAST = 10;
export const GPU_SPAN_NAME_BEAM_HIT = 11;
export const GPU_SPAN_NAME_BEAM = 12;
export const GPU_SPAN_NAME_GRID_CELL = 13;
export const GPU_SPAN_NAME_SAMPLE_HIT = 14;
export const GPU_SPAN_NAME_SAMPLE = 15;
export const GPU_SPAN_NAME_RASTER_BOX = 16;
export const GPU_SPAN_NAME_SRASTER = 17;
export const GPU_SPAN_NAME_PX = 18;
export const GPU_SPAN_NAME_HOLE = 19;
export const GPU_SPAN_NAME_LIGHT_WORLD_RECEIVERS = 20;
export const GPU_SPAN_NAME_LIGHT_DRAWS = 21;
export const GPU_SPAN_NAME_LIGHT_SLIDE = 22;
export const GPU_SPAN_NAME_LIGHT_PROPAGATE = 23;
export const GPU_SPAN_NAME_LIGHT_DAMAGE = 24;
export const GPU_SPAN_NAME_LIGHT_BEAM_HIT = 25;
export const GPU_SPAN_NAME_LIGHT_BEAM = 26;
export const GPU_SPAN_NAME_LIGHT_SAMPLE = 27;
export const GPU_SPAN_NAME_LIGHT_RASTER = 28;
export const GPU_SPAN_NAME_LIGHT_SLOT = 29;
export const GPU_SPAN_NAME_LIGHT_HIT = 30;
export const GPU_SPAN_NAME_LIGHT_WALK = 31;
export const GPU_SPAN_NAME_GENERATE = 32;
export const GPU_SPAN_NAME_SHADOW = 33;
export const GPU_SPAN_NAME_FACE = 34;
export const GPU_SPAN_NAME_DAMAGE = 35;
export const GPU_SPAN_NAME_SHADOW_DAMAGE = 36;
export const GPU_SPAN_NAME_PROPAGATE = 37;

/** The member `val` is, or null for a number no member has. */
export function of_GpuSpanName(val: number): GpuSpanName | null {
  return val === GPU_SPAN_NAME_AIR ||
    val === GPU_SPAN_NAME_EXPOSURE ||
    val === GPU_SPAN_NAME_AO ||
    val === GPU_SPAN_NAME_WORLD ||
    val === GPU_SPAN_NAME_WORLD_REST ||
    val === GPU_SPAN_NAME_RESOLVE ||
    val === GPU_SPAN_NAME_FINISH ||
    val === GPU_SPAN_NAME_MARKER ||
    val === GPU_SPAN_NAME_DEBUG ||
    val === GPU_SPAN_NAME_CHARACTER ||
    val === GPU_SPAN_NAME_CAST ||
    val === GPU_SPAN_NAME_BEAM_HIT ||
    val === GPU_SPAN_NAME_BEAM ||
    val === GPU_SPAN_NAME_GRID_CELL ||
    val === GPU_SPAN_NAME_SAMPLE_HIT ||
    val === GPU_SPAN_NAME_SAMPLE ||
    val === GPU_SPAN_NAME_RASTER_BOX ||
    val === GPU_SPAN_NAME_SRASTER ||
    val === GPU_SPAN_NAME_PX ||
    val === GPU_SPAN_NAME_HOLE ||
    val === GPU_SPAN_NAME_LIGHT_WORLD_RECEIVERS ||
    val === GPU_SPAN_NAME_LIGHT_DRAWS ||
    val === GPU_SPAN_NAME_LIGHT_SLIDE ||
    val === GPU_SPAN_NAME_LIGHT_PROPAGATE ||
    val === GPU_SPAN_NAME_LIGHT_DAMAGE ||
    val === GPU_SPAN_NAME_LIGHT_BEAM_HIT ||
    val === GPU_SPAN_NAME_LIGHT_BEAM ||
    val === GPU_SPAN_NAME_LIGHT_SAMPLE ||
    val === GPU_SPAN_NAME_LIGHT_RASTER ||
    val === GPU_SPAN_NAME_LIGHT_SLOT ||
    val === GPU_SPAN_NAME_LIGHT_HIT ||
    val === GPU_SPAN_NAME_LIGHT_WALK ||
    val === GPU_SPAN_NAME_GENERATE ||
    val === GPU_SPAN_NAME_SHADOW ||
    val === GPU_SPAN_NAME_FACE ||
    val === GPU_SPAN_NAME_DAMAGE ||
    val === GPU_SPAN_NAME_SHADOW_DAMAGE ||
    val === GPU_SPAN_NAME_PROPAGATE
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_GpuSpanName(val: GpuSpanName): string {
  switch (val) {
    case GPU_SPAN_NAME_AIR:
      return "air";
    case GPU_SPAN_NAME_EXPOSURE:
      return "exposure";
    case GPU_SPAN_NAME_AO:
      return "ao";
    case GPU_SPAN_NAME_WORLD:
      return "world";
    case GPU_SPAN_NAME_WORLD_REST:
      return "world_rest";
    case GPU_SPAN_NAME_RESOLVE:
      return "resolve";
    case GPU_SPAN_NAME_FINISH:
      return "finish";
    case GPU_SPAN_NAME_MARKER:
      return "marker";
    case GPU_SPAN_NAME_DEBUG:
      return "debug";
    case GPU_SPAN_NAME_CHARACTER:
      return "character";
    case GPU_SPAN_NAME_CAST:
      return "cast";
    case GPU_SPAN_NAME_BEAM_HIT:
      return "beam_hit";
    case GPU_SPAN_NAME_BEAM:
      return "beam";
    case GPU_SPAN_NAME_GRID_CELL:
      return "grid_cell";
    case GPU_SPAN_NAME_SAMPLE_HIT:
      return "sample_hit";
    case GPU_SPAN_NAME_SAMPLE:
      return "sample";
    case GPU_SPAN_NAME_RASTER_BOX:
      return "raster_box";
    case GPU_SPAN_NAME_SRASTER:
      return "sraster";
    case GPU_SPAN_NAME_PX:
      return "px";
    case GPU_SPAN_NAME_HOLE:
      return "hole";
    case GPU_SPAN_NAME_LIGHT_WORLD_RECEIVERS:
      return "light_world_receivers";
    case GPU_SPAN_NAME_LIGHT_DRAWS:
      return "light_draws";
    case GPU_SPAN_NAME_LIGHT_SLIDE:
      return "light_slide";
    case GPU_SPAN_NAME_LIGHT_PROPAGATE:
      return "light_propagate";
    case GPU_SPAN_NAME_LIGHT_DAMAGE:
      return "light_damage";
    case GPU_SPAN_NAME_LIGHT_BEAM_HIT:
      return "light_beam_hit";
    case GPU_SPAN_NAME_LIGHT_BEAM:
      return "light_beam";
    case GPU_SPAN_NAME_LIGHT_SAMPLE:
      return "light_sample";
    case GPU_SPAN_NAME_LIGHT_RASTER:
      return "light_raster";
    case GPU_SPAN_NAME_LIGHT_SLOT:
      return "light_slot";
    case GPU_SPAN_NAME_LIGHT_HIT:
      return "light_hit";
    case GPU_SPAN_NAME_LIGHT_WALK:
      return "light_walk";
    case GPU_SPAN_NAME_GENERATE:
      return "generate";
    case GPU_SPAN_NAME_SHADOW:
      return "shadow";
    case GPU_SPAN_NAME_FACE:
      return "face";
    case GPU_SPAN_NAME_DAMAGE:
      return "damage";
    case GPU_SPAN_NAME_SHADOW_DAMAGE:
      return "shadow_damage";
    case GPU_SPAN_NAME_PROPAGATE:
      return "propagate";
  }
}

/** A span is among `GPU_SPAN_PER_PX` where every pass it times runs threads over the world's drawn pixels, its fragments or a thread a pixel or a square of them, so its cost goes with the drawn resolution; a span whose passes do not all is split until each is one or the other. Among them: the world's passes, its occlusion, its gathering and the depth's bound (`resolve`), the boxes drawn into the visibility buffer (`sraster`), the pixels' cast and their holes' walk (`px`, `hole`), the screen's damage, its tiles' words set and the damage drawn over the world's depth (`damage`), the light's changed tiles propagated to the screen's, a thread a square of its pixels (`propagate`), and the tiles of the light's box the pixels the eye walked apart stand on, a thread each (`light_world_receivers`); the drawn resolution's scale is judged on these alone (pxl-render renderer.rs `px_ms`, pxl-core domain/scene/resolution.rs). Not among them: the camera grid's beam rays, cells, samples, their boxes written, each stood on by the light's receivers where they stand (`raster_box`), and their faces, on its own angular lattice (`beam`, `grid_cell`, `sample`, `face`); the light's cast, a span a stage: the tiles the drawn receivers stand on (`light_draws`), the slide (`light_slide`), the tiles the stood tiles read (`light_propagate`), the tiles wanted and set to be cast again (`light_damage`), its beam rays' walks over the node cache (`light_beam_hit`) and on asking the world with the cells' starts (`light_beam`), its samples (`light_sample`), their boxes (`light_raster`), its slots (`light_slot`) and their walks (`light_walk`), at the map's texels and tiles; the shadow map's damage, its tiles' words set, the damage drawn over the map and its rect's args (`shadow_damage`), at the map's texels; the node cache's forgetting, over its entries (`cast`); and the canvas's passes, at the display's pixels, the markers' text among them (`finish`, `marker`, `debug`). */
export const GPU_SPAN_PER_PX = [
  2 as GpuSpanName,
  3 as GpuSpanName,
  4 as GpuSpanName,
  5 as GpuSpanName,
  17 as GpuSpanName,
  18 as GpuSpanName,
  19 as GpuSpanName,
  20 as GpuSpanName,
  35 as GpuSpanName,
  37 as GpuSpanName,
] as const;

/** What changed that cast the light's map, a bit each, bit k the k-th (pxl-render ray/cast.rs `light_changed`): what it reads, the light's frame, the eye's zooms its bands are held to, the shadows' setting, or the world under it where the node cache forgets it whole or more columns of it changed than a cast is handed (`content`); the box slid (`slide`); which tiles the receivers read, the eye's place and look, the characters' places or decor compacted again (`receivers`); the footprints owed the first frame the eye stands in after it moved (`footprint`); or columns of the world under it changed, whose tiles alone are cast again (`world`, pxl-engine `Light::reaches`). */
export type LightChanged =
  | typeof LIGHT_CHANGED_CONTENT
  | typeof LIGHT_CHANGED_SLIDE
  | typeof LIGHT_CHANGED_RECEIVERS
  | typeof LIGHT_CHANGED_FOOTPRINT
  | typeof LIGHT_CHANGED_WORLD;
export const LIGHT_CHANGED_CONTENT = 0;
export const LIGHT_CHANGED_SLIDE = 1;
export const LIGHT_CHANGED_RECEIVERS = 2;
export const LIGHT_CHANGED_FOOTPRINT = 3;
export const LIGHT_CHANGED_WORLD = 4;

/** The member `val` is, or null for a number no member has. */
export function of_LightChanged(val: number): LightChanged | null {
  return val === LIGHT_CHANGED_CONTENT ||
    val === LIGHT_CHANGED_SLIDE ||
    val === LIGHT_CHANGED_RECEIVERS ||
    val === LIGHT_CHANGED_FOOTPRINT ||
    val === LIGHT_CHANGED_WORLD
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_LightChanged(val: LightChanged): string {
  switch (val) {
    case LIGHT_CHANGED_CONTENT:
      return "content";
    case LIGHT_CHANGED_SLIDE:
      return "slide";
    case LIGHT_CHANGED_RECEIVERS:
      return "receivers";
    case LIGHT_CHANGED_FOOTPRINT:
      return "footprint";
    case LIGHT_CHANGED_WORLD:
      return "world";
  }
}

/** What the renderer made of a frame, where it decides it (pxl-render renderer.rs `render_frame`, `FrameKind`): `none` before it was asked; `unready` with no device or surface, or while the renderer is made before its first frame (`PipelineCache::share`); `dropped` where the frame did not fit the GPU stream; `plain` drawn with nothing cast; `still` drawn with the eye's rays as the last cast left them, the view and the world still; `cast` drawn with a cast, where the view or the world moved; `unchanged` where nothing the frame reads changed, so nothing is drawn or submitted and the canvases keep what they show; `px` drawn with the pixels cast again over samples that stood, where only the eye's jitter moved once its rays rest (pxl-engine `eye::JITTER`); `masked` drawn through the stencil's mask where only what the core bounds moved: the damaged screen tiles alone drawn again, the rest as they were (pxl-render gpu/tile.rs `MASK_NUMBER_CNT`, screen_tile.wgsl). */
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

export type FrameKind =
  | typeof FRAME_KIND_NONE
  | typeof FRAME_KIND_UNREADY
  | typeof FRAME_KIND_DROPPED
  | typeof FRAME_KIND_PLAIN
  | typeof FRAME_KIND_STILL
  | typeof FRAME_KIND_CAST
  | typeof FRAME_KIND_UNCHANGED
  | typeof FRAME_KIND_PX
  | typeof FRAME_KIND_MASKED;
export const FRAME_KIND_NONE = 0;
export const FRAME_KIND_UNREADY = 1;
export const FRAME_KIND_DROPPED = 2;
export const FRAME_KIND_PLAIN = 3;
export const FRAME_KIND_STILL = 4;
export const FRAME_KIND_CAST = 5;
export const FRAME_KIND_UNCHANGED = 6;
export const FRAME_KIND_PX = 7;
export const FRAME_KIND_MASKED = 8;

/** The member `val` is, or null for a number no member has. */
export function of_FrameKind(val: number): FrameKind | null {
  return val === FRAME_KIND_NONE ||
    val === FRAME_KIND_UNREADY ||
    val === FRAME_KIND_DROPPED ||
    val === FRAME_KIND_PLAIN ||
    val === FRAME_KIND_STILL ||
    val === FRAME_KIND_CAST ||
    val === FRAME_KIND_UNCHANGED ||
    val === FRAME_KIND_PX ||
    val === FRAME_KIND_MASKED
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_FrameKind(val: FrameKind): string {
  switch (val) {
    case FRAME_KIND_NONE:
      return "none";
    case FRAME_KIND_UNREADY:
      return "unready";
    case FRAME_KIND_DROPPED:
      return "dropped";
    case FRAME_KIND_PLAIN:
      return "plain";
    case FRAME_KIND_STILL:
      return "still";
    case FRAME_KIND_CAST:
      return "cast";
    case FRAME_KIND_UNCHANGED:
      return "unchanged";
    case FRAME_KIND_PX:
      return "px";
    case FRAME_KIND_MASKED:
      return "masked";
  }
}
