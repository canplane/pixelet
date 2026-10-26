// One sticker on the marker sheet: shapes by signed distance, text as the platform's
// pixels. Stored premultiplied; marker.wgsl divides it out.

// kind: 0 filled rounded rect, 1 stroked, 2 triangle, 3 text, 4 a text's halo.
// `p`: radius and a fill's edge width or a stroke's, a triangle's first two
// points, or text layer and coverage, and a halo's radius. `q`: a triangle's
// third point, a fill's shade, lighter at its top by as much as darker at
// its foot, or how far in from its radius a halo fades.
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

fn round_box(p: vec2f, half: vec2f, r: f32) -> f32 {
  let q = abs(p) - half + vec2f(r);
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

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
  var rgb = s.color.rgb;
  var edge = 1.0;
  switch s.kind.x {
    case 0u: {
      d = round_box(i.at - center, half, s.p.x);
      edge = max(s.p.y, 1.0);
      let down = (i.at.y - s.rect.y) / max(s.rect.w - s.rect.y, 1.0);
      rgb = rgb * (1.0 + s.q.x * (1.0 - 2.0 * down));
    }
    case 1u: {
      d = abs(round_box(i.at - center, half, s.p.x)) - s.p.y * 0.5;
    }
    case 2u: {
      d = triangle(i.at, s.p.xy, s.p.zw, s.q.xy);
    }
    case 4u: {
      // The glyphs' coverage grown by the radius: the most of it within reach,
      // each texel weighed by how much of it the disc covers.
      let size = vec2i(s.rect.zw - s.rect.xy);
      let base = vec2i(floor(i.at - s.rect.xy));
      let r = s.p.w;
      let n = i32(ceil(r));
      var a = 0.0;
      for (var dy = -n; dy <= n; dy++) {
        for (var dx = -n; dx <= n; dx++) {
          let w = clamp((r + 0.5 - length(vec2f(f32(dx), f32(dy)))) / max(s.q.x, 1.0), 0.0, 1.0);
          let q = base + vec2i(dx, dy);
          if (w > 0.0 && all(q >= vec2i(0)) && all(q < size)) {
            a = max(a, textureLoad(text, q, u32(s.p.x), 0).a * w);
          }
        }
      }
      return vec4f(s.color.rgb, s.color.a * a);
    }
    default: {
      let uv = (i.at - s.rect.xy) / (s.rect.zw - s.rect.xy) * s.p.yz;
      let inside = all(i.at >= s.rect.xy) && all(i.at <= s.rect.zw);
      let texel = textureSampleLevel(text, text_sampler, uv, u32(s.p.x), 0.0);
      return select(vec4f(0.0), texel, inside);
    }
  }
  return vec4f(rgb, s.color.a * clamp(0.5 - d / edge, 0.0, 1.0));
}
