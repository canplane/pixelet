// Unlit and half transparent, so the zone reads as an indicator rather than a surface.

@group(2) @binding(0) var<storage, read> zone: array<f32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  // Four a face, in the box's order (gpu/pass/puck.rs `box_lanes`).
  @builtin(vertex_index) corner: u32,
  @location(0) pos: vec3f,
  @location(1) uv: vec2f,
};
struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) borders: vec4f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let k = v.i * PUCK_ZONE_INSTANCE_FLOATS;
  let center = vec3f(zone[k], zone[k + 1u], zone[k + 2u]);
  // Only the faces open to the air: one the ground buries stays unseen
  // behind it rather than fight it along the edges it meets. A shut face's
  // corners meet at one point outside the view, which draws nothing.
  let open = ((u32(zone[k + 7u]) >> (v.corner / 4u)) & 1u) != 0u;
  var o: VertexOut;
  o.clip = select(vec4f(2.0, 2.0, 2.0, 1.0), jittered(frame_view_proj() * vec4f(v.pos + center + draw_translation(), 1.0)), open);
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
