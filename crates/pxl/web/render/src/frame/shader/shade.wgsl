// The shading every lit pass shares, so a chunk, decor and an actor cannot be lit differently.

// The exponent only sets the band's width; rim_intensity scales the peak.
const RIM_POWER: f32 = 1.0;

// The sky reflected where a face turns from the eye, so it dims with the sky
// at night and takes its color at dusk.
fn rim_light(n: vec3f, world_pos: vec3f, fill: vec3f) -> vec3f {
  let view_dir = normalize(frame_camera_pos() - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER);
  return fill / PI * rim * frame_rim_intensity();
}

// The share of the light that keeps its direction; the rest, what an up-facing
// face takes, reaches every face alike. Lit by the sun's direction alone, a
// matte wall turned from it fell to a fifth of the ground and read as a toy's.
const DIRECTED: f32 = 0.25;
// What a cast shadow takes of the sun in the light that reaches every face:
// all of it left the shade lit by the sky alone, as dark as the toy's.
const SHADOW_DEPTH: f32 = 0.5;

// `open` scales all of the light, the sun's too: no shadow is drawn by default,
// and over the sky's light alone a corner went unmarked in the sun.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  let e = exposure();
  let sun = frame_sun_color() * e;
  let other = frame_other_color() * e;
  // A face turned from the sun reads no shadow map, and is not its own shade.
  let shaded = select(1.0, mix(1.0, lit, SHADOW_DEPTH), dot(n, frame_sun_dir()) > 0.0);
  let up = sun * max(frame_sun_dir().z, 0.0) * shaded + other * max(frame_other_dir().z, 0.0) + seen.fill;
  // three's standard material at roughness 1, metalness 0: Lambert, albedo over pi.
  let hemi = mix(ground_bounce(seen.fill, e), seen.fill, n.z * 0.5 + 0.5);
  let directed = sun * max(dot(n, frame_sun_dir()), 0.0) * lit + other * max(dot(n, frame_other_dir()), 0.0) + hemi;
  var color = albedo * mix(up, directed, DIRECTED) * open / PI;
  color += rim_light(n, world_pos, seen.fill);
  color = color * seen.through + seen.scattered;
  // A smoothstep on distance from the eye, a sphere, so the band hides the
  // ground before its drawn edge toward the screen's corners as at its middle.
  let fade = smoothstep(frame_fog_near(), frame_fog_far(), length(world_pos - frame_camera_pos()));
  return mix(color, seen.past, fade);
}
