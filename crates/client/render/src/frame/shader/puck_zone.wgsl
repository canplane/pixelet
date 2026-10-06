// Unlit and half transparent, so the zone reads as an indicator rather than a surface.

const ZONE_FILL_OPACITY: f32 = 0.4;
const ZONE_BORDER_WIDTH: f32 = 0.04;

@group(2) @binding(0) var<storage, read> zone: array<f32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) position: vec3f,
  @location(1) uv: vec2f,
};
struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) borders: vec4f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let k = v.i * 7u;
  let center = vec3f(zone[k], zone[k + 1u], zone[k + 2u]);
  var o: VertexOut;
  o.clip = frame_view_proj() * vec4f(on_globe(v.position + center + draw_translation()), 1.0);
  o.uv = v.uv;
  o.borders = vec4f(zone[k + 3u], zone[k + 4u], zone[k + 5u], zone[k + 6u]);
  return o;
}

fn aastep(thres: f32, value: f32) -> f32 {
  let w = fwidth(value) * 0.5;
  return smoothstep(thres - w, thres + w, value);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let bw = ZONE_BORDER_WIDTH;
  let top = aastep(1.0 - bw, i.uv.y) * i.borders.x;
  let bottom = (1.0 - aastep(bw, i.uv.y)) * i.borders.y;
  let right = aastep(1.0 - bw, i.uv.x) * i.borders.z;
  let left = (1.0 - aastep(bw, i.uv.x)) * i.borders.w;
  let border = max(max(top, bottom), max(left, right));
  return vec4f(finish(draw_color(), i.clip.xy), mix(ZONE_FILL_OPACITY, 1.0, border));
}
