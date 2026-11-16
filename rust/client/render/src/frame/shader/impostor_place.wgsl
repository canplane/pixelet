// pxl-render/frame/shader/impostor_place.wgsl

// Places an impostor's quad, turned about the world's up to face the eye. Each
// instance is read from the decor's instances (instance.wgsl). The lit pass
// (impostor.wgsl) and the mark (tile_mark.wgsl) both place the quad here, so
// the mark covers the pixels the impostor draws.

@group(2) @binding(2) var<uniform> fits: array<vec4f, DECOR_GROUP_CNT>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @builtin(vertex_index) k: u32,
};

// A placement is one corner of the quad: its point in the world, the way the
// quad faces, and the corner as -1 or 1 across and up.
struct Placement {
  world: vec3f,
  facing: vec3f,
  corner: vec2f,
};

fn placed(v: VertexIn) -> Placement {
  let i = order[instance_idx() + v.i];
  let base = vec3f(lane(0u, i), lane(1u, i), lane(2u, i)) + draw_translation();
  let scale = lane(4u, i);
  let fit = fits[instance_layer()];
  let corner = vec2f(strip_corner(v.k)) * 2.0 - 1.0;
  let up = vec3f(0.0, 0.0, 1.0);
  let to_eye = frame_camera_pos() - base;
  let flat = vec3f(to_eye.xy, 0.0);
  let facing = select(vec3f(0.0, -1.0, 0.0), normalize(flat), dot(flat, flat) > 1e-8);
  let side = cross(up, facing);
  let local = side * (fit.x + corner.x * fit.z) * scale + up * (fit.y + corner.y * fit.z) * scale;
  return Placement(base + local, facing, corner);
}

@vertex
fn vs_place(v: VertexIn) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(placed(v).world, 1.0));
}
