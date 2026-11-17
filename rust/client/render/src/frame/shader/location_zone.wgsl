// Draws the zone unlit and see-through, with its outline opaque.
// location_zone_place.wgsl places it.

struct VertexOut {
  @builtin(position) clip: vec4f,
  // The fragment about the box's center, on a box of side one.
  @location(0) pos: vec3f,
  // Which of the face's edges carry a line, by the face across each: those at
  // the high end of x, y and z, and those at the low end.
  @location(1) @interpolate(flat) hi: vec3f,
  @location(2) @interpolate(flat) lo: vec3f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let k = v.i * LOCATION_ZONE_INSTANCE_FLOATS;
  let face = zone_face(v);
  // The face's four lines, a bit for each face across its edges, in the
  // faces' order with the face's own axis left out (pxl-core
  // domain/scene/indicator/location.rs `line_bit`).
  let lines = u32(zone[k + 4u]) >> (4u * face);
  var o: VertexOut;
  o.clip = zone_clip(v);
  o.pos = v.pos;
  var bit = 0u;
  for (var g = 0u; g < 6u; g++) {
    if (g / 2u == face / 2u) {
      continue;
    }
    let on = f32((lines >> bit) & 1u);
    if ((g & 1u) == 0u) {
      o.hi[g / 2u] = on;
    } else {
      o.lo[g / 2u] = on;
    }
    bit++;
  }
  return o;
}

// One where `dist` lies within `width` of an edge, antialiased. Along a face's
// own axis `dist` does not vary, and smoothstep is undefined over an empty
// span, so the span is never empty.
fn near(width: vec3f, dist: vec3f) -> vec3f {
  let w = max(fwidth(dist), vec3f(1e-6)) * 0.5;
  return 1.0 - smoothstep(width - w, width + w, dist);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let width = vec3f(LOCATION_ZONE_LINE_WIDTH);
  // How near the fragment lies to each edge, by the face across it.
  let hi = near(width, vec3f(0.5) - i.pos) * i.hi;
  let lo = near(width, vec3f(0.5) + i.pos) * i.lo;
  let m = max(hi, lo);
  let line = max(m.x, max(m.y, m.z));
  return vec4f(tone(LOCATION_ZONE_COLOR), mix(LOCATION_ZONE_FILL_OPACITY, 1.0, line));
}
