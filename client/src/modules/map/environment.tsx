// @pxl/web/modules/map/environment.tsx

import { useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64 } from "@pixelet/shared/kit/word.ts";

import { render_cfg } from "@/modules/world/render/config.ts";
import { get_theme_key, themes, type WorldTheme, WorldThemeKey } from "@/modules/world/render/themes.ts";

const { FOG_NEAR, FOG_FAR } = render_cfg;

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

void main() {
  vec3 dir = normalize(v_world_pos);
  float h = clamp(dir.z * 0.5 + 0.5, 0.0, 1.0);

  vec3 color = mix(u_sky_horizon, u_sky_middle, smoothstep(0.0, 0.35, h));
  color = mix(color, u_sky_top, smoothstep(0.35, 1.0, h));

  // soft horizon lift
  float horizon_glow = 1.0 - smoothstep(0.0, 0.22, abs(h - 0.08));
  color += u_sky_horizon * horizon_glow * 0.06;

  // tiny sun glow
  float sun_dot = max(dot(dir, normalize(u_sun_dir)), 0.0);
  float sun_glow = pow(sun_dot, 12.0);
  color = mix(color, u_sun_glow, sun_glow * 0.18);

  gl_FragColor = vec4(color, 1.0);
}
`;

export function MapEnvironment() {
  const { scene } = useThree();

  const theme_key = get_theme_key();
  const theme: WorldTheme = themes[theme_key];
  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;

  const ambient_ref = useRef<THREE.AmbientLight>(null);
  const sun_ref = useRef<THREE.DirectionalLight>(null);
  const moon_ref = useRef<THREE.DirectionalLight>(null);
  const hemi_ref = useRef<THREE.HemisphereLight>(null);

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
  const sky_material_ref = useRef<THREE.ShaderMaterial>(null);

  useFrame((_, delta) => {
    if (!scene) return;

    const lerp_factor: f64 = delta * 1.5;

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
      uniforms.u_sun_dir.value.lerp(current_sun_position.current, lerp_factor).normalize();
      uniforms.u_sun_glow.value.lerp(sun_glow_color, lerp_factor);
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
      <mesh scale={400000} frustumCulled={false}>
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
          }}
        />
      </mesh>

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
        shadow-camera-left={-768}
        shadow-camera-right={768}
        shadow-camera-top={768}
        shadow-camera-bottom={-768}
        shadow-camera-far={2400}
        shadow-bias={-0.0008}
        shadow-normalBias={0.03}
      />

      <directionalLight
        ref={moon_ref}
        color="#9fb4d6"
        intensity={is_night ? 0.18 : 0.0}
        position={moon_position}
        castShadow={is_night}
        shadow-mapSize={[1024, 1024]}
        shadow-camera-left={-384}
        shadow-camera-right={384}
        shadow-camera-top={384}
        shadow-camera-bottom={-384}
        shadow-camera-far={1200}
        shadow-bias={-0.0012}
        shadow-normalBias={0.05}
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
