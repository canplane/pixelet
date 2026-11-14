// @pxl/client/abi/schema.spec.ts
//
// GENERATED from spec/client/core/domain/setting/schema.json (xpute-gen data.rs)
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
// development one; the UI's production bundle carries SETTING_PROD,
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

/** Which controller makes the camera's pose.
 *
 *   orbit        — third person: the camera turns about the pivot, its
 *                  radius ahead of the eye, and a drag moves the pivot.
 *   first_person — the eye is the pivot and turns in place; a drag
 *                  walks it as the orbit's pivot walks, the eye at its
 *                  height over where the walk stands. */
export type CameraMode =
  typeof CAMERA_MODE_ORBIT | typeof CAMERA_MODE_FIRST_PERSON;
export const CAMERA_MODE_ORBIT = 0;
export const CAMERA_MODE_FIRST_PERSON = 1;

/** The member `val` is, or null for a number no member has. */
export function of_CameraMode(val: number): CameraMode | null {
  return val === CAMERA_MODE_ORBIT || val === CAMERA_MODE_FIRST_PERSON
    ? val
    : null;
}

/** What terrain does to the camera.
 *
 *   through — nothing. The eye goes wherever the controls put it, ground
 *             included, where its rays start inside what they meet and
 *             the scene reads inside out.
 *   stop    — a control goes as far as it asked or as far as it can.
 *             Nothing moves on its own; a zoom into a hill stops short.
 *   climb   — dist is granted and the *view* pitches up instead, as far
 *             as straight down, at the cost of an angle nobody asked for.
 *             Where its way hits something all the same, a zoom in goes
 *             through to the pivot's side and any other control stops. */
export type OrbitObstacleMode =
  | typeof ORBIT_OBSTACLE_MODE_THROUGH
  | typeof ORBIT_OBSTACLE_MODE_STOP
  | typeof ORBIT_OBSTACLE_MODE_CLIMB;
export const ORBIT_OBSTACLE_MODE_THROUGH = 0;
export const ORBIT_OBSTACLE_MODE_STOP = 1;
export const ORBIT_OBSTACLE_MODE_CLIMB = 2;

/** The member `val` is, or null for a number no member has. */
export function of_OrbitObstacleMode(val: number): OrbitObstacleMode | null {
  return val === ORBIT_OBSTACLE_MODE_THROUGH ||
    val === ORBIT_OBSTACLE_MODE_STOP ||
    val === ORBIT_OBSTACLE_MODE_CLIMB
    ? val
    : null;
}

/** Where the ambient occlusion comes from. Either is the sky's visibility
 * and darkens only the light from the sky and its bounce, never the direct
 * sun or moon, which the shadows are its visibility of; neither tints
 * (rust/client/render/src/frame/shader/atmos_light.wgsl `surface_light_in`).
 *
 *   off    — none.
 *   corner — the screen pass's own: the floor along a cube and the inside
 *            of a corner, where a neighboring pixel's face rises in front
 *            of the pixel's own plane within a cell or two of it
 *            (rust/client/render/src/frame/shader/screen_ray.wgsl).
 *            A few taps of the rays a pixel, behind a uniform.
 *   screen — N8AO
 *            (rust/client/render/src/pass/ao.rs): the world's depth drawn
 *            once more, the occlusion sampled from it at half resolution,
 *            denoised, and read by every lit surface. Five passes and 11
 *            bytes a pixel. The corners' too, the darker of the two where
 *            both fall (screen_shade.wgsl), as N8AO reads a cube's corner
 *            softer at half resolution. */
export type AoMode =
  typeof AO_MODE_OFF | typeof AO_MODE_CORNER | typeof AO_MODE_SCREEN;
export const AO_MODE_OFF = 0;
export const AO_MODE_CORNER = 1;
export const AO_MODE_SCREEN = 2;

/** The member `val` is, or null for a number no member has. */
export function of_AoMode(val: number): AoMode | null {
  return val === AO_MODE_OFF || val === AO_MODE_CORNER || val === AO_MODE_SCREEN
    ? val
    : null;
}

