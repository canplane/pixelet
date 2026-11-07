// pxl-render/frame/shader/light_lay.wgsl

// The world's depth from the light, as its cast left it a word a slot of the
// clipmap (light.wgsl `light_cs`, pxl-engine `light::word`; its depth
// read as `light_depth`, which build.rs makes off its layout), as the map
// lays it in the light box's own order (light_map.wgsl, light_gate.wgsl):
// box texel p's slot is its world texel, the box's corner on by p, modulo
// the map's side. A box tile no cast of the reads' epoch cast yet holds another light
// frame's depth, or none, and is laid at the far plane, lit until it is cast: its read word
// (pxl-engine `light::reads_at`) names a cast before the epoch's first (`light::reads_epoch`).

/// What the lay reads the list by (gpu/shadow.rs `Shadow::begin`): the box's corner, the
/// host's epoch and where the reads lie; then where the tiles read lie
/// (`light::LISTED_READ`) and the word of the first cast whose reads hold where a slide went
/// unrun (`light::READS_FROM`).
struct Lay {
  corner: vec4i,
  read: vec4u,
}

@group(0) @binding(0) var world_depth: texture_2d_array<u32>;
@group(0) @binding(1) var<uniform> lay: Lay;
@group(0) @binding(2) var<storage, read> reads: array<u32>;

// The depth the world's cast left at the map's texel under `clip`.
fn light_lay(clip: vec4f) -> f32 {
  let side = vec2i(textureDimensions(world_depth));
  let t = vec2u(clip.xy) / LAY_TILE;
  let across = u32(side.x) / LAY_TILE;
  let epoch = max(u32(lay.corner.z), reads[lay.read.y]);
  if (reads[u32(lay.corner.w) + t.y * across + t.x] < epoch) {
    return 1.0;
  }
  let slot = (vec2i(clip.xy) + lay.corner.xy) & (side - 1);
  return light_depth(textureLoad(world_depth, slot, 0, 0).x);
}
