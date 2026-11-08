// @pxl/client/abi/schema.spec.ts
//
// GENERATED from spec/client/core/domain/setting/schema.json
//
// Every setting the app has, declared once: its default, its kind, and
// where it is shown, its `section`: one of the settings sheet's, or
// `perf`, the profiler panel's own. The store reads the defaults from
// here and the sheet renders rows from here, so adding a setting to the
// sheet is one entry in SETTING plus its two strings in the locale catalog
// under `settings.<section>.<key>`. A choice may carry `costs` beside its
// `options`, one per option in the same order, for when what an option costs
// differs from its neighbors'; an empty string is no badge. A `dev` setting
// is in the core's table in every build and offered only under a
// development one; the UI's production bundle carries SETTING_PRODUCTION,
// the table without them, in SETTING's place. A setting whose `stored` is
// false is the session's alone: the store neither writes it to its kept
// slot nor reads it from there, so every session starts it at its default.
//
// A setting carries its `number` (the core's SettingKey), and a choice's
// `options` names one of the lists below, which numbers each option. The
// numbers are what crosses and what storage keeps; the names are the
// UI's alone, for its rows and the locale. A number is given out once;
// the generator refuses one given twice. An `index` is a place in a list
// kept elsewhere — the person and the pet are by theirs in character.json — and
// crosses as itself.

