// The shading every lit pass shares, so a chunk, decor and an actor cannot be lit differently.

// The exponent only sets the band's width; rim_intensity scales the peak.
const RIM_POWER: f32 = 1.0;

fn rim_light(n: vec3f, world_pos: vec3f) -> vec3f {
  let view_dir = normalize(frame_camera_pos() - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER);
  return vec3f(1.0) * rim * frame_rim_intensity();
}

// `open` scales only the light from everywhere (ambient, hemisphere, second
// light), never the sun, whose visibility is the shadow map's.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  // three's standard material at roughness 1, metalness 0: Lambert, albedo over pi.
  let sun = frame_sun_color() * max(dot(n, frame_sun_dir()), 0.0) * lit;
  let other = frame_other_color() * max(dot(n, frame_other_dir()), 0.0);
  let hemi = mix(ground_bounce(seen.fill), seen.fill, n.z * 0.5 + 0.5);
  var color = albedo * (sun + (other + frame_ambient() + hemi) * open) / PI + albedo * frame_floor() * open;
  color += rim_light(n, world_pos);
  color = color * seen.through + seen.scattered;
  // A smoothstep on distance from the eye, a sphere, so the band hides the
  // ground before its drawn edge toward the screen's corners as at its middle.
  let fade = smoothstep(frame_fog_near(), frame_fog_far(), length(world_pos - frame_camera_pos()));
  return mix(color, seen.past, fade);
}
