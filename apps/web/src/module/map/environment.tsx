// @pxl/web/module/map/environment.tsx

import { type RefObject, useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { invalidate, useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";

import { render_cfg, shadow_cfg } from "@/module/world/environment/config.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { themes, type WorldTheme, WorldThemeKey } from "@/module/world/environment/theme.ts";
import { useThemeKey } from "@/module/world/environment/theme_clock.ts";
import { camera_cfg } from "@/module/camera/config.ts";
import { SUN_DIR } from "@/module/world/environment/sun_dir.ts";
import { get_moon_position, get_sun_position } from "@/module/world/environment/celestial_position.ts";
import { useFrameRotation } from "@/module/world/frame/use_frame_rotation.ts";
import { useFrameRefs } from "@/module/world/frame/context.tsx";
import { rotate2 } from "@/module/camera/orbit.ts";
import { ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L4_BASE } from "@pxl/shared/spec/space/indexer.ts";
import { useMapStore } from "@/module/map/store.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { gpu_draws_chunks, set_sky_radius } from "@/render/gpu/stage.tsx";

const { FOG_NEAR, FOG_FAR, FAR_CLIP } = render_cfg;
const { MAX_DIST } = camera_cfg.view;

// get_sun_position/get_moon_position track real wall-clock time (a full
// sweep is 24 real hours — celestial_position.ts), so from frame to frame
// the light direction barely moves at all; re-rendering a full shadow map
// (SUN_MAP_SIZE/MOON_MAP_SIZE-texel) every single frame regardless would be
// pure waste. shadow.autoUpdate is turned off per-light (JSX shadow-autoUpdate
// prop below) and shadow.needsUpdate is instead set by hand exactly when the
// light's own direction has actually drifted past this angle since the last
// shadow render — small enough that the shadow's own visible position
// doesn't noticeably lag, large enough that at the real ~0.0042deg/s sweep
// rate this fires roughly once every 10ish seconds during full daylight
// instead of every frame.
const SHADOW_UPDATE_DOT_THRESHOLD = Math.cos((shadow_cfg.UPDATE_ANGLE_DEG * Math.PI) / 180);

// Same Seoul coordinate use_map_bootstrap.ts seeds map_center with before a
// real GPS fix (or that fallback itself) resolves — reused here so the very
// first frame(s), before map_center is available, still get a real location
// instead of computing sun/moon position for (0,0) (equator/prime meridian).
const FALLBACK_LAT = 37.5665;
const FALLBACK_LON = 126.978;

// must stay inside the camera's far clip plane (use_camera.ts sets
// camera.far = FAR_CLIP) or the sky dome gets frustum-clipped — which is
// exactly what was happening before this: the sphere sat at radius 400000
// against a far plane of ~3072, so every "sky" pixel anyone ever saw was
// just scene.background showing through, never this shader.
//
// the dome is centered on the world origin, not the camera, so it isn't
// enough to just fit the radius under FAR_CLIP — the camera itself can sit
// up to MAX_DIST away from that origin, and looking toward the far side of
// the dome from an off-center point adds that same distance again. Leaving
// out the MAX_DIST margin clips the dome exactly where a wide-shot camera
// is most likely to be looking.
const SKY_DOME_SCALE = (FAR_CLIP - MAX_DIST) * 0.9;
set_sky_radius(SKY_DOME_SCALE);

// GodRaysEffect (scene.tsx's EffectComposer) needs a real, occludable scene
// Mesh to sample as its light source — get_sun_position's own real position
// (celestial_position.ts's SUN_DISTANCE=100) sits far too close, well inside
// real terrain's own visible range (render_cfg's FOG_FAR≈900, and the camera
// itself can orbit up to MAX_DIST=512 further out), so a sun proxy placed
// there would constantly clip into/through nearby chunks instead of reading
// as a distant light source. FRUSTUM-safe middle ground: past everywhere
// real terrain/objects actually render, short of SKY_DOME_SCALE so the dome
// doesn't clip it.
const SUN_PROXY_DISTANCE = FOG_FAR + MAX_DIST + 400;
const SUN_PROXY_RADIUS = SUN_PROXY_DISTANCE * 0.02;

const sky_vert = /* glsl */ `
varying vec3 v_world_pos;

void main() {
  vec4 world_pos = modelMatrix * vec4(position, 1.0);
  v_world_pos = world_pos.xyz;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const sky_frag = /* glsl */ `
precision highp float;

varying vec3 v_world_pos;

uniform vec3 u_sky_top;
uniform vec3 u_sky_middle;
uniform vec3 u_sky_horizon;
uniform vec3 u_sun_dir;
uniform vec3 u_sun_glow;
uniform vec3 u_moon_dir;
uniform float u_time;
uniform float u_star_intensity;
// damped azimuth (world_frame_ref's own rotation.z) — see this uniform's
// assignment in MapEnvironment's useFrame for why the dome mesh's own
// (no-op) rotation can't carry this instead.
uniform float u_sky_az;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float value_noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

float fbm(vec2 p) {
  float sum = 0.0;
  float amp = 0.5;
  for (int i = 0; i < 4; i++) {
    sum += amp * value_noise(p);
    p *= 2.02;
    amp *= 0.5;
  }
  return sum;
}

void main() {
  vec3 dir = normalize(v_world_pos);
  float h = clamp(dir.z * 0.5 + 0.5, 0.0, 1.0);

  // angular (azimuth, elevation) coordinates for the procedural layers below —
  // *not* a tangent-plane projection (dir.xy / dir.z): that style blows up
  // and aliases into nothing exactly near the horizon, which is where this
  // camera spends most of its time (a birds-eye map view tilts close to
  // horizontal far more often than it looks straight up). Plain angles have
  // no division and stay well-behaved across the whole visible band; the
  // only cost is some pole-distortion right at the zenith, which this camera
  // rarely if ever frames. Minus u_sky_az: dir is a raw screen-ray direction,
  // invariant to the map's azimuth (the dome mesh's own rotation is a no-op
  // — see u_sky_az's declaration above) — without this, the star/cloud
  // pattern below reads this angle straight off the screen and stays
  // completely fixed on screen as the map turns, while the ground (and the
  // sun/moon discs, separately rotated via u_sun_dir/u_moon_dir) visibly
  // spin underneath it. Subtracting the same azimuth here instead makes az
  // track a stable *world* angle, so the pattern turns together with
  // everything else.
  float az = atan(dir.y, dir.x) - u_sky_az;

  // ---- base gradient (extra overlapping stop softens the two-band seam) ----
  vec3 color = mix(u_sky_horizon, u_sky_middle, smoothstep(0.0, 0.4, h));
  color = mix(color, u_sky_top, smoothstep(0.3, 1.0, h));

  // soft horizon lift
  float horizon_glow = 1.0 - smoothstep(0.0, 0.22, abs(h - 0.08));
  color += u_sky_horizon * horizon_glow * 0.06;

  // ---- stars (fade in at night only; twinkle via a per-star phase) ----
  if (u_star_intensity > 0.001) {
    vec2 star_uv = vec2(az * 26.0, h * 90.0);
    vec2 cell = floor(star_uv);
    float star_seed = hash(cell);
    float twinkle = 0.55 + 0.45 * sin(u_time * 2.2 + star_seed * 62.0);
    float star = smoothstep(0.975, 0.995, star_seed) * twinkle;
    float star_visibility = smoothstep(0.04, 0.3, h) * u_star_intensity;
    color += vec3(star) * star_visibility;
  }

  // ---- clouds: soft painterly puffs drifting slowly across the upper sky ----
  vec2 cloud_uv = vec2(az * 2.4, h * 5.0) + vec2(u_time * 0.02, u_time * 0.004);
  float cloud_density = fbm(cloud_uv * 1.6);
  cloud_density = smoothstep(0.52, 0.82, cloud_density);
  float cloud_mask = smoothstep(0.04, 0.28, h) * (1.0 - smoothstep(0.72, 1.0, h));
  vec3 cloud_color = mix(u_sky_horizon, vec3(1.0), 0.55);
  color = mix(color, cloud_color, cloud_density * cloud_mask * 0.5);

  // ---- sun: soft glow halo + a clearly visible disc ----
  //
  // This is a stylized miniature-diorama sky, not a physically accurate one:
  // the real sun's angular radius (~0.5°) reads as a barely-there pinprick
  // against a sky this size, easy to lose entirely in bloom, so both the
  // glow falloff and the disc itself are widened well past real proportions
  // so it actually reads as an object sitting in the sky.
  float sun_dot = max(dot(dir, normalize(u_sun_dir)), 0.0);
  float sun_glow = pow(sun_dot, 6.0);
  color = mix(color, u_sun_glow, sun_glow * 0.35);
  float sun_disc = smoothstep(0.997, 0.9992, sun_dot);
  color = mix(color, vec3(1.0, 0.98, 0.9), sun_disc);
  // thin bright rim right at the disc's edge — sells it as a solid object
  // rather than just the glow's brightest point
  float sun_rim = smoothstep(0.997, 0.9985, sun_dot) * (1.0 - smoothstep(0.9985, 0.9992, sun_dot));
  color = mix(color, vec3(1.0, 0.95, 0.85), sun_rim * 0.6);

  // ---- moon: dimmer disc + halo, only meaningfully visible once stars fade in ----
  vec3 moon_axis = normalize(u_moon_dir);
  float moon_dot = max(dot(dir, moon_axis), 0.0);
  float moon_glow = pow(moon_dot, 20.0) * u_star_intensity;
  color = mix(color, vec3(0.85, 0.9, 1.0), moon_glow * 0.4);

  float moon_disc_mask = smoothstep(0.998, 0.9993, moon_dot) * u_star_intensity;
  if (moon_disc_mask > 0.001) {
    // Real crescent/gibbous shape, not a flat circle — derived from actual
    // sphere-lit-from-one-side geometry rather than a canned phase texture,
    // so it stays consistent with whatever u_sun_dir/u_moon_dir (both real
    // ephemeris directions — see celestial_position.ts) happen to be at any
    // moment; the moon's real phase falls straight out of the angle between
    // them, no separate "phase" value needed.
    //
    // Build a local (right, up) basis perpendicular to moon_axis — world Z
    // as the "up" hint (this scene is Z-up), swapped to X on the rare frame
    // moon_axis points almost exactly along Z, so cross() doesn't degenerate.
    vec3 ref_up = abs(moon_axis.z) < 0.99 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
    vec3 m_right = normalize(cross(ref_up, moon_axis));
    vec3 m_up = cross(moon_axis, m_right);

    // this fragment's offset from the disc center, in the same tangent
    // plane, normalized so the disc's own edge (moon_dot's threshold above)
    // lands at radius ~1 — the disc's whole angular radius is a couple
    // degrees, small enough that this flat-plane approximation of the
    // sphere's surface is effectively exact.
    const float MOON_ANGULAR_RADIUS = 0.0374;
    vec3 local = dir - moon_dot * moon_axis;
    float p = dot(local, m_right) / MOON_ANGULAR_RADIUS;
    float q = dot(local, m_up) / MOON_ANGULAR_RADIUS;
    float nz = sqrt(max(0.0, 1.0 - p * p - q * q));

    // a point on the visible hemisphere has local-space normal (p, q, nz)
    // (nz being the component pointing back toward the viewer) — it's lit
    // exactly when that normal faces the real sun direction, expressed in
    // the same local basis.
    vec3 sun_n = normalize(u_sun_dir);
    float lit = p * dot(sun_n, m_right) + q * dot(sun_n, m_up) + nz * (-dot(sun_n, moon_axis));
    float lit_mask = smoothstep(-0.05, 0.05, lit);

    vec3 moon_dark = vec3(0.20, 0.21, 0.26);
    vec3 moon_lit = vec3(0.92, 0.94, 0.98);

    // moon_disc_mask alone is just "inside the disc's silhouette" — blending
    // the whole disc at that same strength regardless of lit_mask painted
    // the *shadowed* hemisphere as an equally solid, sharply-bounded circle
    // too, so a crescent/gibbous still showed its full circular outline
    // instead of the dark side blending into the night sky the way a real
    // phase does. Scaling the blend strength down toward dark_visibility as
    // lit_mask drops fixes that — the lit sliver stays fully opaque, the
    // dark side mostly gives way to whatever's behind it (sky/stars), with
    // just a faint earthshine-like hint rather than fully invisible.
    float dark_visibility = 0.06;
    float moon_visible_strength = mix(dark_visibility, 1.0, lit_mask);
    color = mix(color, mix(moon_dark, moon_lit, lit_mask), moon_disc_mask * moon_visible_strength);
  }

  // ---- dither: breaks up 8-bit banding across the smooth gradient ----
  color += (hash(gl_FragCoord.xy) - 0.5) * (1.0 / 255.0);

  gl_FragColor = vec4(color, 1.0);
}
`;

// Real-time sun/moon position and the fog/sky/light color lerps below all
// change a little every frame purely from the wall clock ticking, not from
// any prop/store update react could hang an effect off of — under
// frameloop="demand" (scene.tsx) nothing else would ever ask for another
// frame on their behalf, so they'd otherwise freeze the instant the canvas
// goes idle. A self-rescheduling setTimeout chain (below, near
// star_intensity) keeps them alive without it, at one of two rates instead
// of a single flat one: fast while a day/night theme transition is actually
// in flight (lerp_factor decays over roughly a second, and this cadence
// needs to stay well under that to still read as a smooth fade rather than
// a visible step), slow the rest of the time — which, given the real
// sun/moon sweep is a full lap every 24 real hours, is almost always. This
// adaptive scheme gets the same transition smoothness while the idle case
// (the much more common one) drops further
// still, the same "give each element its own real update need instead of
// one shared rate" reasoning the location_puck beacon (always, no gap) and
// the camera/frame damping hooks (event-driven, no fixed cadence at all)
// already apply in their own ways.
const ENV_INVALIDATE_FAST_MS = 80;
const ENV_INVALIDATE_SLOW_MS = 2000;
// how close star_intensity needs to be to its target before a transition
// counts as "done" and this can drop back to the slow rate.
const ENV_TRANSITION_SETTLE_EPS = 0.005;

type MapEnvironmentProps = {
  // Owned by MapScene (scene.tsx), not here — scene.tsx's own <GodRays sun={...}>
  // needs this same Mesh instance to read, so the ref has to be created
  // above both consumers rather than a new one made and kept local here.
  sun_proxy_ref: RefObject<THREE.Mesh | null>;
};

export function MapEnvironment(props: MapEnvironmentProps) {
  const { sun_proxy_ref } = props;
  const { scene } = useThree();

  const show_god_rays = useSettingsStore((state) => state.show_god_rays);

  // Real sun/moon position needs the map's actual latitude/longitude — this
  // read doesn't gate anything, just supplies a location for celestial_position.ts.
  const map_center = useMapStore((state) => state.map_center);
  const latlon_scratch = useRef<G4326>([0, 0, 0]);
  const get_observer_lat_lon = (): { lat: f64; lon: f64 } => {
    if (map_center) {
      const [lon, lat] = epsg4326.unproject(latlon_scratch.current, map_center.i, map_center.f);
      return { lat, lon };
    }
    return { lat: FALLBACK_LAT, lon: FALLBACK_LON };
  };

  const show_shadow_map = useSettingsStore((state) => state.show_shadow_map);

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];
  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;
  // kept in sync every render so the setTimeout chain below (a stable
  // closure set up once on mount) always reads the latest value instead of
  // whatever is_night happened to be the moment that closure was created.
  const is_night_ref = useRef(is_night);
  is_night_ref.current = is_night;

  const ambient_ref = useRef<THREE.AmbientLight>(null);
  const sun_ref = useRef<THREE.DirectionalLight>(null);
  const moon_ref = useRef<THREE.DirectionalLight>(null);
  const hemi_ref = useRef<THREE.HemisphereLight>(null);

  // read-only: MapEnvironment is a FrameProvider sibling of World (scene.tsx),
  // not nested under it. Needed below to compose map_center's true world
  // position via local_world_frame_ref.localToWorld() — see the shadow
  // anchor block's own comment for why. Never write to this ref (owned by
  // World's own useFrameTranslation call).
  const { local_world_frame_ref } = useFrameRefs();

  /**
   * Rotating the dome mesh itself is a no-op: the sky shader colors purely
   * from the resulting world-space ray direction, and a sphere's surface
   * covers every world direction exactly once regardless of its own
   * rotation — this group's rotation is kept only as a convenient,
   * already-damped azimuth value, not because spinning the mesh does
   * anything visible.
   *
   * The camera never rotates by azimuth (only dist/polar do — see orbit.ts);
   * azimuth instead spins world_frame_ref under a fixed camera, which is
   * what sweeps a wall's lit side across the screen as the map turns. For
   * the sky to track that, u_sun_dir/u_moon_dir get rotated by that same
   * azimuth before reaching the sky material each frame (below) — equivalent
   * to rotating the whole sky with the ground. SUN_DIR (mat.ts's
   * u_light_dir, real terrain lighting) deliberately stays unrotated: the
   * terrain geometry already rotates with world_frame under a fixed light,
   * which is what tracks a wall's lit side correctly — rotating the light
   * too would cancel that out. Same reasoning keeps sun_ref/moon_ref (the
   * real DirectionalLights) untouched.
   */
  const sky_frame_ref = useRef<THREE.Group>(null);
  useFrameRotation({ frame_ref: sky_frame_ref });
  const sky_sun_dir_scratch = useRef(new THREE.Vector3());
  const sky_moon_dir_scratch = useRef(new THREE.Vector3());

  const fog_color = useMemo(() => new THREE.Color(theme.fog), [theme.fog]);
  const sky_top_color = useMemo(() => new THREE.Color(theme.sky.colors.top), [theme.sky.colors.top]);
  const sky_middle_color = useMemo(() => new THREE.Color(theme.sky.colors.middle), [theme.sky.colors.middle]);
  const sky_horizon_color = useMemo(() => new THREE.Color(theme.sky.colors.horizon), [theme.sky.colors.horizon]);
  const bg_color = useMemo(() => new THREE.Color(theme.background), [theme.background]);
  const ambient_color = useMemo(() => new THREE.Color(theme.lighting.ambient.color), [theme.lighting.ambient.color]);
  const sun_color = useMemo(() => new THREE.Color(theme.lighting.directional.color), [theme.lighting.directional.color]);
  const hemi_sky_color = useMemo(() => new THREE.Color(theme.lighting.hemisphere.sky), [theme.lighting.hemisphere.sky]);
  const hemi_ground_color = useMemo(() => new THREE.Color(theme.lighting.hemisphere.ground), [theme.lighting.hemisphere.ground]);
  const sun_glow_color = useMemo(() => new THREE.Color(theme.lighting.directional.color), [theme.lighting.directional.color]);
  const moon_color = useMemo(() => new THREE.Color("#9fb4d6"), []);

  // Continuous, real-time sun/moon positions (celestial_position.ts) — real
  // solar/lunar ephemeris now, not a stylized sweep. Recomputed fresh every
  // frame below (cheap trig, and everything else here already does per-frame
  // trig/lerp); current_*_position still lerps toward that fresh target
  // each frame so the motion stays smooth rather than snapping every tick.
  //
  // The seed below only matters for the very first frame (useFrame overwrites
  // it immediately after) — useMemo(..., []) instead of a bare useRef(expr)
  // so get_observer_lat_lon()/get_sun_position()/get_moon_position() (real
  // trig, plus an inverse-Mercator unproject) actually run once at mount, not
  // on every re-render this component gets from map_center changing (a plain
  // useRef(expr) still evaluates its argument every render — React just
  // discards it after the first — so this was silently redone, and thrown
  // away, on every pan/drag step).
  const initial_position = useMemo(() => {
    const { lat, lon } = get_observer_lat_lon();
    return {
      sun: new THREE.Vector3(...get_sun_position(lat, lon)),
      moon: new THREE.Vector3(...get_moon_position(lat, lon)),
    };
  }, []);
  const current_sun_position = useRef(initial_position.sun);
  const current_moon_position = useRef(initial_position.moon);
  const sun_position_scratch = useRef(new THREE.Vector3());
  const moon_position_scratch = useRef(new THREE.Vector3());
  const moon_dir_scratch = useRef(new THREE.Vector3());
  const sky_material_ref = useRef<THREE.ShaderMaterial>(null);
  // Created exactly once (empty deps) and never again — this is the actual
  // fix for a real, long-standing bug: R3F's shaderMaterial `uniforms` prop
  // has no special-case handling (see @react-three/fiber's applyProps —
  // Color/Vector-like targets get `.copy()`'d, but a plain uniforms object
  // just gets `root.uniforms = value`, a full replacement). Passing a fresh
  // object literal here on every render meant every MapEnvironment re-render
  // silently reset every uniform below back to this literal's values —
  // wiping out whatever the useFrame loop had been imperatively writing via
  // `.value = ...` all along, including u_sun_dir's azimuth rotation and
  // u_time (hence stationary clouds/stars/sun regardless of real rotation —
  // confirmed live: sky_frame_ref's own rotation.z tracked azimuth
  // correctly the whole time, only the uniforms feeding off it never held
  // still long enough to matter). The values below are only ever the
  // *starting* point; useFrame owns them from the first frame on.
  const sky_uniforms = useMemo(
    () => ({
      u_sky_top: { value: sky_top_color.clone() },
      u_sky_middle: { value: sky_middle_color.clone() },
      u_sky_horizon: { value: sky_horizon_color.clone() },
      u_sun_dir: { value: current_sun_position.current.clone().normalize() },
      u_sun_glow: { value: sun_glow_color.clone() },
      u_moon_dir: { value: current_moon_position.current.clone().normalize() },
      u_time: { value: 0 },
      u_star_intensity: { value: 0 },
      u_sky_az: { value: 0 },
    }),
    [],
  );

  // last light direction that actually triggered a shadow-map render (see
  // SHADOW_UPDATE_DOT_THRESHOLD above) — zero-vector start is deliberate,
  // not a magic sentinel: no real (normalized) light direction can dot to
  // >= the threshold against a zero vector, so the very first frame always
  // triggers the initial shadow render for free, without a separate
  // "already initialized" flag.
  const last_sun_shadow_dir = useRef(new THREE.Vector3());
  const last_moon_shadow_dir = useRef(new THREE.Vector3());
  const prev_is_night = useRef(is_night);
  const prev_show_shadow_map = useRef(show_shadow_map);

  // map_center's true world position (composing local-world-frame's
  // translation AND world-frame's rotation via localToWorld — see the big
  // comment where this is computed, below in "Light interpolation", for why
  // this and not local-world-frame's own raw position or camera.position)
  // — the point both shadow frustums recenter on every frame the setting is
  // on.
  const shadow_anchor_scratch = useRef(new THREE.Vector3());
  const last_shadow_anchor = useRef(new THREE.Vector3());

  // stars/moon-disc visibility — lerped the same way as the light intensities
  // below (theme.lighting.hemisphere.intensity etc.), so night doesn't just
  // cut in but fades in alongside the rest of the theme transition. Also
  // doubles as this component's own "is a transition actually in flight"
  // signal for the adaptive invalidate loop below — it lerps on the same
  // lerp_factor as everything else here, so how close it is to its target
  // is a fair proxy for the fog/sky/light lerps without needing to check
  // each of those separately.
  const star_intensity = useRef<f64>(0);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      invalidate();
      const target = is_night_ref.current ? 1 : 0;
      const transitioning = Math.abs(star_intensity.current - target) > ENV_TRANSITION_SETTLE_EPS;
      timer = setTimeout(tick, transitioning ? ENV_INVALIDATE_FAST_MS : ENV_INVALIDATE_SLOW_MS);
    };
    timer = setTimeout(tick, ENV_INVALIDATE_FAST_MS);
    return () => clearTimeout(timer);
  }, []);

  useTick("cosmetic", ({ state, delta }) => {
    if (!scene) return;

    // clamped: delta can spike well past a normal frame time (first frame
    // after a shader compile stall, tab backgrounding, GC pause, ...), and
    // an un-clamped factor overshoots every lerp/MathUtils.lerp call below
    // past its target instead of just reaching it faster.
    const lerp_factor: f64 = clamp(delta * 1.5, 0, 1);
    star_intensity.current = THREE.MathUtils.lerp(star_intensity.current, is_night ? 1 : 0, lerp_factor);

    // ---- 1. Fog interpolation ----
    // Where the WebGPU renderer draws the sky, three clears to nothing and
    // draws no dome; the uniforms below still move, and it reads them.
    const gpu_sky = gpu_draws_chunks();
    const dome = sky_material_ref.current ? scene.getObjectByName("sky") : undefined;
    if (dome) dome.visible = !gpu_sky;
    if (gpu_sky) {
      scene.background = null;
    } else if (scene.background instanceof THREE.Color) {
      scene.background.lerp(bg_color, lerp_factor);
    } else {
      scene.background = bg_color.clone();
    }

    if (scene.fog && "isFog" in scene.fog) {
      const fog = scene.fog as THREE.Fog;
      fog.color.lerp(fog_color, lerp_factor);
    }

    // ---- 2. Sky interpolation ----
    // Fresh target every frame (not memoized on theme) — this is what
    // actually makes the sun/moon track the real ephemeris continuously.
    // lerp still smooths frame-to-frame motion (real position changes in
    // small enough steps frame-to-frame that this is purely cosmetic
    // smoothing, not chasing a target that jumps).
    const { lat: observer_lat, lon: observer_lon } = get_observer_lat_lon();
    sun_position_scratch.current.set(...get_sun_position(observer_lat, observer_lon));
    current_sun_position.current.lerp(sun_position_scratch.current, lerp_factor);
    moon_position_scratch.current.set(...get_moon_position(observer_lat, observer_lon));
    current_moon_position.current.lerp(moon_position_scratch.current, lerp_factor);

    // Hoisted out of the sky block below (it's also read there) because the
    // shadow-casting sun_ref/moon_ref direction needs it too — see the
    // shadow-position block's own comment for why: a shadow's projected
    // shape is only rotation-rigid (an L staying an L, not shearing into an
    // I) if BOTH the geometry AND the light direction rotate together by
    // the same azimuth. world_frame_ref's rotation is a pure Z-axis spin
    // simulating "the camera turned" by turning the world under a fixed
    // camera instead — so anything meant to represent a real,
    // camera-independent physical direction (the actual sun) has to rotate
    // WITH that spin to stay attached to the (rotating) world it's lighting,
    // the same way SUN_DIR already does for the sky dome below.
    const az = sky_frame_ref.current?.rotation.z ?? 0;

    if (sky_material_ref.current) {
      const { uniforms } = sky_material_ref.current;
      uniforms.u_sky_top.value.lerp(sky_top_color, lerp_factor);
      uniforms.u_sky_middle.value.lerp(sky_middle_color, lerp_factor);
      uniforms.u_sky_horizon.value.lerp(sky_horizon_color, lerp_factor);

      // true (unrotated) sun direction — shared with terrain lighting via
      // SUN_DIR, which must stay real (see sky_frame_ref's doc comment
      // above for why).
      SUN_DIR.copy(current_sun_position.current).normalize();

      // rotate the *shader's own copies* of the sun/moon directions by the
      // same azimuth world_frame_ref rotates by (az, hoisted above), so the
      // visible disc sweeps across the sky as the map turns instead of
      // sitting fixed on screen. sky_frame_ref is read purely as an
      // already-damped azimuth value (see its doc comment above) — this
      // hook call registers its useFrame before this one runs (called
      // earlier in this component body), so rotation.z is current as of
      // this same frame, not one frame stale.
      // stars/clouds (sky_frag) key off raw screen-ray direction, which
      // rotating the dome mesh itself can't fix (a sphere occupies the same
      // world-space point set regardless of its own spin, so v_world_pos for
      // a fixed screen ray is unchanged by it — see the dome group's own
      // comment). Passing az through lets the shader subtract it from the
      // angle it derives from that direction instead, the same fix shape as
      // rotating u_sun_dir/u_moon_dir just below, applied where a whole
      // vector can't be pre-rotated on the CPU side (every screen pixel has
      // its own direction).
      uniforms.u_sky_az.value = az;
      const rotated_sun = rotate2(SUN_DIR.x, SUN_DIR.y, az);
      sky_sun_dir_scratch.current.set(rotated_sun.x, rotated_sun.y, SUN_DIR.z);
      uniforms.u_sun_dir.value.copy(sky_sun_dir_scratch.current);

      moon_dir_scratch.current.copy(current_moon_position.current).normalize();
      const rotated_moon = rotate2(moon_dir_scratch.current.x, moon_dir_scratch.current.y, az);
      sky_moon_dir_scratch.current.set(rotated_moon.x, rotated_moon.y, moon_dir_scratch.current.z);
      uniforms.u_moon_dir.value.copy(sky_moon_dir_scratch.current);

      uniforms.u_sun_glow.value.lerp(sun_glow_color, lerp_factor);
      uniforms.u_time.value = state.clock.getElapsedTime();
      uniforms.u_star_intensity.value = star_intensity.current;
    }

    // ---- 3. Light interpolation ----

    // day/night flips which of sun/moon actually casts (castShadow={!is_night}
    // / castShadow={is_night} below) — the newly-active one may not have
    // rendered a shadow map in a while (its own angular-delta check below
    // still ran while it was inactive, so last_*_shadow_dir was already
    // "caught up" and wouldn't otherwise re-trigger right on this transition
    // frame), which would leave it showing a stale shadow angle from
    // whenever it last actually cast one until the next natural angular
    // update. Force both fresh exactly on the transition instead.
    if (show_shadow_map && is_night !== prev_is_night.current) {
      prev_is_night.current = is_night;
      if (sun_ref.current) sun_ref.current.shadow.needsUpdate = true;
      if (moon_ref.current) moon_ref.current.shadow.needsUpdate = true;
    }

    // Toggling show_shadow_map back on can't rely on the angular-delta
    // checks below alone: those only fire once the light has drifted past
    // SHADOW_UPDATE_DOT_THRESHOLD since last_*_shadow_dir was last touched,
    // but last_*_shadow_dir is frozen while the setting is off — so a quick
    // off/on cycle with the sun barely moving would otherwise skip a refresh
    // even though chunk streaming (independent of the toggle) may have
    // mounted/unmounted geometry the last-rendered shadow map never saw.
    if (show_shadow_map !== prev_show_shadow_map.current) {
      prev_show_shadow_map.current = show_shadow_map;
      if (show_shadow_map) {
        if (sun_ref.current) sun_ref.current.shadow.needsUpdate = true;
        if (moon_ref.current) moon_ref.current.shadow.needsUpdate = true;
      }
    }

    // sun_ref/moon_ref (below) live outside World's floating-origin frame
    // chain (view-frame > world-frame [rotates by azimuth] > local-world-frame
    // [translates] > per-L4-anchor group > per-page group > per-chunk group —
    // world.tsx/registry.ts/anchor/node.tsx), so their shadow-camera frustum
    // never inherits either the rotation or the translation that keeps
    // actually-rendered chunk/object geometry positioned correctly as you pan
    // and rotate. camera.position (use_camera.ts) is NOT a substitute for
    // this: it's a fixed local orbit offset, always calling
    // camera.lookAt(0, 0, 0), completely uninvolved in panning, so it
    // silently breaks the instant azimuth is non-zero.
    //
    // The only correct anchor is "map_center's own true world position",
    // computed the same way AnchorNode positions any of its children
    // (anchor/node.tsx): map_center.i's offset from its owning L4 anchor's
    // base, converted to real units, taken as a LOCAL point within
    // local-world-frame and pushed through localToWorld() — which composes
    // local-world-frame's translation AND world-frame's rotation correctly
    // via its matrixWorld. A raw vector add is not equivalent:
    // local-world-frame's own position already has the rotation baked in
    // for ITS OWN origin, but a residual local offset on top of that still
    // needs its own rotation applied, or it points the same raw direction
    // regardless of current azimuth.
    if (show_shadow_map && local_world_frame_ref.current && map_center) {
      const anchor_base = L4_BASE(map_center.i);
      const local: PVF3 = { i: ivec.sub(map_center.i, anchor_base), f: map_center.f };
      shadow_anchor_scratch.current.set(
        pvf3.to_real(pvf3.X(local)),
        pvf3.to_real(pvf3.Y(local)),
        pvf3.to_real(pvf3.Z(local)),
      );
      local_world_frame_ref.current.localToWorld(shadow_anchor_scratch.current);

      if (!shadow_anchor_scratch.current.equals(last_shadow_anchor.current)) {
        last_shadow_anchor.current.copy(shadow_anchor_scratch.current);
        if (sun_ref.current) sun_ref.current.shadow.needsUpdate = true;
        if (moon_ref.current) moon_ref.current.shadow.needsUpdate = true;
      }
    }

    if (ambient_ref.current) {
      ambient_ref.current.color.lerp(ambient_color, lerp_factor);
      ambient_ref.current.intensity = THREE.MathUtils.lerp(ambient_ref.current.intensity, theme.lighting.ambient.intensity, lerp_factor);
    }

    if (sun_ref.current) {
      // Rotated by az (unlike SUN_DIR/terrain's own uniform above) — sun_ref
      // is a real DirectionalLight, illuminating real MeshStandardMaterial
      // surfaces (placed objects, chunk_stage.tsx) via ordinary dot-product
      // lighting AND driving the shadow camera below, and for a shadow's
      // projected shape to stay rigid under rotation (an L staying an L,
      // not shearing into an I as azimuth changes — this is what an
      // un-rotated light direction against rotating geometry does: the
      // light-to-geometry angle keeps changing even though nothing "really"
      // moved) the light has to rotate WITH the geometry it's lighting, the
      // same way the sky dome's own u_sun_dir already does above. This
      // does NOT touch SUN_DIR (terrain's own shader uniform stays as
      // before) — a real, pre-existing inconsistency this surfaces but
      // doesn't fix, since unifying that is a bigger change than tonight's
      // shadow-rotation fix.
      const rotated_sun_dir = rotate2(current_sun_position.current.x, current_sun_position.current.y, az);
      sun_ref.current.position.set(rotated_sun_dir.x, rotated_sun_dir.y, current_sun_position.current.z);
      sun_ref.current.color.lerp(sun_color, lerp_factor);
      sun_ref.current.intensity = THREE.MathUtils.lerp(sun_ref.current.intensity, theme.lighting.directional.intensity, lerp_factor);

      // GodRaysEffect's own light-source Mesh (see SUN_PROXY_DISTANCE's doc
      // comment) — same rotated direction as sun_ref itself, just normalized
      // and scaled out to a distance real terrain/objects can't clip into.
      // Not floating-frame-anchored, same reasoning as sun_ref/the sky dome
      // (MapEnvironment's own doc comment above): this whole component
      // renders outside World's floating-origin chain, so raw world
      // coordinates already track the (fixed, lookAt(0,0,0)) camera
      // correctly without an anchor offset. Kept mounted and updated
      // unconditionally (show_god_rays or not) rather than conditionally
      // mounted — scene.tsx's own <GodRays sun={...}> needs a real,
      // already-attached Mesh ref the first render the setting flips on;
      // conditionally mounting this element would leave sun_proxy_ref.current
      // null for that render (refs attach during commit, one render after
      // the JSX first appears), so GodRaysEffect would either skip a frame
      // or need extra state just to know when the ref becomes available. A
      // permanently-mounted 16-segment sphere is cheap enough that avoiding
      // that complexity is worth it outright.
      if (sun_proxy_ref.current) {
        sun_proxy_ref.current.position
          .set(rotated_sun_dir.x, rotated_sun_dir.y, current_sun_position.current.z)
          .normalize()
          .multiplyScalar(SUN_PROXY_DISTANCE);
      }

      // shadow.autoUpdate=false (JSX below) means this is the only thing
      // that ever asks for a new shadow render — see
      // SHADOW_UPDATE_DOT_THRESHOLD's own comment.
      if (show_shadow_map) {
        // sun_position_scratch is free to reuse here: its own job (this
        // frame's fresh get_sun_position() target) is already done by the
        // lerp above, nothing later in this frame reads it again. Must use
        // the ROTATED direction (rotated_sun_dir), not current_sun_position
        // directly — otherwise a pure azimuth change (sun angle itself
        // unchanged) would never trip SHADOW_UPDATE_DOT_THRESHOLD below,
        // leaving the shadow map stale after rotating even though the
        // light's actual direction (post-rotation) demonstrably moved.
        sun_ref.current.target.position.copy(shadow_anchor_scratch.current);
        sun_ref.current.target.updateMatrixWorld();
        sun_ref.current.position.add(shadow_anchor_scratch.current);

        const sun_dir_now = sun_position_scratch.current.set(rotated_sun_dir.x, rotated_sun_dir.y, current_sun_position.current.z).normalize();
        if (sun_dir_now.dot(last_sun_shadow_dir.current) < SHADOW_UPDATE_DOT_THRESHOLD) {
          sun_ref.current.shadow.needsUpdate = true;
          last_sun_shadow_dir.current.copy(sun_dir_now);
        }
      }
    }

    if (moon_ref.current) {
      // same rotation reasoning as the sun block above.
      const rotated_moon_dir = rotate2(current_moon_position.current.x, current_moon_position.current.y, az);
      moon_ref.current.position.set(rotated_moon_dir.x, rotated_moon_dir.y, current_moon_position.current.z);
      moon_ref.current.color.lerp(moon_color, lerp_factor);

      // same reasoning as the sun block above, reusing moon_position_scratch
      // the same way.
      if (show_shadow_map) {
        moon_ref.current.target.position.copy(shadow_anchor_scratch.current);
        moon_ref.current.target.updateMatrixWorld();
        moon_ref.current.position.add(shadow_anchor_scratch.current);

        const moon_shadow_dir_now = moon_position_scratch.current.set(rotated_moon_dir.x, rotated_moon_dir.y, current_moon_position.current.z).normalize();
        if (moon_shadow_dir_now.dot(last_moon_shadow_dir.current) < SHADOW_UPDATE_DOT_THRESHOLD) {
          moon_ref.current.shadow.needsUpdate = true;
          last_moon_shadow_dir.current.copy(moon_shadow_dir_now);
        }
      }

      moon_ref.current.intensity = THREE.MathUtils.lerp(
        moon_ref.current.intensity,
        is_night ? 0.18 : 0.0,
        lerp_factor,
      );
    }

    if (hemi_ref.current) {
      hemi_ref.current.color.lerp(hemi_sky_color, lerp_factor);
      hemi_ref.current.groundColor.lerp(hemi_ground_color, lerp_factor);
      hemi_ref.current.intensity = THREE.MathUtils.lerp(hemi_ref.current.intensity, theme.lighting.hemisphere.intensity, lerp_factor);
    }
  }, "environment");

  return (
    <>
      <group ref={sky_frame_ref}>
        {
          /*
          renderOrder is what makes this the background rather than a
          foreground object that happens to be far away.

          It writes colour with depthTest off and depthWrite off, so it paints
          over whatever has already been drawn and leaves the depth buffer
          alone. Without a renderOrder it takes its place in three's opaque
          list by `material.id` (WebGLRenderLists' painterSortStable, which
          compares groupOrder, then renderOrder, then material.id, and only
          then distance) — a global counter incremented at material
          construction. Every material built before this one therefore draws
          before the sky and is erased by it, while its depth survives, so
          whatever stands behind it is then depth-rejected. The result is that
          object's exact silhouette filled with sky.

          Which materials lose is a race, because construction order is not
          render order: decor's materials are built when their skin PNGs
          resolve (decor/asset_store.ts), which with a warm browser cache can
          land before this component has mounted at all. That is the whole of
          the "decor draws as a hole through to the sky" bug — clouds and all,
          night sky at night — and of the same thing happening to the location
          puck's beacon perhaps once in a couple of hundred loads, while the
          zone box beside it, which carries renderOrder 5, never did.
        */
        }
        <mesh name="sky" scale={SKY_DOME_SCALE} frustumCulled={false} renderOrder={-1}>
          <sphereGeometry args={[1, 32, 32]} />
          <shaderMaterial
            ref={sky_material_ref}
            vertexShader={sky_vert}
            fragmentShader={sky_frag}
            side={THREE.BackSide}
            depthWrite={false}
            depthTest={false}
            fog={false}
            uniforms={sky_uniforms}
          />
        </mesh>
      </group>

      {/* Linear scene fog. */}
      <fog attach="fog" args={[theme.fog, FOG_NEAR, FOG_FAR]} />

      <ambientLight
        ref={ambient_ref}
        color={theme.lighting.ambient.color}
        intensity={theme.lighting.ambient.intensity}
      />

      <directionalLight
        ref={sun_ref}
        color={theme.lighting.directional.color}
        intensity={theme.lighting.directional.intensity}
        position={current_sun_position.current}
        castShadow={show_shadow_map && !is_night}
        // manual update instead — see SHADOW_UPDATE_DOT_THRESHOLD's comment
        // and the sun_ref block in the useFrame above.
        shadow-autoUpdate={false}
        shadow-mapSize={[shadow_cfg.SUN_MAP_SIZE, shadow_cfg.SUN_MAP_SIZE]}
        // see shadow_cfg's own doc comment (environment/config.ts) for why
        // this extent reuses its original tuned value unchanged.
        shadow-camera-left={-shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-right={shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-top={shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-bottom={-shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-far={shadow_cfg.SUN_CAMERA_FAR}
        shadow-bias={shadow_cfg.SUN_BIAS}
        shadow-normalBias={shadow_cfg.SUN_NORMAL_BIAS}
      />

      {
        // GodRaysEffect's light-source Mesh (see SUN_PROXY_DISTANCE's doc
        // comment above and scene.tsx's own <GodRays sun={sun_proxy_ref}>) —
        // permanently mounted (see the useFrame block's own comment on why),
        // visible only toggles whether it's ever drawn into the main color
        // pass; GodRaysEffect reads the ref directly, unaffected by this.
        // depthWrite={false} + transparent: GodRaysEffect's own constructor
        // doc comment requires both, so real terrain/objects between the
        // camera and this mesh occlude the light source for the effect's
        // own occlusion pass without this sphere ever writing into (or
        // fighting) the main depth buffer itself.
        //
        // colorWrite={false} would seem like a way to hide this sphere's own
        // visible disc, but GodRaysEffect samples this mesh's actual
        // contribution to the main color buffer for its occlusion pass — it
        // is not a separate internal render — so hiding it that way removes
        // the rays too. This means it really does show two "suns" (this
        // sphere plus the sky dome's own procedural disc) whenever god rays
        // are on; known, not fixed.
      }
      <mesh ref={sun_proxy_ref} visible={show_god_rays} position={current_sun_position.current}>
        <sphereGeometry args={[SUN_PROXY_RADIUS, 16, 16]} />
        <meshBasicMaterial color="#fff6d8" transparent depthWrite={false} toneMapped={false} />
      </mesh>

      <directionalLight
        ref={moon_ref}
        color="#9fb4d6"
        // Raised alongside light_cfg.FLOOR (see its comment). Directional,
        // unlike that floor: it brightens without flattening, because faces
        // still catch it at different angles.
        intensity={is_night ? 0.3 : 0.0}
        position={current_moon_position.current}
        castShadow={show_shadow_map && is_night}
        // manual update instead — see SHADOW_UPDATE_DOT_THRESHOLD's comment
        // and the moon_ref block in the useFrame above.
        shadow-autoUpdate={false}
        shadow-mapSize={[shadow_cfg.MOON_MAP_SIZE, shadow_cfg.MOON_MAP_SIZE]}
        // same resolution reasoning as the sun above, scaled to this
        // light's smaller map (see shadow_cfg's own doc comment).
        shadow-camera-left={-shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-right={shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-top={shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-bottom={-shadow_cfg.FRUSTUM_HALF_EXTENT}
        shadow-camera-far={shadow_cfg.MOON_CAMERA_FAR}
        shadow-bias={shadow_cfg.MOON_BIAS}
        shadow-normalBias={shadow_cfg.MOON_NORMAL_BIAS}
      />

      <hemisphereLight
        ref={hemi_ref}
        args={[
          theme.lighting.hemisphere.sky,
          theme.lighting.hemisphere.ground,
          theme.lighting.hemisphere.intensity,
        ]}
      />
    </>
  );
}
