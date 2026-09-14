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

fn rim_light(n: vec3f, world_pos: vec3f) -> vec3f {
  let view_dir = normalize(frame.camera_pos - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER);
  return vec3f(1.0) * rim * frame.rim_intensity;
}

// `view_depth` is the fragment's depth along the view axis — 1 / the
// position builtin's w — the distance three's fog took, not the length of
// the ray: at equal depth a pixel at the screen's edge is no foggier than
// one at its centre.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, view_depth: f32, lit: f32) -> vec3f {
  // three's standard material, roughness 1, metalness 0: the Lambert BRDF
  // is the albedo over pi, for the direct sun and for the ambient and
  // hemisphere irradiance alike; the emissive floor is added as it is. The
  // two directional lights are summed as three summed them; the shadow is
  // the brighter's.
  let sun = frame.sun_color * max(dot(n, frame.sun_dir), 0.0) * lit;
  let other = frame.other_color * max(dot(n, frame.other_dir), 0.0);
  let hemi = mix(frame.hemi_ground, frame.hemi_sky, n.z * 0.5 + 0.5);
  var color = albedo * (sun + other + frame.ambient + hemi) / PI + albedo * frame.floor;
  color += rim_light(n, world_pos);
  // three's linear fog: a smoothstep over the band, not a ramp.
  let fog = smoothstep(frame.fog_near, frame.fog_far, view_depth);
  return mix(color, frame.fog_color, fog);
}

fn vignette(frag: vec2f) -> f32 {
  return vignette_of(frag, frame.viewport, frame.vignette);
}

fn finish(color: vec3f, frag: vec2f) -> vec3f {
  return to_srgb(grade(color) * vignette(frag));
}
