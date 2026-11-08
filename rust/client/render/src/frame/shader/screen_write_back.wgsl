// pxl-render/frame/shader/screen_write_back.wgsl

// Copies the claimed tiles of the history the resolve wrote this frame back into the last
// history, a texel a pixel over the tiles' quads (screen_tile.wgsl). In a frame not drawn
// whole the last history stays the one read, so it must take this frame's tiles
// (gpu/resolve.rs `draw`).

@group(0) @binding(1) var written: texture_2d<f32>;

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return textureLoad(written, vec2i(v.clip.xy), 0);
}