/** How many device pixels the world is drawn at to a CSS pixel, and the rate
 * frames go at.
 *
 *   auto  — a frame each whole number of vsync intervals, to RATE_MIN_HZ;
 *           the display's pixels, stepped down to a share of them, while
 *           moving frames' GPU time runs over their budget — the frame's
 *           target interval — and back up once the step above would fit, a
 *           budget control as congestion control is
 *           (rust/client/core/src/domain/scene/resolution.rs).
 *   fixed — the display's, at its rate, whatever the frames cost.
 *   low   — half the display's each way, auto's floor (RES_MIN), at the
 *           display's rate. */
export type ResMode =
  typeof RES_MODE_AUTO | typeof RES_MODE_FIXED | typeof RES_MODE_LOW;
export const RES_MODE_AUTO = 0;
export const RES_MODE_FIXED = 1;
export const RES_MODE_LOW = 2;

/** The member `val` is, or null for a number no member has. */
export function of_ResMode(val: number): ResMode | null {
  return val === RES_MODE_AUTO || val === RES_MODE_FIXED || val === RES_MODE_LOW
    ? val
    : null;
}

/** The hour the sky is drawn at, at the place the sky is seen from: the
 * wall clock's own, or one held in the place's solar time, so the sun
 * stands where it does there at that hour today whatever the clock reads
 * (pxl-core domain/scene/environment.rs `shown`). Only the sky's time;
 * the clouds, the frame's step and the storage keep the wall clock.
 *
 *   real      — the wall clock.
 *   dawn      — 7:00, a low sun.
 *   morning   — 9:00.
 *   noon      — 12:00, the sun highest.
 *   afternoon — 15:00.
 *   dusk      — 17:00, a low sun. */
export type SkyTime =
  | typeof SKY_TIME_REAL
  | typeof SKY_TIME_DAWN
  | typeof SKY_TIME_MORNING
  | typeof SKY_TIME_NOON
  | typeof SKY_TIME_AFTERNOON
  | typeof SKY_TIME_DUSK;
export const SKY_TIME_REAL = 0;
export const SKY_TIME_DAWN = 1;
export const SKY_TIME_MORNING = 2;
export const SKY_TIME_NOON = 3;
export const SKY_TIME_AFTERNOON = 4;
export const SKY_TIME_DUSK = 5;

/** The member `val` is, or null for a number no member has. */
export function of_SkyTime(val: number): SkyTime | null {
  return val === SKY_TIME_REAL ||
    val === SKY_TIME_DAWN ||
    val === SKY_TIME_MORNING ||
    val === SKY_TIME_NOON ||
    val === SKY_TIME_AFTERNOON ||
    val === SKY_TIME_DUSK
    ? val
    : null;
}

/** Every setting, by its number in schema.json's SETTING. */
export type SettingKey =
  | typeof SETTING_KEY_TEXTURE
  | typeof SETTING_KEY_DECOR
  | typeof SETTING_KEY_CLOUD
  | typeof SETTING_KEY_PLACE
  | typeof SETTING_KEY_TILT_LOCK
  | typeof SETTING_KEY_CAMERA_MODE
  | typeof SETTING_KEY_SURFACE_FOLLOW
  | typeof SETTING_KEY_ORBIT_OBSTACLE
  | typeof SETTING_KEY_EDGE_SHADE
  | typeof SETTING_KEY_RIM_LIGHT
  | typeof SETTING_KEY_AO
  | typeof SETTING_KEY_SHADOW
  | typeof SETTING_KEY_RES
  | typeof SETTING_KEY_VIEW
  | typeof SETTING_KEY_PERF
  | typeof SETTING_KEY_WORLD_GIZMO
  | typeof SETTING_KEY_CAMERA_GIZMO
  | typeof SETTING_KEY_PERSON
  | typeof SETTING_KEY_PET
  | typeof SETTING_KEY_RAY_STEP
  | typeof SETTING_KEY_RAY_CNT
  | typeof SETTING_KEY_SKY_TIME;
/** Ground color from the art pack (a tap of the layer array per
 * fragment) instead of the flat per-instance average. The paint shader's
 * `frame.low_spec` is this, inverted.
 *
 * Low because the tap is all of it: fill rate, which scales with
 * the screen and not with the world, and nothing else — no geometry, no
 * pass of its own, and no work off the GPU past an atlas the client
 * already holds. */
