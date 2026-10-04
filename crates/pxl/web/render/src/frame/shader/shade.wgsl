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
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  var color = albedo * surface_light(n, lit, seen.light) * open * (1.0 + rim(n, world_pos));
  color = color * seen.through + seen.scattered;
  // A smoothstep on distance from the eye, a sphere, so the band hides the
  // ground before its drawn edge toward the screen's corners as at its middle;
  // and the whole view gives way to the Earth past it as it goes over to it.
  let fade = max(smoothstep(frame_fog_near(), frame_fog_far(), length(world_pos - frame_camera_pos())), frame_earth_share());
  return mix(color, seen.past, fade);
}
