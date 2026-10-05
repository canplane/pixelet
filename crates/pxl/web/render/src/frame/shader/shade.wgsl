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

// Jimenez's interleaved gradient noise: even over the screen and still in time,
// so a share of pixels left reads as that share without a history to average.
fn interleaved_gradient(frag: vec2f) -> f32 {
  return fract(52.9829189 * fract(dot(frag, vec2f(0.06711056, 0.00583715))));
}

// `open` scales all of the light, the sun's too: no shadow is drawn by default,
// and over the sky's light alone a corner went unmarked in the sun.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, frag: vec2f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  // Leave pixels to the sky pass's Earth only near the drawn edge.
  let fade = smoothstep(frame_fog_near(), frame_fog_far(), length(world_pos - frame_camera_pos()));
  if (interleaved_gradient(frag) < fade) {
    discard;
  }
  let color = albedo * surface_light(n, up_at(world_pos), lit, seen.light) * open * (1.0 + rim(n, world_pos));
  return color * seen.through + seen.scattered;
}