export const SETTING = {
  /** Ground color from the art pack (a tap of the layer array per
   * fragment) instead of the flat per-instance average. The paint shader's
   * `frame.low_spec` is this, inverted.
   *
   * Low because the tap is all of it: fill rate, which scales with
   * the screen and not with the world, and nothing else — no geometry, no
   * pass of its own, and no work off the GPU past an atlas the client
   * already holds. */
  textures: {
    key: 0 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "low",
  },
  /** Procedurally scattered decor only; what the player placed stays.
   *
   * Med because it is models rather than a shading term: the GPU generates the
   * ground's decor where the eye's clip levels are (decor_cs) and the frame draws
   * each kind as a model, an impostor or nothing by the angle it
   * spans, so what it costs follows what is in view. The badge is an ordering,
   * and this is the one to reach for before any shading term. */
  decor: {
    key: 1 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "med",
  },
  /** The cumulus' models and the altocumulus and cirrus sheets
   * (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/gpu/cloud.rs).
   * Med as decor is: models in view, and a pass over the screen for the sheets. */
  clouds: {
    key: 2 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "med",
  },
  /** The places' markers over the world, from the published places tiles. */
  place: {
    key: 3 as number,
    kind: "toggle",
    default: false,
    section: "world",
    cost: "low",
    stored: false,
  },
  /** Rotate-drag and pinch stop writing polar; azimuth is unaffected.
   * Off by default: the tilt is the one control a view from above is expected to
   * have, and locking it reads as the app being stuck. */
  tilt_lock: {
    key: 4 as number,
    kind: "toggle",
    default: false,
    section: "camera",
  },
  /** Which controller makes the camera's pose; CAMERA_MODES says each. To
   * first person the person stands at the pivot, the eye at its
   * height over the ground; to third the pivot is laid on the ground
   * and the eye steps back by the radius. */
  camera_mode: {
    key: 5 as number,
    kind: "choice",
    default: "orbit",
    options: {
      orbit: 0 as number,
      first_person: 1 as number,
    },
    section: "world",
    stored: false,
  },
  /** The pivot stands on what supports it and a pan takes the lowest way over what is in it (control/support.rs). Off, a pan keeps the pivot's height and passes through anything; a tap still stands it on the surface it struck. */
  surface_follow: {
    key: 6 as number,
    kind: "toggle",
    default: true,
    section: "camera",
    dev: true,
  },
  /** What the camera does when the controls would put it underground; ORBIT_OBSTACLE_MODES says each. Only `climb` goes into a tunnel (orbit/collision.rs `dive`), so a shipped build holds it, and the others are a development build's to compare, each session from the default. */
  orbit_obstacle: {
    key: 7 as number,
    kind: "choice",
    default: "climb",
    options: {
      through: 0 as number,
      stop: 1 as number,
      climb: 2 as number,
    },
    section: "camera",
    dev: true,
    stored: false,
  },
  /** Darkens cube edges that border open air (rust/client/render/src/frame/shader/paint.wgsl).
   * A few fragment ops behind a uniform; the shader is the same either way. */
  edge_shade: {
    key: 8 as number,
    kind: "toggle",
    default: true,
    section: "lighting",
    cost: "low",
  },
  /** A uniform on an already-compiled shader term; not a pass. */
  rim_light: {
    key: 9 as number,
    kind: "toggle",
    default: true,
    section: "lighting",
    cost: "low",
  },
  /** corner, the occlusion a voxel world reads off its own cubes' faces;
   * screen, N8AO's screen-space pass with the corners' too, the costliest
   * of the three. AO_MODES says each. */
  ao: {
    key: 10 as number,
    kind: "choice",
    default: "corner",
    options: {
      off: 0 as number,
      corner: 1 as number,
      screen: 2 as number,
    },
    costs: ["", "low", "high"],
    section: "lighting",
  },
  /** The shadow map (rust/client/render/src/gpu/shadow.rs): the world cast
   * once more from the sun, or the moon where the sun is down, as the eye's
   * rays are, none where neither is up; beams and samples on
   * the node cache, the boxes they stop at drawn and a texel's sample
   * taken, the rest walked (pxl-render queue.wgsl), recast only where what it
   * reads moves; decor and the characters drawn into it depth only, and a 3x3
   * compare per fragment; 16 MB of map. */
  shadows: {
    key: 11 as number,
    kind: "toggle",
    default: false,
    cost: "high",
    section: "lighting",
  },
  /** The pixels the world is drawn at: two to a CSS pixel is four times the
   * work done a drawn pixel at a time (span.json `per_px`), and nothing
   * more; the camera grid's rays and the light's map cost the same at any.
   * The markers and the finish stay at the display's. RES_MODES says each. */
  res: {
    key: 12 as number,
    kind: "choice",
    default: "auto",
    options: {
      auto: 0 as number,
      fixed: 1 as number,
      low: 2 as number,
    },
    section: "display",
  },
  /** Where the camera and the pointer are, read live (perf/read.rs, NOW's view section). */
  view: {
    key: 13 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  /** The performance panel, recording while it is on (perf/window.rs `perf_follow`). */
  perf: {
    key: 14 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  /** Marks for the pivot and the cell it floors to, and rings in the level, the view's upright and the north–south planes (domain/scene/gizmo/frame.rs); and over the ground the eye's windows' bricks by how they stand, their tiles and shards (domain/scene/gizmo/coverage.rs). */
  world_gizmo: {
    key: 15 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  person: {
    key: 17 as number,
    kind: "index",
    default: 1 as number,
    section: "character",
  },
  pet: {
    key: 18 as number,
    kind: "index",
    default: 5 as number,
    section: "character",
  },
  /** Counts what the eye's cast and the sun's shadow map do: the nodes each pass's walks asked, the pixels or slots that took the visible sample or walked, and the boxes drawn. The GPU counts them and writes them into the profiler's ray lanes of the frame that counted them (rust/spec/engine/src/count.rs, rust/client/render/src/gpu/ray/count.rs). The counting's atomics run inside the cast's spans, so time the cast with this off. */
  ray_counts: {
    key: 20 as number,
    kind: "toggle",
    default: false,
    section: "perf",
    dev: true,
  },
  /** The hour the sky is drawn at; SKY_TIMES says each. */
  sky_time: {
    key: 21 as number,
    kind: "choice",
    default: "real",
    options: {
      real: 0 as number,
      dawn: 1 as number,
      morning: 2 as number,
      noon: 3 as number,
      afternoon: 4 as number,
      dusk: 5 as number,
    },
    section: "debug",
    dev: true,
    stored: false,
  },
} as const;

/** SETTING without its `dev` entries: what a production bundle carries in its place. */
export const SETTING_PRODUCTION = {
  /** Ground color from the art pack (a tap of the layer array per
   * fragment) instead of the flat per-instance average. The paint shader's
   * `frame.low_spec` is this, inverted.
   *
   * Low because the tap is all of it: fill rate, which scales with
   * the screen and not with the world, and nothing else — no geometry, no
   * pass of its own, and no work off the GPU past an atlas the client
   * already holds. */
  textures: {
    key: 0 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "low",
  },
  /** Procedurally scattered decor only; what the player placed stays.
   *
   * Med because it is models rather than a shading term: the GPU generates the
   * ground's decor where the eye's clip levels are (decor_cs) and the frame draws
   * each kind as a model, an impostor or nothing by the angle it
   * spans, so what it costs follows what is in view. The badge is an ordering,
   * and this is the one to reach for before any shading term. */
  decor: {
    key: 1 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "med",
  },
  /** The cumulus' models and the altocumulus and cirrus sheets
   * (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/gpu/cloud.rs).
   * Med as decor is: models in view, and a pass over the screen for the sheets. */
  clouds: {
    key: 2 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "med",
  },
  /** The places' markers over the world, from the published places tiles. */
  place: {
    key: 3 as number,
    kind: "toggle",
    default: false,
    section: "world",
    cost: "low",
    stored: false,
  },
  /** Rotate-drag and pinch stop writing polar; azimuth is unaffected.
   * Off by default: the tilt is the one control a view from above is expected to
   * have, and locking it reads as the app being stuck. */
  tilt_lock: {
    key: 4 as number,
    kind: "toggle",
    default: false,
    section: "camera",
  },
  /** Which controller makes the camera's pose; CAMERA_MODES says each. To
   * first person the person stands at the pivot, the eye at its
   * height over the ground; to third the pivot is laid on the ground
   * and the eye steps back by the radius. */
  camera_mode: {
    key: 5 as number,
    kind: "choice",
    default: "orbit",
    options: {
      orbit: 0 as number,
      first_person: 1 as number,
    },
    section: "world",
    stored: false,
  },
  /** Darkens cube edges that border open air (rust/client/render/src/frame/shader/paint.wgsl).
   * A few fragment ops behind a uniform; the shader is the same either way. */
  edge_shade: {
    key: 8 as number,
    kind: "toggle",
    default: true,
    section: "lighting",
    cost: "low",
  },
  /** A uniform on an already-compiled shader term; not a pass. */
  rim_light: {
    key: 9 as number,
    kind: "toggle",
    default: true,
    section: "lighting",
    cost: "low",
  },
  /** corner, the occlusion a voxel world reads off its own cubes' faces;
   * screen, N8AO's screen-space pass with the corners' too, the costliest
   * of the three. AO_MODES says each. */
  ao: {
    key: 10 as number,
    kind: "choice",
    default: "corner",
    options: {
      off: 0 as number,
      corner: 1 as number,
      screen: 2 as number,
    },
    costs: ["", "low", "high"],
    section: "lighting",
  },
  /** The shadow map (rust/client/render/src/gpu/shadow.rs): the world cast
   * once more from the sun, or the moon where the sun is down, as the eye's
   * rays are, none where neither is up; beams and samples on
   * the node cache, the boxes they stop at drawn and a texel's sample
   * taken, the rest walked (pxl-render queue.wgsl), recast only where what it
   * reads moves; decor and the characters drawn into it depth only, and a 3x3
   * compare per fragment; 16 MB of map. */
  shadows: {
    key: 11 as number,
    kind: "toggle",
    default: false,
    cost: "high",
    section: "lighting",
  },
  /** The pixels the world is drawn at: two to a CSS pixel is four times the
   * work done a drawn pixel at a time (span.json `per_px`), and nothing
   * more; the camera grid's rays and the light's map cost the same at any.
   * The markers and the finish stay at the display's. RES_MODES says each. */
  res: {
    key: 12 as number,
    kind: "choice",
    default: "auto",
    options: {
      auto: 0 as number,
      fixed: 1 as number,
      low: 2 as number,
    },
    section: "display",
  },
  /** Where the camera and the pointer are, read live (perf/read.rs, NOW's view section). */
  view: {
    key: 13 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  /** The performance panel, recording while it is on (perf/window.rs `perf_follow`). */
  perf: {
    key: 14 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  /** Marks for the pivot and the cell it floors to, and rings in the level, the view's upright and the north–south planes (domain/scene/gizmo/frame.rs); and over the ground the eye's windows' bricks by how they stand, their tiles and shards (domain/scene/gizmo/coverage.rs). */
  world_gizmo: {
    key: 15 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  person: {
    key: 17 as number,
    kind: "index",
    default: 1 as number,
    section: "character",
  },
  pet: {
    key: 18 as number,
    kind: "index",
    default: 5 as number,
    section: "character",
  },
} as const;