export const SETTING_KEY_TEXTURE = 0;
/** Procedurally scattered decor only; what the player placed stays.
 *
 * Med because it is models rather than a shading term: the GPU generates the
 * ground's decor where the eye's clip levels are (decor_pose_cs) and the frame draws
 * each kind as a model, an impostor or nothing by the angle it
 * spans, so what it costs follows what is in view. The badge is an ordering,
 * and this is the one to reach for before any shading term. */
export const SETTING_KEY_DECOR = 1;
/** The cumulus' models and the altocumulus and cirrus sheets
 * (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/pass/cloud.rs).
 * Med as decor is: models in view, and a pass over the screen for the sheets. */
export const SETTING_KEY_CLOUD = 2;
/** The places' markers over the world, from the published places tiles. */
export const SETTING_KEY_PLACE = 3;
/** Rotate-drag and pinch stop writing polar; azimuth is unaffected.
 * Off by default: the tilt is the one control a view from above is expected to
 * have, and locking it reads as the app being stuck. */
export const SETTING_KEY_TILT_LOCK = 4;
/** On a switch to first person the person stands at the pivot, the eye at its
 * height over the ground. On a switch to third person the pivot is laid on the
 * ground and the eye steps back by the radius. */
export const SETTING_KEY_CAMERA_MODE = 5;
/** The pivot stands on what supports it and a pan takes the lowest way over what is in it (control/support.rs). Off, a pan keeps the pivot's height and passes through anything; a tap still stands it on the surface it struck. */
export const SETTING_KEY_SURFACE_FOLLOW = 6;
/** Only `climb` goes into a tunnel (camera/orbit.rs `dive`), so a shipped build holds it, and the others are a development build's to compare, each session from the default. */
export const SETTING_KEY_ORBIT_OBSTACLE = 7;
/** Darkens cube edges that border open air (rust/client/render/src/frame/shader/paint.wgsl).
 * A few fragment ops behind a uniform; the shader is the same either way. */
export const SETTING_KEY_EDGE_SHADE = 8;
/** A uniform on an already-compiled shader term; not a pass. */
export const SETTING_KEY_RIM_LIGHT = 9;
export const SETTING_KEY_AO = 10;
/** The shadow map (rust/client/render/src/pass/shadow.rs): the world cast
 * once more from the sun, or the moon where the sun is down, as the eye's
 * rays are, none where neither is up; beams and samples on
 * the node cache, the boxes they stop at drawn and a texel's sample
 * taken, the rest walked (pxl-render queue.wgsl), recast only where what it
 * reads moves; decor and the characters drawn into it depth only, and a 3x3
 * compare per fragment; 16 MB of map. */
export const SETTING_KEY_SHADOW = 11;
/** The pixels the world is drawn at: two to a CSS pixel is four times the
 * work done a drawn pixel at a time (span.json `GPU_SPAN_PER_PX`), and nothing
 * more; the camera grid's rays and the light's map cost the same at any.
 * The markers and the finish stay at the display's. */
export const SETTING_KEY_RES = 12;
/** Where the camera and the pointer are, read live (perf/read.rs, NOW's view section). */
export const SETTING_KEY_VIEW = 13;
/** The performance panel, recording while it is on (perf/window.rs `perf_follow`). */
export const SETTING_KEY_PERF = 14;
/** Marks for the pivot and the cell it floors to, and rings in the level, the view's upright and the north–south planes (domain/scene/gizmo/frame.rs); and over the ground the eye's windows' bricks by how they stand, their tiles and shards (domain/scene/gizmo/coverage.rs). */
export const SETTING_KEY_WORLD_GIZMO = 15;
/** The camera grid over the screen, the cells the rays are spaced by, and the cells the rays stop on tinted by the share of a grid cell their faces show the eye (rust/client/render/src/frame/shader/camera_grid.wgsl, screen_shade.wgsl `cell_share`), in the looks debug_look.json gives them. */
export const SETTING_KEY_CAMERA_GIZMO = 16;
export const SETTING_KEY_PERSON = 17;
export const SETTING_KEY_PET = 18;
/** Each ray's pixel colored by the steps taken on its way from the eye, as the frame casts it, on debug_look.json's log ramp: the steps of the beam ray its cell starts off (beam::LINE_STEPS) and its own after, or, where it took a voxel or the sky without a walk, those of the sample before it on the camera grid's lattice, counted the same way (beam::SAMPLE_STEPS_SHIFT; rust/client/render/src/frame/shader/cast.wgsl `taken_word`, queue.wgsl `finish`). */
export const SETTING_KEY_RAY_STEP = 19;
/** Counts what the eye's cast and the sun's shadow map do: the nodes each pass's walks asked, the pixels or slots that took the visible sample or walked, and the boxes drawn. The GPU counts them and writes them into the profiler's ray lanes of the frame that counted them (rust/client/engine/src/count.rs, rust/client/render/src/ray/count.rs). The counting's atomics run inside the cast's spans, so time the cast with this off. */
export const SETTING_KEY_RAY_CNT = 20;
export const SETTING_KEY_SKY_TIME = 21;

