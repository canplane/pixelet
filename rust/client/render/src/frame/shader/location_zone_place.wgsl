// pxl-render/frame/shader/location_zone_place.wgsl

// Places the zone's boxes, one instance for each box, read from the zone's
// upload (pxl-core domain/scene/indicator/location.rs `lay_zone`): a box's
// center, the faces it shows and the lines it draws. The zone's draw
// (location_zone.wgsl) and the mark (tile_mark.wgsl) both place them here, so
// the mark covers the pixels the zone draws.

@group(2) @binding(0) var<storage, read> zone: array<f32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  // Four vertices for each face, the faces in the order pass/location.rs
  // `box_lanes` lays them out: +x, -x, +y, -y, +z, -z.
  @builtin(vertex_index) corner: u32,
  // The corner about the box's center, on a box of side one.
  @location(0) pos: vec3f,
};

fn zone_face(v: VertexIn) -> u32 {
  return v.corner / 4u;
}

// Only the faces the box shows are drawn. The ground hides a face it buries,
// and drawn, that face would fight the ground along the edges where they meet.
// A hidden face's corners all go to one point outside the view, so the face
// draws nothing.
fn zone_clip(v: VertexIn) -> vec4f {
  let k = v.i * LOCATION_ZONE_INSTANCE_FLOATS;
  let center = vec3f(zone[k], zone[k + 1u], zone[k + 2u]);
  let shows = ((u32(zone[k + 3u]) >> zone_face(v)) & 1u) != 0u;
  return select(vec4f(2.0, 2.0, 2.0, 1.0), jittered(frame_view_proj() * vec4f(v.pos + center + draw_translation(), 1.0)), shows);
}

@vertex
fn vs_place(v: VertexIn) -> @builtin(position) vec4f {
  return zone_clip(v);
}
