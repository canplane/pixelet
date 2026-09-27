// An entry of the billboard sheet, drawn into a texture of its own size
// (gpu/sheet.rs): one draw of instances, each a shape or a line of text, in
// the entry's pixels. A shape's edge is its signed distance's, so it is
// smooth at any size without a path to fill; a line of text is the
// platform's pixels, one layer of the text array each.
//
// Every instance comes out straight-alpha and is laid over what is under it,
// so the texture holds its colors premultiplied (badge.wgsl divides them out).

// kind: 0 a rounded rectangle filled, 1 one stroked, 2 a triangle, 3 a line
// of text. `rect` bounds it, in pixels. `p`: a rectangle's radius and stroke;
// a triangle's first two points; a text's layer and how much of the layer it
// covers. `q`: a triangle's third point.
struct Shape {
  rect: vec4f,
  color: vec4f,
  p: vec4f,
  q: vec4f,
  kind: vec4u,
};

@group(0) @binding(0) var<uniform> entry: vec4f;
@group(0) @binding(1) var<storage, read> shapes: array<Shape>;
@group(1) @binding(0) var text: texture_2d_array<f32>;
@group(1) @binding(1) var text_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) at: vec2f,
  @location(1) @interpolate(flat) shape: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  let s = shapes[ii];
  // A pixel past the bounds, for the edge's half of a pixel of coverage.
  let lo = s.rect.xy - vec2f(1.0);
  let hi = s.rect.zw + vec2f(1.0);
  let corner = vec2f(f32(vi & 1u), f32(vi >> 1u));
  let at = mix(lo, hi, corner);
  var o: VertexOut;
  o.clip = vec4f(at.x / entry.xy.x * 2.0 - 1.0, 1.0 - at.y / entry.xy.y * 2.0, 0.0, 1.0);
  o.at = at;
  o.shape = ii;
  return o;
}

// The signed distance to a box of half-size `half` about the origin, its
// corners rounded by `r`.
fn round_box(p: vec2f, half: vec2f, r: f32) -> f32 {
  let q = abs(p) - half + vec2f(r);
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// The signed distance to the triangle a, b, c.
fn triangle(p: vec2f, a: vec2f, b: vec2f, c: vec2f) -> f32 {
  let e0 = b - a;
  let e1 = c - b;
  let e2 = a - c;
  let v0 = p - a;
  let v1 = p - b;
  let v2 = p - c;
  let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
  let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
  let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
  let s = sign(e0.x * e2.y - e0.y * e2.x);
  let d = min(min(vec2f(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)), vec2f(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))), vec2f(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
  return -sqrt(d.x) * sign(d.y);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let s = shapes[i.shape];
  let center = (s.rect.xy + s.rect.zw) * 0.5;
  let half = (s.rect.zw - s.rect.xy) * 0.5;
  var d: f32;
  switch s.kind.x {
    case 0u: {
      d = round_box(i.at - center, half, s.p.x);
    }
    case 1u: {
      d = abs(round_box(i.at - center, half, s.p.x)) - s.p.y * 0.5;
    }
    case 2u: {
      d = triangle(i.at, s.p.xy, s.p.zw, s.q.xy);
    }
    default: {
      let uv = (i.at - s.rect.xy) / (s.rect.zw - s.rect.xy) * s.p.yz;
      let inside = all(i.at >= s.rect.xy) && all(i.at <= s.rect.zw);
      let texel = textureSampleLevel(text, text_sampler, uv, u32(s.p.x), 0.0);
      return select(vec4f(0.0), texel, inside);
    }
  }
  return vec4f(s.color.rgb, s.color.a * clamp(0.5 - d, 0.0, 1.0));
}
