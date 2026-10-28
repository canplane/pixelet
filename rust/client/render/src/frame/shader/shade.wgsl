// The shading every lit pass shares, so the screen's cells, decor and an actor cannot be lit differently.

// How much a face brightens where it turns from the eye, as a share of its own
// light: the sky's light added to it shone a silhouette two or three times as
// bright as its face at night, when the sky is all the light there is.
fn rim(n: vec3f, world_pos: vec3f) -> f32 {
  let view_dir = normalize(frame_camera_pos() - world_pos);
  return pow(1.0 - max(dot(n, view_dir), 0.0), LIGHT_RIM_POWER) * frame_rim_intensity();
}

// `open` scales all of the light, the sun's too: over the sky's light alone
// a corner went unmarked in the sun.
// Opaque to the drawn edge: where ground is drawn, the Earth's picture never shows through it.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  let color = albedo * surface_light_in(n, up_at(world_pos), lit, seen.light, lights_now()) * open * (1.0 + rim(n, world_pos));
  return color * seen.through + seen.scattered;
}
