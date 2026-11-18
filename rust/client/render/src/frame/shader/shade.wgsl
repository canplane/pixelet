// The shading every lit pass shares, so the screen's cells, decor and a character cannot be lit
// differently.

// How much a face brightens where it turns away from the eye, as a share of its own light. The
// rim ends at the effects' reach and fades before it (effect.wgsl `effect_share`).
fn rim(n: vec3f, world_pos: vec3f) -> f32 {
  let to_eye = frame_camera_pos() - world_pos;
  let share = effect_share(length(to_eye));
  return pow(1.0 - max(dot(n, normalize(to_eye)), 0.0), LIGHT_RIM_POWER) * frame_rim_intensity() * share;
}

// The shadow and the occlusion are both visibility, each applied to its own light
// (atmos_light.wgsl `surface_light_in`). The color is opaque up to the drawn edge: where
// ground is drawn, the imagery never shows through it. `lit` is the shadow map's
// visibility, which applies only to the light the map is cast from (shadow.wgsl
// `shadow_light`).
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, ao: f32, seen: Air, l: Light) -> vec3f {
  let color = albedo * surface_light_in(n, up_at(world_pos), lit_each(lit), ao, l, lights_now()) * (1.0 + rim(n, world_pos));
  return color * seen.transmittance + seen.scattered;
}

// `lit` split into the sun's visibility and the moon's: the light the shadow map is cast from
// takes `lit`, and the other takes 1.
fn lit_each(lit: f32) -> vec2f {
  return select(vec2f(lit, 1.0), vec2f(1.0, lit), frame_shadow_moon() > 0.0);
}

// A face of three.js's standard material. The share `metalness` of its light is mirrored,
// tinted by its color, at `roughness`, and the rest scatters as `shade` scatters it.
fn shade_metal(albedo: vec3f, metalness: f32, roughness: f32, n: vec3f, world_pos: vec3f, lit: f32, ao: f32, seen: Air, l: Light) -> vec3f {
  let v = normalize(frame_camera_pos() - world_pos);
  let f0 = mix(vec3f(0.04), albedo, metalness);
  let mirrored = surface_specular_in(n, v, f0, roughness, lit_each(lit), l, lights_now());
  return shade(albedo * (1.0 - metalness), n, world_pos, lit, ao, seen, l) + mirrored * seen.transmittance;
}
