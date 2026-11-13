// pxl-render/frame/shader/light_lay.wgsl

// Lays the world's depth from the light into the map, in the light box's own order
// (light_map.wgsl, light_mask.wgsl). The light's cast leaves one word for each slot of the
// clipmap (light.wgsl `light_cs`, pxl-engine `light::word`), and `light_depth`, which build.rs
// writes from that layout, reads the depth from it. Box texel p is held in the slot of its
// world texel, the box's corner plus p, modulo the map's side. A box tile that no cast of the
// reads' epoch has cast yet holds the depth of another light frame, or none. It is laid at the
// far plane, and so lit, until it is cast. Its read word (pxl-engine `light::reads_at`) names
// a cast before the first of the epoch (`light::reads_epoch`).

/// What the lay reads the list by (pass/shadow.rs `Shadow::begin`). `corner` holds the box's
/// corner, the host's epoch and where the reads lie. `read` holds where the list of tiles read
/// lies (`light::LIST_READS`), then the word of the first cast whose reads hold where a slide
/// went unrun (`light::READS_FROM`).
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
  let t = vec2u(clip.xy) / LIGHT_TILE;
  let across = u32(side.x) / LIGHT_TILE;
  let epoch = max(u32(lay.corner.z), reads[lay.read.y]);
  if (reads[u32(lay.corner.w) + t.y * across + t.x] < epoch) {
    return 1.0;
  }
  let slot = (vec2i(clip.xy) + lay.corner.xy) & (side - 1);
  return light_depth(textureLoad(world_depth, slot, 0, 0).x);
}
