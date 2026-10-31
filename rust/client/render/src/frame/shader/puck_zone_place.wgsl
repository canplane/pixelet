// pxl-render/frame/shader/puck_zone_place.wgsl

// The zone's boxes placed, an instance a box read from the zone's upload:
// what the lit pass (puck_zone.wgsl) and the mark (tile_mark.wgsl) lay alike.

@group(2) @binding(0) var<storage, read> zone: array<f32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  // Four a face, in the box's order (gpu/pass/puck.rs `box_lanes`).
  @builtin(vertex_index) corner: u32,
  @location(0) pos: vec3f,
  @location(1) uv: vec2f,
};

// Only the faces open to the air: one the ground buries stays unseen behind
// it rather than fight it along the edges it meets. A shut face's corners
// meet at one point outside the view, which draws nothing.
fn zone_clip(v: VertexIn) -> vec4f {
  let k = v.i * PUCK_ZONE_INSTANCE_FLOATS;
  let center = vec3f(zone[k], zone[k + 1u], zone[k + 2u]);
  let open = ((u32(zone[k + 7u]) >> (v.corner / 4u)) & 1u) != 0u;
  return select(vec4f(2.0, 2.0, 2.0, 1.0), jittered(frame_view_proj() * vec4f(v.pos + center + draw_translation(), 1.0)), open);
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return zone_clip(v);
}
