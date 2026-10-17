// The planet's map: a point about the planet taken back to the flat map it lies over.

// Drops the height. atan2 is held to its ulps, not 2^-11.
fn off_globe_of(p: vec3f, planet: vec4f, globe: vec4f) -> vec2f {
  let r = planet.z;
  if (r <= 0.0) {
    return p.xy - planet.xy;
  }
  let v = vec3f(p.xy - planet.xy, p.z - planet.w + r);
  let axis = v.y * globe.z + v.z * globe.y;
  let meridian = -v.y * globe.y + v.z * globe.z;
  let across = length(vec2f(v.x, meridian));
  let e = (length(v) + axis) / across;
  return vec2f(globe.w * atan2(v.x, meridian), globe.w * log(e / globe.x));
}
