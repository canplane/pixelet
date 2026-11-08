// pxl-render/hit.wgsl

// The holes' stops the walks keep, an observer's among its words (pxl-engine
// `beam::HIT_WORDS`), `ray::HITS_A_FRAME` at most: the eye's past the beam rays', the light's
// past its pool (`light::hits_at`). Where they lie and their room are the observer's
// (`hits_at`, `hits_cap`: queue.wgsl, record_hits.eye.wgsl, record_hits.light.wgsl).

/// The first word of the hit numbered `r`.
fn hit_at(r: u32) -> u32 {
  return hits_at() + r * HIT_WORDS;
}
