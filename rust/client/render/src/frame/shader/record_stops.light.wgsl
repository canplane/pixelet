// pxl-render/record_stops.light.wgsl

// The light as its holes' stops into its record take it (record_stops.wgsl), as its samples'
// do (sample_box.light.wgsl): its stops and those put off past its holes' region, a stop each
// of the slots its cast claims (`light::hits_at`, `light::defers_at`, `light::parked_room`); a
// stop keyed at its slot's band from where its slot's ray starts, whole where it is coarser
// than its band (`record::light_whole`); and no faces' asks, as its slots read no face's word.

fn hits_at() -> u32 {
  return head_word(LIGHT_HEAD_HITS);
}

fn defers_at() -> u32 {
  return head_word(LIGHT_HEAD_DEFERS);
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
  return k.zoom < k.band;
}

/// No room: the light's slots read no face's word.
fn hit_asks() -> Asks {
  return Asks(0u, 0u, 0u);
}