/** The member `val` is, or null for a number no member has. */
export function of_SettingKey(val: number): SettingKey | null {
  return val === SETTING_KEY_TEXTURE ||
    val === SETTING_KEY_DECOR ||
    val === SETTING_KEY_CLOUD ||
    val === SETTING_KEY_PLACE ||
    val === SETTING_KEY_TILT_LOCK ||
    val === SETTING_KEY_CAMERA_MODE ||
    val === SETTING_KEY_SURFACE_FOLLOW ||
    val === SETTING_KEY_ORBIT_OBSTACLE ||
    val === SETTING_KEY_EDGE_SHADE ||
    val === SETTING_KEY_RIM_LIGHT ||
    val === SETTING_KEY_AO ||
    val === SETTING_KEY_SHADOW ||
    val === SETTING_KEY_RES ||
    val === SETTING_KEY_VIEW ||
    val === SETTING_KEY_PERF ||
    val === SETTING_KEY_WORLD_GIZMO ||
    val === SETTING_KEY_CAMERA_GIZMO ||
    val === SETTING_KEY_PERSON ||
    val === SETTING_KEY_PET ||
    val === SETTING_KEY_RAY_STEP ||
    val === SETTING_KEY_RAY_CNT ||
    val === SETTING_KEY_SKY_TIME
    ? val
    : null;
}

