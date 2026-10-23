// The world's depth from the sun, as its cast left it a word a slot of the
// clipmap (pxl-shader `sun_cs`, pxl-spec-core `sun::Sun::word`; its depth
// read as `sun::depth_of` reads it, `sun_depth`), laid into
// the sun's shadow map in the light box's own order before the decor and
// the actors are drawn into it (gpu/shadow.rs): box texel p's slot is its
// world texel, the box's corner on by p, modulo the map's side.

@group(0) @binding(0) var world_depth: texture_2d_array<u32>;
@group(0) @binding(1) var<uniform> corner: vec4i;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  return vec4f(p, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  let side = vec2i(textureDimensions(world_depth));
  let slot = (vec2i(clip.xy) + corner.xy) & (side - 1);
  return sun_depth(textureLoad(world_depth, slot, 0, 0).x);
}
