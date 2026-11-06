// pxl-render/hit.wgsl

// The holes' stops the walks keep, an observer's among its words (pxl-engine
// `beam::HIT_WORDS`), as many as its room: the eye's past the beam rays', `HITS` at most; the
// light's past its holes' region, a stop each of the slots its cast claims
// (`light::hits_at`). Where they lie and their room are the observer's (`hits_at`,
// `hits_room`: queue.wgsl, record_stops.eye.wgsl, record_stops.light.wgsl).

/// The first word of the hit numbered `r`.
fn hit_at(r: u32) -> u32 {
  return hits_at() + r * HIT_WORDS;
}
