// pxl-render/frame/shader/decor_place.wgsl

// A decor instance's vertices placed, each instance read from the decor's
// instances (instance.wgsl): what the lit pass (decor.wgsl), the depth
// passes and the mark (tile_mark.wgsl) all lay alike.

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

// One function, so the depth passes place a vertex where the lit pass does.
struct Placed {
  world: vec3f,
  spin: f32,
};

fn placed(v: VertexIn) -> Placed {
  let i = instances[draw_idx() + v.i];
  let at = vec3f(lane(0u, i), lane(1u, i), lane(2u, i));
  var p: Placed;
  p.spin = lane(3u, i);
  let scale = lane(4u, i);
  p.world = place_up(v.pos * scale, p.spin) + at + draw_translation();
  return p;
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(placed(v).world, 1.0));
}
