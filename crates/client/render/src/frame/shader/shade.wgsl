// The shading every lit pass shares, so a chunk, decor and an actor cannot be lit differently.

// The exponent only sets the band's width; rim_intensity scales the peak.
const RIM_POWER: f32 = 1.0;

// How much a face brightens where it turns from the eye, as a share of its own
// light: the sky's light added to it shone a silhouette two or three times as
// bright as its face at night, when the sky is all the light there is.
fn rim(n: vec3f, world_pos: vec3f) -> f32 {
  let view_dir = normalize(frame_camera_pos() - world_pos);
  return pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER) * frame_rim_intensity();
}

// `open` scales all of the light, the sun's too: no shadow is drawn by default,
// and over the sky's light alone a corner went unmarked in the sun.
// Opaque to the drawn edge: where ground is drawn, the Earth's picture never shows through it.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, open: vec3f, seen: Seen) -> vec3f {
  let color = albedo * surface_light(n, up_at(world_pos), seen.light) * open * (1.0 + rim(n, world_pos));
  return color * seen.through + seen.scattered;
}
