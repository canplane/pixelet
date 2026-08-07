// @pxl/web/modules/map/environment.tsx

import { useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";

import { render_cfg, shadow_cfg } from "@/modules/world/render/config.ts";
import { themes, type WorldTheme, WorldThemeKey } from "@/modules/world/render/themes.ts";
import { useThemeKey } from "@/modules/world/render/theme_clock.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { SUN_DIR } from "@/modules/world/render/sun_dir.ts";
import { useFrameRotation } from "@/modules/world/frame/use_frame_rotation.ts";
import { rotate2 } from "@/modules/camera/orbit.ts";

const { FOG_NEAR, FOG_FAR, FAR_CLIP } = render_cfg;
const { MAX_DIST } = camera_cfg.view;
const { FRUSTUM_HALF_EXTENT: SHADOW_HALF_EXTENT } = shadow_cfg;

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
  // rarely if ever frames.
  float az = atan(dir.y, dir.x);

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
  // the disc used to be smoothstep(0.9990, 0.9997, sun_dot) — an angular
  // radius matching the *real* sun (~0.5°), which is accurate but reads as
  // a barely-there pinprick against a sky this size, easy to lose entirely
  // in bloom. This is a stylized miniature-diorama sky, not a physically
  // accurate one (see the reference stills this whole render pass has been
  // chasing, which all show a graphic, clearly-placed sun/moon) — widened
  // both the glow falloff and the disc itself well past real proportions so
  // it actually reads as an object sitting in the sky.
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
  float moon_dot = max(dot(dir, normalize(u_moon_dir)), 0.0);
  float moon_glow = pow(moon_dot, 20.0) * u_star_intensity;
  color = mix(color, vec3(0.85, 0.9, 1.0), moon_glow * 0.4);
  float moon_disc = smoothstep(0.998, 0.9993, moon_dot) * u_star_intensity;
  color = mix(color, vec3(0.92, 0.94, 0.98), moon_disc);

  // ---- dither: breaks up 8-bit banding across the smooth gradient ----
  color += (hash(gl_FragCoord.xy) - 0.5) * (1.0 / 255.0);

  gl_FragColor = vec4(color, 1.0);
}
`;

export function MapEnvironment() {
  const { scene } = useThree();

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];
  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;

  const ambient_ref = useRef<THREE.AmbientLight>(null);
  const sun_ref = useRef<THREE.DirectionalLight>(null);
  const moon_ref = useRef<THREE.DirectionalLight>(null);
  const hemi_ref = useRef<THREE.HemisphereLight>(null);

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

  const sun_position = useMemo(() => new THREE.Vector3(...theme.sky.sun_position), [theme.sky.sun_position]);
  const current_sun_position = useRef(sun_position.clone());
  const moon_position = useMemo(() => new THREE.Vector3(120, 180, 80), []);
  const moon_dir = useMemo(() => moon_position.clone().normalize(), [moon_position]);
  const sky_material_ref = useRef<THREE.ShaderMaterial>(null);

  // stars/moon-disc visibility — lerped the same way as the light intensities
  // below (theme.lighting.hemisphere.intensity etc.), so night doesn't just
  // cut in but fades in alongside the rest of the theme transition.
  const star_intensity = useRef<f64>(0);

  useFrame((state, delta) => {
    if (!scene) return;

    // clamped: delta can spike well past a normal frame time (first frame
    // after a shader compile stall, tab backgrounding, GC pause, ...), and
    // an un-clamped factor overshoots every lerp/MathUtils.lerp call below
    // past its target instead of just reaching it faster.
    const lerp_factor: f64 = clamp(delta * 1.5, 0, 1);
    star_intensity.current = THREE.MathUtils.lerp(star_intensity.current, is_night ? 1 : 0, lerp_factor);

    // ---- 1. Fog interpolation ----
    if (scene.background instanceof THREE.Color) {
      scene.background.lerp(bg_color, lerp_factor);
    } else {
      scene.background = bg_color.clone();
    }

    if (scene.fog && "isFog" in scene.fog) {
      const fog = scene.fog as THREE.Fog;
      fog.color.lerp(fog_color, lerp_factor);
    }

    // ---- 2. Sky interpolation ----
    current_sun_position.current.lerp(sun_position, lerp_factor);
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
      // same azimuth world_frame_ref rotates by, so the visible disc sweeps
      // across the sky as the map turns instead of sitting fixed on screen.
      // sky_frame_ref is read purely as an already-damped azimuth value
      // here (see its doc comment above) — this hook call registers its
      // useFrame before this one runs (called earlier in this component
      // body), so rotation.z is current as of this same frame, not one
      // frame stale.
      const az = sky_frame_ref.current?.rotation.z ?? 0;
      const rotated_sun = rotate2(SUN_DIR.x, SUN_DIR.y, az);
      sky_sun_dir_scratch.current.set(rotated_sun.x, rotated_sun.y, SUN_DIR.z);
      uniforms.u_sun_dir.value.copy(sky_sun_dir_scratch.current);

      const rotated_moon = rotate2(moon_dir.x, moon_dir.y, az);
      sky_moon_dir_scratch.current.set(rotated_moon.x, rotated_moon.y, moon_dir.z);
      uniforms.u_moon_dir.value.copy(sky_moon_dir_scratch.current);

      uniforms.u_sun_glow.value.lerp(sun_glow_color, lerp_factor);
      uniforms.u_time.value = state.clock.getElapsedTime();
      uniforms.u_star_intensity.value = star_intensity.current;
    }

    // ---- 3. Light interpolation ----
    if (ambient_ref.current) {
      ambient_ref.current.color.lerp(ambient_color, lerp_factor);
      ambient_ref.current.intensity = THREE.MathUtils.lerp(ambient_ref.current.intensity, theme.lighting.ambient.intensity, lerp_factor);
    }

    if (sun_ref.current) {
      sun_ref.current.position.copy(current_sun_position.current);
      sun_ref.current.color.lerp(sun_color, lerp_factor);
      sun_ref.current.intensity = THREE.MathUtils.lerp(sun_ref.current.intensity, theme.lighting.directional.intensity, lerp_factor);
    }

    if (moon_ref.current) {
      moon_ref.current.position.copy(moon_position);
      moon_ref.current.color.lerp(moon_color, lerp_factor);
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
  });

  return (
    <>
      <group ref={sky_frame_ref}>
        <mesh scale={SKY_DOME_SCALE} frustumCulled={false}>
          <sphereGeometry args={[1, 32, 32]} />
          <shaderMaterial
            ref={sky_material_ref}
            vertexShader={sky_vert}
            fragmentShader={sky_frag}
            side={THREE.BackSide}
            depthWrite={false}
            depthTest={false}
            fog={false}
            uniforms={{
              u_sky_top: { value: sky_top_color.clone() },
              u_sky_middle: { value: sky_middle_color.clone() },
              u_sky_horizon: { value: sky_horizon_color.clone() },
              u_sun_dir: { value: current_sun_position.current.clone().normalize() },
              u_sun_glow: { value: sun_glow_color.clone() },
              u_moon_dir: { value: moon_dir.clone() },
              u_time: { value: 0 },
              u_star_intensity: { value: 0 },
            }}
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
        castShadow={!is_night}
        shadow-mapSize={[2048, 2048]}
        /**
         * A ±768 frustum over a 2048-texel map was ~0.75 world units/texel —
         * coarser than a terrain cell (0.25u), so adjacent cells aliased to
         * the same texel and self-shadowed incorrectly (diagonal banding —
         * shadow-map resolution aliasing, not classic acne; a bias tweak
         * alone wouldn't fix it). Floating-origin keeps navigable terrain
         * near the scene origin, so shrinking to ±96 still covers normal-zoom
         * view while giving ~4 texels/cell — clean edges instead of moiré.
         * Zoomed out past DEFAULT_DIST, shadows just fade near the frustum
         * edge instead of covering the view (acceptable for now; a
         * camera-following frustum would be the real fix).
         *
         * The frustum's own hard edge showed up as a flat dark rectangle
         * (PCF clamping at the boundary instead of falling back to
         * unshadowed) — the terrain shader fades its own shadow contribution
         * to zero before SHADOW_HALF_EXTENT to hide that seam, so this
         * constant has to stay the actual frustum size for the fade to land
         * inside it.
         */
        shadow-camera-left={-SHADOW_HALF_EXTENT}
        shadow-camera-right={SHADOW_HALF_EXTENT}
        shadow-camera-top={SHADOW_HALF_EXTENT}
        shadow-camera-bottom={-SHADOW_HALF_EXTENT}
        shadow-camera-far={2400}
        shadow-bias={-0.0015}
        shadow-normalBias={0.05}
      />

      <directionalLight
        ref={moon_ref}
        color="#9fb4d6"
        intensity={is_night ? 0.18 : 0.0}
        position={moon_position}
        castShadow={is_night}
        shadow-mapSize={[1024, 1024]}
        // same resolution-aliasing fix and edge-fade coupling as the sun
        // above, scaled to this light's smaller 1024 map (still ~10.7
        // texels/unit at this span).
        shadow-camera-left={-SHADOW_HALF_EXTENT}
        shadow-camera-right={SHADOW_HALF_EXTENT}
        shadow-camera-top={SHADOW_HALF_EXTENT}
        shadow-camera-bottom={-SHADOW_HALF_EXTENT}
        shadow-camera-far={1200}
        shadow-bias={-0.0018}
        shadow-normalBias={0.07}
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
