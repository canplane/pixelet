// pxl-render/record_hits.light.wgsl

// The light as its holes' stops take it (record_hits.wgsl), as its samples' do
// (sample_box.light.wgsl): its stops past its pool, `ray::HITS_A_FRAME` at most
// (`light::hits_at`); a stop's node from where its slot's ray starts, whole where it is coarser
// than its slot's band (`record::light_whole`); and no faces' asks, as its slots read no face's
// word.

fn hits_at() -> u32 {
  return header_word(LIGHT_HEADER_HITS);
}

fn hits_cap() -> u32 {
  return HITS_A_FRAME;
}

/// The texel of hit `r`'s slot.
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

/// No room: the light's slots read no face's word.
fn hit_asks() -> Asks {
  return Asks(0u, 0u, 0u);
}
