// The shading every lit pass shares, over `frame` and the air
// (atmos_view.wgsl): `shade` lights an albedo at a world point with a
// normal — the Lambert sun, ambient and hemisphere three's standard
// material summed (albedo over pi), the emissive floor, the rim light —
// then sees it through the air between it and the eye. One source, so a
// chunk, a decor instance and an actor cannot be lit differently.

// pow(1 - N.V, RIM_POWER): the exponent does not change the peak (at the
// silhouette pow(1, n) is 1 whatever n), only how wide the lit band reads;
// frame.rim_intensity is the one that scales the peak.
const RIM_POWER: f32 = 1.0;

fn rim_light(n: vec3f, world_pos: vec3f) -> vec3f {
  let view_dir = normalize(frame.camera_pos - world_pos);
  let rim = pow(1.0 - max(dot(n, view_dir), 0.0), RIM_POWER);
  return vec3f(1.0) * rim * frame.rim_intensity;
}

// `open` is how much of the ambient sky the point sees, per channel, 1 for
// all of it: the corner occlusion a chunk computes from
// its own geometry (chunk.wgsl's occlusion), tinted warm so a corner reads
// as a contact shadow and not as a rendering artefact. It scales the
// ambient, hemisphere and second light, which come from everywhere, and
// not the sun, whose visibility is the shadow map's to say.
//
// `seen` is the air in front of the point, as its vertices took it
// (atmos_view.wgsl's seen_from_eye) and the rasterizer blended it.
fn shade(albedo: vec3f, n: vec3f, world_pos: vec3f, lit: f32, open: vec3f, seen: Seen) -> vec3f {
  // three's standard material, roughness 1, metalness 0: the Lambert BRDF
  // is the albedo over pi, for the direct sun and for the ambient and
  // hemisphere irradiance alike; the emissive floor is added as it is. The
  // two directional lights are summed as three summed them; the shadow is
  // the brighter's.
  let sun = frame.sun_color * max(dot(n, frame.sun_dir), 0.0) * lit;
  let other = frame.other_color * max(dot(n, frame.other_dir), 0.0);
  // The hemisphere: the ground's bounce below, the sky's own light above,
  // both the air's (atmos_view.wgsl's ground_bounce and sky_fill, the latter
  // as the vertices took it).
  let hemi = mix(ground_bounce(seen.fill), seen.fill, n.z * 0.5 + 0.5);
  var color = albedo * (sun + (other + frame.ambient + hemi) * open) / PI + albedo * frame.floor * open;
  color += rim_light(n, world_pos);
  color = color * seen.through + seen.scattered;
  // Past the band the ground is not drawn, so it fades out before it ends:
  // into what the sky pass draws past it along the same ray, so the edge is
  // not a seam. A smoothstep over the band, not a ramp, on the distance from
  // the eye: the band is a sphere round it, which meets the ground inside
  // the circle round the eye's foot that the window is drawn out to
  // (terrain/level.rs's window), so the drawn edge lies past where the band
  // has hidden the ground, toward every corner of the screen as toward its
  // middle — where a depth along the view axis reaches a corner's ground
  // further out than its middle's.
  let fade = smoothstep(frame.fog_near, frame.fog_far, length(world_pos - frame.camera_pos));
  return mix(color, seen.past, fade);
}
