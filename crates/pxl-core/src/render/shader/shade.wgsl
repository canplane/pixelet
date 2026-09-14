// The shading every lit pass shares, over `frame`: `shade` lights an albedo
// at a world point with a normal — the Lambert sun, ambient and hemisphere
// three's standard material summed (albedo over pi), the emissive floor,
// the rim light — then fogs it; `finish` grades, vignettes and encodes it.
// One source, so a chunk, a decor instance and an actor cannot be lit
// differently.

// pow(1 - N.V, RIM_POWER): the exponent does not change the peak (at the
// silhouette pow(1, n) is 1 whatever n), only how wide the lit band reads;
// frame.rim_intensity is the one that scales the peak.
const RIM_POWER: f32 = 1.0;
// postprocessing's VignetteEffect, its default technique: the darkness and
// the offset three's stack ran at.
const VIGNETTE_DARKNESS: f32 = 0.45;
const VIGNETTE_OFFSET: f32 = 0.3;

fn rim_light(n: vec3f, world_pos: vec3f) -> vec3f {
  let view_dir = normalize(frame.camera_pos - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER);
  return vec3f(1.0) * rim * frame.rim_intensity;
}

fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32) -> vec3f {
  // three's standard material, roughness 1, metalness 0: the Lambert BRDF
  // is the albedo over pi, for the direct sun and for the ambient and
  // hemisphere irradiance alike; the emissive floor is added as it is.
  let sun = frame.sun_color * max(dot(n, frame.sun_dir), 0.0) * lit;
  let hemi = mix(frame.hemi_ground, frame.hemi_sky, n.z * 0.5 + 0.5);
  var color = albedo * (sun + frame.ambient + hemi) / 3.14159265 + albedo * frame.floor;
  color += rim_light(n, world_pos);
  let view_depth = length(world_pos - frame.camera_pos);
  let fog = clamp((view_depth - frame.fog_near) / (frame.fog_far - frame.fog_near), 0.0, 1.0);
  return mix(color, frame.fog_color, fog);
}

fn vignette(frag: vec2f) -> f32 {
  let d = distance(frag / frame.viewport, vec2f(0.5));
  let dark = smoothstep(0.8, VIGNETTE_OFFSET * 0.799, d * (VIGNETTE_DARKNESS + VIGNETTE_OFFSET));
  return mix(1.0, dark, frame.vignette);
}

fn finish(color: vec3f, frag: vec2f) -> vec3f {
  return to_srgb(grade(color) * vignette(frag));
}
