// pxl-render/frame/shader/tile_quad.wgsl

// The vertex stage of a pass that draws the claimed tiles alone, or every one,
// a quad each (screen_tiles.wgsl `tile_clip`). A part of its own: screen_tiles.wgsl
// is also part of a compute shader, which runs no vertex stage.

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}