export const SETTING = {
  /** Ground color from the art pack (a tap of the layer array per
   * fragment) instead of the flat per-instance average. The paint shader's
   * `frame.low_spec` is this, inverted.
   *
   * Low because the tap is all of it: fill rate, which scales with
   * the screen and not with the world, and nothing else — no geometry, no
   * pass of its own, and no work off the GPU past an atlas the client
   * already holds. */
  texture: {
    key: 0 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "low",
  },
  /** Procedurally scattered decor only; what the player placed stays.
   *
   * Med because it is models rather than a shading term: the GPU generates the
   * ground's decor where the eye's clip levels are (decor_pose_cs) and the frame draws
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
   * (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/pass/cloud.rs).
   * Med as decor is: models in view, and a pass over the screen for the sheets. */
  cloud: {
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
  /** On a switch to first person the person stands at the pivot, the eye at its
   * height over the ground. On a switch to third person the pivot is laid on the
   * ground and the eye steps back by the radius. */
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
  /** Only `climb` goes into a tunnel (camera/orbit.rs `dive`), so a shipped build holds it, and the others are a development build's to compare, each session from the default. */
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
  /** The shadow map (rust/client/render/src/pass/shadow.rs): the world cast
   * once more from the sun, or the moon where the sun is down, as the eye's
   * rays are, none where neither is up; beams and samples on
   * the node cache, the boxes they stop at drawn and a texel's sample
   * taken, the rest walked (pxl-render queue.wgsl), recast only where what it
   * reads moves; decor and the characters drawn into it depth only, and a 3x3
   * compare per fragment; 16 MB of map. */
  shadow: {
    key: 11 as number,
    kind: "toggle",
    default: false,
    cost: "high",
    section: "lighting",
  },
  /** The pixels the world is drawn at: two to a CSS pixel is four times the
   * work done a drawn pixel at a time (span.json `GPU_SPAN_PER_PX`), and nothing
   * more; the camera grid's rays and the light's map cost the same at any.
   * The markers and the finish stay at the display's. */
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
  /** The camera grid over the screen, the cells the rays are spaced by, and the cells the rays stop on tinted by the share of a grid cell their faces show the eye (rust/client/render/src/frame/shader/camera_grid.wgsl, screen_shade.wgsl `cell_share`), in the looks debug_look.json gives them. */
  camera_gizmo: {
    key: 16 as number,
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
  /** Each ray's pixel colored by the steps taken on its way from the eye, as the frame casts it, on debug_look.json's log ramp: the steps of the beam ray its cell starts off (beam::LINE_STEPS) and its own after, or, where it took a voxel or the sky without a walk, those of the sample before it on the camera grid's lattice, counted the same way (beam::SAMPLE_STEPS_SHIFT; rust/client/render/src/frame/shader/cast.wgsl `taken_word`, queue.wgsl `finish`). */
  ray_step: {
    key: 19 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
  /** Counts what the eye's cast and the sun's shadow map do: the nodes each pass's walks asked, the pixels or slots that took the visible sample or walked, and the boxes drawn. The GPU counts them and writes them into the profiler's ray lanes of the frame that counted them (rust/client/engine/src/count.rs, rust/client/render/src/ray/count.rs). The counting's atomics run inside the cast's spans, so time the cast with this off. */
  ray_cnt: {
    key: 20 as number,
    kind: "toggle",
    default: false,
    section: "perf",
    dev: true,
  },
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
export const SETTING_PROD = {
  /** Ground color from the art pack (a tap of the layer array per
   * fragment) instead of the flat per-instance average. The paint shader's
   * `frame.low_spec` is this, inverted.
   *
   * Low because the tap is all of it: fill rate, which scales with
   * the screen and not with the world, and nothing else — no geometry, no
   * pass of its own, and no work off the GPU past an atlas the client
   * already holds. */
  texture: {
    key: 0 as number,
    kind: "toggle",
    default: true,
    section: "display",
    cost: "low",
  },
  /** Procedurally scattered decor only; what the player placed stays.
   *
   * Med because it is models rather than a shading term: the GPU generates the
   * ground's decor where the eye's clip levels are (decor_pose_cs) and the frame draws
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
   * (rust/client/core/src/domain/scene/environment/cloud.rs, rust/client/render/src/pass/cloud.rs).
   * Med as decor is: models in view, and a pass over the screen for the sheets. */
  cloud: {
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
  /** On a switch to first person the person stands at the pivot, the eye at its
   * height over the ground. On a switch to third person the pivot is laid on the
   * ground and the eye steps back by the radius. */
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
  /** The shadow map (rust/client/render/src/pass/shadow.rs): the world cast
   * once more from the sun, or the moon where the sun is down, as the eye's
   * rays are, none where neither is up; beams and samples on
   * the node cache, the boxes they stop at drawn and a texel's sample
   * taken, the rest walked (pxl-render queue.wgsl), recast only where what it
   * reads moves; decor and the characters drawn into it depth only, and a 3x3
   * compare per fragment; 16 MB of map. */
  shadow: {
    key: 11 as number,
    kind: "toggle",
    default: false,
    cost: "high",
    section: "lighting",
  },
  /** The pixels the world is drawn at: two to a CSS pixel is four times the
   * work done a drawn pixel at a time (span.json `GPU_SPAN_PER_PX`), and nothing
   * more; the camera grid's rays and the light's map cost the same at any.
   * The markers and the finish stay at the display's. */
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
  /** The camera grid over the screen, the cells the rays are spaced by, and the cells the rays stop on tinted by the share of a grid cell their faces show the eye (rust/client/render/src/frame/shader/camera_grid.wgsl, screen_shade.wgsl `cell_share`), in the looks debug_look.json gives them. */
  camera_gizmo: {
    key: 16 as number,
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
  /** Each ray's pixel colored by the steps taken on its way from the eye, as the frame casts it, on debug_look.json's log ramp: the steps of the beam ray its cell starts off (beam::LINE_STEPS) and its own after, or, where it took a voxel or the sky without a walk, those of the sample before it on the camera grid's lattice, counted the same way (beam::SAMPLE_STEPS_SHIFT; rust/client/render/src/frame/shader/cast.wgsl `taken_word`, queue.wgsl `finish`). */
  ray_step: {
    key: 19 as number,
    kind: "toggle",
    default: false,
    section: "debug",
  },
} as const;
