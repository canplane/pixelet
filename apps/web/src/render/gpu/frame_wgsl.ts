// @pxl/web/render/gpu/frame_wgsl.ts

/**
 * The frame's uniform as every pass declares it, and the shading every lit
 * pass shares: the Lambert sun, ambient and hemisphere three's standard
 * material summed (albedo over pi), the emissive floor, the rim light, the
 * sun's shadow, the fog, and the vignette over the screen — one source, so
 * a chunk, a decor instance and an actor cannot be lit differently.
 *
 * The layout is stage.tsx's to fill; FRAME_UNIFORM_FLOATS counts it.
 */

import { light_cfg, post_cfg, rim_cfg } from "@/module/world/environment/config.ts";

const f = (v: number) => (Number.isInteger(v) ? `${v}.0` : `${v}`);

/** Floats the frame uniform holds. */
export const FRAME_UNIFORM_FLOATS = 68;

/** Where each field starts, in floats. */
export const FRAME_AT = {
  view_proj: 0,
  camera_pos: 16,
  edge_strength: 19,
  /** The environment's run: sun_dir through fog_color (environment.rs). */
  env: 20,
  low_spec: 23,
  edge_width: 39,
  rim_intensity: 43,
  viewport: 44,
  shadow_strength: 46,
  shadow_texel: 47,
  /** 1 to darken the screen's corners, 0 to leave them. */
  vignette: 48,
  light_view_proj: 52,
} as const;

export const FRAME_WGSL: string = /* wgsl */ `
struct Frame {
  view_proj: mat4x4f,
  camera_pos: vec3f,
  edge_strength: f32,
  sun_dir: vec3f,
  low_spec: f32,
  sun_color: vec3f,
  fog_near: f32,
  ambient: vec3f,
  fog_far: f32,
  hemi_sky: vec3f,
  floor: f32,
  hemi_ground: vec3f,
  edge_width: f32,
  fog_color: vec3f,
  rim_intensity: f32,
  viewport: vec2f,
  shadow_strength: f32,
  shadow_texel: f32,
  vignette: f32,
  _p0: f32,
  _p1: f32,
  _p2: f32,
  light_view_proj: mat4x4f,
};
`;

/**
 * The shading, over \`frame\` and, where the pass binds one, the sun's shadow
 * map at group SHADOW_GROUP: \`shade\` lights an albedo at a world point with
 * a normal, then fogs it; \`finish\` grades, vignettes and encodes it.
 */
export const SHADE_WGSL: string = /* wgsl */ `
fn rim_light(n: vec3f, world_pos: vec3f) -> vec3f {
  let view_dir = normalize(frame.camera_pos - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), ${f(rim_cfg.POWER)});
  return vec3f(${f(1)}) * rim * frame.rim_intensity;
}

fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32) -> vec3f {
  // three's standard material, roughness 1, metalness 0: the Lambert BRDF
  // is the albedo over pi, for the direct sun and for the ambient and
  // hemisphere irradiance alike; the emissive floor is added as it is.
  let sun = frame.sun_color * max(dot(n, frame.sun_dir), 0.0) * lit;
  let hemi = mix(frame.hemi_ground, frame.hemi_sky, n.z * 0.5 + 0.5);
  var color = albedo * (sun + frame.ambient + hemi) / 3.14159265 + albedo * ${f(light_cfg.FLOOR)};
  color += rim_light(n, world_pos);
  let view_depth = length(world_pos - frame.camera_pos);
  let fog = clamp((view_depth - frame.fog_near) / (frame.fog_far - frame.fog_near), 0.0, 1.0);
  return mix(color, frame.fog_color, fog);
}

// postprocessing's VignetteEffect, its default technique, at post_cfg's
// darkness and offset, over the screen position.
fn vignette(frag: vec2f) -> f32 {
  let d = distance(frag / frame.viewport, vec2f(0.5));
  let dark = smoothstep(0.8, ${f(post_cfg.VIGNETTE.OFFSET)} * 0.799, d * (${f(post_cfg.VIGNETTE.DARKNESS)} + ${f(post_cfg.VIGNETTE.OFFSET)}));
  return mix(1.0, dark, frame.vignette);
}

fn finish(color: vec3f, frag: vec2f) -> vec3f {
  return to_srgb(grade(color) * vignette(frag));
}
`;

/**
 * The sun's shadow: how much of the sun reaches a world point, read off
 * the shadow map with a 3x3 percentage-closer filter, a texel of slope
 * bias; 1 where the map does not reach. Bound beside the frame uniform, at
 * `binding` and the next, in whichever group a pass keeps the frame in.
 */
export const SHADOW_WGSL = (group: number, binding: number): string => /* wgsl */ `
@group(${group}) @binding(${binding}) var shadow_map: texture_depth_2d;
@group(${group}) @binding(${binding + 1}) var shadow_sampler: sampler_comparison;

fn sun_lit(world_pos: vec3f, n: vec3f) -> f32 {
  if (frame.shadow_strength <= 0.0) {
    return 1.0;
  }
  // Pushed along the normal by a texel, so a surface does not shadow
  // itself where the map's texel straddles it.
  let p = frame.light_view_proj * vec4f(world_pos + n * frame.shadow_texel, 1.0);
  let ndc = p.xyz / p.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0 || ndc.z < 0.0) {
    return 1.0;
  }
  let step = 1.0 / vec2f(textureDimensions(shadow_map));
  var lit = 0.0;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2f(f32(x), f32(y)) * step, ndc.z);
    }
  }
  return mix(1.0, lit / 9.0, frame.shadow_strength);
}
`;
