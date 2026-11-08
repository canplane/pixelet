// pxl-render/frame/shader/tile_quad.wgsl

// The vertex stage of a pass that draws a quad a tile, over the claimed tiles or over every
// tile (screen_tiles.wgsl `tile_clip`). It is a part of its own because screen_tiles.wgsl is
// also part of compute shaders, which run no vertex stage.

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}
