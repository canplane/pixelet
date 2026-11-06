// pxl-render/record_stops.light.wgsl

// The light as its holes' stops take it (record_stops.wgsl), as its samples'
// do (sample_box.light.wgsl): its stops past its holes' region, a stop each of the slots its
// cast claims (`light::hits_at`, `light::parked_room`); a stop's node from where its slot's ray
// starts, whole where it is coarser than its slot's band (`record::light_whole`); and no faces'
// asks, as its slots read no face's word.

fn hits_at() -> u32 {
  return head_word(LIGHT_HEAD_HITS);
}

fn hits_room() -> u32 {
  return LIGHT_PARKED_ROOM;
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

fn hit_whole(k: Kept, r: u32) -> bool {
  return k.zoom < hit_band(r);
}

/// No room: the light's slots read no face's word.
fn hit_asks() -> Asks {
  return Asks(0u, 0u, 0u);
}
