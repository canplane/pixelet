// pxl-render/frame/shader/light_lay.wgsl

// The world's depth from the light, as its cast left it a word a slot of the
// clipmap (pxl-shader `light_cs`, pxl-spec-core `light::word`; its depth
// read as `light_depth`, which build.rs makes off its layout), as the map
// lays it in the light box's own order (light_map.wgsl, light_gate.wgsl):
// box texel p's slot is its world texel, the box's corner on by p, modulo
// the map's side.

@group(0) @binding(0) var world_depth: texture_2d_array<u32>;
@group(0) @binding(1) var<uniform> corner: vec4i;

// The depth the world's cast left at the map's texel under `clip`.
fn light_lay(clip: vec4f) -> f32 {
  let side = vec2i(textureDimensions(world_depth));
  let slot = (vec2i(clip.xy) + corner.xy) & (side - 1);
  return light_depth(textureLoad(world_depth, slot, 0, 0).x);
}
