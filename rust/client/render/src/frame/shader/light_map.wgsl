// The world's depth from the light, as its cast left it a word a slot of the
// clipmap (pxl-shader `light_cs`, pxl-spec-core `light::word`; its depth
// read as `light_depth`, which build.rs makes off its layout), laid into
// the light's shadow map in the light box's own order before the decor and
// the actors are drawn into it (gpu/shadow.rs): box texel p's slot is its
// world texel, the box's corner on by p, modulo the map's side. Laid in the
// map's claimed tiles alone, a quad a mark or a tile of one
// (screen_tiles.wgsl `mark_clip`; one of every tile where the map is drawn
// whole): its fragment writes its depth, so a stencil could not reject it
// early.

@group(0) @binding(0) var world_depth: texture_2d_array<u32>;
@group(0) @binding(1) var<uniform> corner: vec4i;

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> @builtin(position) vec4f {
  return mark_clip(vi, ii);
}

@fragment
fn fs(@builtin(position) clip: vec4f) -> @builtin(frag_depth) f32 {
  let side = vec2i(textureDimensions(world_depth));
  let slot = (vec2i(clip.xy) + corner.xy) & (side - 1);
  return light_depth(textureLoad(world_depth, slot, 0, 0).x);
}
