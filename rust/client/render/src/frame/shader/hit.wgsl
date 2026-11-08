// pxl-render/hit.wgsl

// The stops of the holes' rays, which the walks keep. A hole is a pixel of the eye or a slot of
// the light that took neither a visible voxel nor the sky, and the holes' queue walks its ray
// alone. A stop takes `beam::HIT_WORDS` words (pxl-engine), and an observer keeps at most
// `ray::HITS_A_FRAME` of them. The eye keeps its stops past the beam rays' words, and the light
// keeps its own past its pool (`light::hits_at`). Each observer's part says where they lie and
// how many fit (`hits_at` and `hits_cap`, in queue.wgsl, record_hits.eye.wgsl and
// record_hits.light.wgsl).

fn hit_at(r: u32) -> u32 {
  return hits_at() + r * HIT_WORDS;
}
