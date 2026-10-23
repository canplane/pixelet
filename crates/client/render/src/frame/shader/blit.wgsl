// The world image laid on a frame whole, its colour and its depth (gpu/renderer.rs,
// stage 3): what moves on its own is drawn over it against the world's depth,
// and nothing drawn over it outlives the frame.

@group(0) @binding(0) var world_colour: texture_2d<f32>;
@group(0) @binding(1) var world_depth: texture_depth_2d;

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  return vec4f(p[vi], 0.0, 1.0);
}

struct Laid {
  @location(0) colour: vec4f,
  @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(@builtin(position) at: vec4f) -> Laid {
  let texel = vec2i(at.xy);
  var o: Laid;
  o.colour = textureLoad(world_colour, texel, 0);
  o.depth = textureLoad(world_depth, texel, 0);
  return o;
}
