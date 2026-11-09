// pxl-render/record_hits.light.wgsl

// What record_hits.wgsl reads of the light, which takes its stops as its samples' stops are
// taken (sample_hit.light.wgsl). The light's holes' stops lie past its pool, at most
// `ray::HITS_A_FRAME` of them (pxl-engine `light::hits_at`). A stop's node is held from where
// its slot's ray starts, and the stop is whole where the node is coarser than its slot's band
// (`record::light_whole`).

fn hits_at() -> u32 {
  return header_word(LIGHT_HEADER_HITS);
}

fn hits_cap() -> u32 {
  return HITS_A_FRAME;
}

fn hit_texel(r: u32) -> vec2<u32> {
  let px = beams[hit_at(r) + HIT_PX];
  return slot_texel(vec2<u32>(px & 0xffffu, px >> 16u));
}

fn hit_band(r: u32) -> u32 {
  return texel_band(hit_texel(r));
}

fn hit_origin(r: u32) -> vec3<i32> {
  return light_aim(hit_texel(r)).origin;
}

fn hit_whole(k: Node, r: u32) -> bool {
  return k.zoom < hit_band(r);
}

/// The light opens no faces' asks, because its slots read no face's word.
fn hit_asks() -> AskList {
  return AskList(0u, 0u, 0u);
}
