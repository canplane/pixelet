// pxl-render/frame/shader/ao_apply.wgsl

// The occlusion's map over the world's color, a texel a pixel, as the tiles'
// quads (screen_tiles.wgsl): black blended over by how much the map darkens,
// so what the static opaque draws laid is multiplied by it (gpu/ao.rs
// `apply`). A pixel the map leaves at 1 (the sky) is left as it is.

@group(0) @binding(1) var ao_map: texture_2d<f32>;

struct VertexOut {
  @builtin(position) clip: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  var o: VertexOut;
  o.clip = tile_clip(vi, ii);
  return o;
}

@fragment
fn fs(v: VertexOut) -> @location(0) vec4f {
  return vec4f(0.0, 0.0, 0.0, 1.0 - textureLoad(ao_map, vec2i(v.clip.xy), 0).r);
}
