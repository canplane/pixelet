// pxl-render/frame/shader/decor_place.wgsl

// Places a decor instance's vertices, reading each instance from the decor's
// instances (instance.wgsl). The lit pass (decor.wgsl), the depth passes and
// the mark (tile_mark.wgsl) all place them by it alike.

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

// A vertex placed in the world, and its instance's spin. Every pass places a
// vertex through `placed`, so the depth passes place it where the lit pass
// does.
struct Placement {
  world: vec3f,
  spin: f32,
};

fn placed(v: VertexIn) -> Placement {
  let i = order[instance_idx() + v.i];
  let at = vec3f(lane(0u, i), lane(1u, i), lane(2u, i));
  let spin = lane(3u, i);
  let scale = lane(4u, i);
  return Placement(place_up(v.pos * scale, spin) + at + draw_translation(), spin);
}

@vertex
fn vs_depth(v: VertexIn) -> @builtin(position) vec4f {
  return jittered(frame_view_proj() * vec4f(placed(v).world, 1.0));
}
