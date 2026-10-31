// Unlit and half transparent, so the zone reads as an indicator rather than a
// surface; placed as puck_zone_place.wgsl lays it.

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) borders: vec4f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let k = v.i * PUCK_ZONE_INSTANCE_FLOATS;
  var o: VertexOut;
  o.clip = zone_clip(v);
  o.uv = v.uv;
  o.borders = vec4f(zone[k + 3u], zone[k + 4u], zone[k + 5u], zone[k + 6u]);
  return o;
}

fn aastep(thres: f32, val: f32) -> f32 {
  let w = fwidth(val) * 0.5;
  return smoothstep(thres - w, thres + w, val);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let bw = PUCK_ZONE_BORDER_WIDTH;
  let top = aastep(1.0 - bw, i.uv.y) * i.borders.x;
  let bottom = (1.0 - aastep(bw, i.uv.y)) * i.borders.y;
  let right = aastep(1.0 - bw, i.uv.x) * i.borders.z;
  let left = (1.0 - aastep(bw, i.uv.x)) * i.borders.w;
  let border = max(max(top, bottom), max(left, right));
  return vec4f(tone(draw_color()), mix(PUCK_ZONE_FILL_OPACITY, 1.0, border));
}
