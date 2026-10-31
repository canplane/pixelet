// pxl-render/frame/shader/impostor_place.wgsl

// An impostor's quad placed, turned to the eye about the world's up, each
// instance read from the decor's instances (instance.wgsl): what the lit pass
// (impostor.wgsl) and the mark (tile_mark.wgsl) lay alike.

@group(2) @binding(2) var<uniform> fits: array<vec4f, DECOR_GROUPS>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @builtin(vertex_index) k: u32,
};

// Corner `k` of the quad: its world point, the way it faces and the corner
// itself.
struct Placed {
  world: vec3f,
  facing: vec3f,
  corner: vec2f,
};

fn placed(v: VertexIn) -> Placed {
  let i = instances[draw_idx() + v.i];
  let base = vec3f(lane(0u, i), lane(1u, i), lane(2u, i)) + draw_translation();
  let scale = lane(4u, i);
  let fit = fits[draw_layer()];
  var p: Placed;
  p.corner = vec2f(select(-1.0, 1.0, (v.k & 1u) != 0u), select(-1.0, 1.0, (v.k & 2u) != 0u));
  let up = vec3f(0.0, 0.0, 1.0);
  let to_eye = frame_camera_pos() - base;
  let flat = vec3f(to_eye.xy, 0.0);
  p.facing = select(vec3f(0.0, -1.0, 0.0), normalize(flat), dot(flat, flat) > 1e-8);
  let side = cross(up, p.facing);
  let local = side * (fit.x + p.corner.x * fit.z) * scale + up * (fit.y + p.corner.y * fit.z) * scale;
  p.world = base + local;
  return p;
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(placed(v).world, 1.0));
}
