// The puck's beacon: a small flat-shaded octahedron in the day or night
// colour, glowing as its emissive material did.

// #ff1493 and #00f2ff, in linear light.
const BEACON_DAY: vec3f = vec3f(1.0, 0.006995, 0.291771);
const BEACON_NIGHT: vec3f = vec3f(0.0, 0.887923, 1.0);

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
};

fn spin(v: vec3f) -> vec3f {
  let c = cos(draw.fourth);
  let s = sin(draw.fourth);
  return vec3f(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

@vertex
fn vs(@location(0) position: vec3f, @location(1) normal: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(spin(position) + draw.translation, 1.0);
  o.normal = spin(normal);
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let colour = select(BEACON_DAY, BEACON_NIGHT, draw.night > 0.5);
  let glow = select(0.25, 1.0, draw.night > 0.5);
  let n = normalize(i.normal);
  // Mostly metal, which three lit with no environment to reflect: a little
  // diffuse under the lights, and the emissive glow.
  let lit = colour * (frame.sun_color * max(dot(n, frame.sun_dir), 0.0) + frame.other_color * max(dot(n, frame.other_dir), 0.0) + frame.ambient) / PI * 0.2;
  return vec4f(finish(lit + colour * glow, i.clip.xy), 1.0);
}
