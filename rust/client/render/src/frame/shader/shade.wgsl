// The shading every lit pass shares, so the screen's cells, decor and an actor cannot be lit differently.

// How much a face brightens where it turns from the eye, as a share of its own
// light: the sky's light added to it shone a silhouette two or three times as
// bright as its face at night, when the sky is all the light there is.
fn rim(n: vec3f, world_pos: vec3f) -> f32 {
  let view_dir = normalize(frame_camera_pos() - world_pos);
  return pow(1.0 - max(dot(n, view_dir), 0.0), LIGHT_RIM_POWER) * frame_rim_intensity();
}

// The shadow and the occlusion are both visibility, each of its own light
// (atmos_light.wgsl `surface_light_in`): one rule for one kind of thing. Gray
// occlusion over all the light dimmed the direct sun too, which a warm tint
// once made up for; a fixed tint is wrong under a sunset, the moon or the
// night, and with the sun left alone the warmth in a corner is the light's.
// Opaque to the drawn edge: where ground is drawn, the Earth's picture never shows through it.
// `lit` is the shadow map's, the light it is cast from's alone (shadow.wgsl
// `shadow_light`).
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: f32, seen: Seen) -> vec3f {
  let each = select(vec2f(lit, 1.0), vec2f(1.0, lit), frame_shadow_moon() > 0.0);
  let color = albedo * surface_light_in(n, up_at(world_pos), each, open, seen.light, lights_now()) * (1.0 + rim(n, world_pos));
  return color * seen.through + seen.scattered;
}
