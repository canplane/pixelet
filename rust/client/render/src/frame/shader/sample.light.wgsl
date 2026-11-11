// pxl-render/sample.light.wgsl

// What a pass over samples reads of the light's samples. The light takes a sample every
// `LIGHT_SAMPLE` slots across and down, only on the tiles the cast lists as fed
// (light_list.wgsl), and each sample's ray asks its tile's band.

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(hdr_word(LIGHT_HDR_SAMPLES_ACROSS));
}

/// The sample the thread at `id` takes, on one of the tiles listed as fed, or `NONE` past the
/// list (light_list.wgsl `light_item`).
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return light_item(LEVEL_SAMPLE, id);
}

/// The number of samples a run takes: those of the tiles listed as fed. A tile's samples take
/// consecutive places in the run, in the tile's order in the list (light_list.wgsl
/// `light_item_at`).
fn sample_cnt() -> u32 {
  let per = (LIGHT_TILE / LIGHT_SAMPLE) * (LIGHT_TILE / LIGHT_SAMPLE);
  let n = light_tiles();
  return in_cap(beams[list_cnt_at(LIGHT_LIST_SOURCES)], n * n) * per;
}

fn sample_queued(k: u32) -> vec2<u32> {
  return light_item_at(LEVEL_SAMPLE, k);
}

fn sample_word(s: vec2<u32>) -> u32 {
  return light_sample_at(s);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  return light_aim(slot_texel(s * LIGHT_SAMPLE));
}

/// The band the ray of sample `s` asks, which is its tile's (light_aim.wgsl `texel_band`).
fn sample_band(s: vec2<u32>) -> u32 {
  return texel_band(slot_texel(s * LIGHT_SAMPLE));
}

/// The light's samples have no line to ask faces' words from, because the light's slots read
/// none (sample_hit.light.wgsl `sample_asks`).
fn sample_line(s: vec2<u32>) -> u32 {
  return NONE;
}

/// Where the ray of sample `s` starts its walk: at the start of the light's cell its texel
/// lies in (light_resume.wgsl).
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return light_resumed(aim, slot_texel(s * LIGHT_SAMPLE));
}
