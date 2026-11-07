// pxl-render/frame/shader/screen_write_back.wgsl

// The claimed tiles of the history the resolve wrote this frame copied back
// into the last, which stays the one read where the frame is not drawn whole
// (gpu/resolve.rs `draw`): a texel a pixel, as the tiles' quads
// (screen_tiles.wgsl).

@group(0) @binding(1) var written: texture_2d<f32>;

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return textureLoad(written, vec2i(v.clip.xy), 0);
}
