// pxl-render/sample.light.wgsl

// The shadow map as the samples' walks take it, off its list (light_list.wgsl): a sample every
// `LIGHT_SAMPLE` slots, taken off what the cast casts, each ray at its tile's band.

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(head_word(LIGHT_HEAD_SAMPLES_ACROSS));
}

/// The sample the thread at `id` takes: one of the tiles listed as fed, `NONE` past them
/// (light_list.wgsl `light_item`).
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return light_item(LEVEL_SAMPLE, id);
}

/// The samples a run takes: those of the tiles listed as fed, a sample's place its tile's
/// place's samples in order (light_list.wgsl `light_item_at`).
fn samples_listed() -> u32 {
  let per = (LIGHT_TILE / LIGHT_SAMPLE) * (LIGHT_TILE / LIGHT_SAMPLE);
  let n = light_tiles();
  return in_room(beams[listed_at(LIGHT_LISTED_FED)], n * n) * per;
}

fn sample_queued(k: u32) -> vec2<u32> {
  return light_item_at(LEVEL_SAMPLE, k);
}

/// A cast's samples start afresh: one the budget left parked owes nothing (samples_run.light.wgsl).
fn samples_resume() -> bool {
  return false;
}

/// Whether the cell of the box its texel lies in waits on a beam ray of its window not ended
/// (light_cell.wgsl `beam::CELL_WAITS`): the sample waits, parked, and the next cast casts it
/// afresh (`samples_resume`).
fn sample_waits(s: vec2<u32>) -> bool {
  return beams[light_cell_at(slot_texel(s * LIGHT_SAMPLE) / LIGHT_BEAM) + CELL_RESUMES] == CELL_WAITS;
}

/// The first word of sample `s` (light_list.wgsl `light_sample_at`).
fn sample_word(s: vec2<u32>) -> u32 {
  return light_sample_at(s);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  return light_aim(slot_texel(s * LIGHT_SAMPLE));
}

/// The band its ray asked, its tile's (light_aim.wgsl `texel_band`).
fn sample_band(s: vec2<u32>) -> u32 {
  return texel_band(slot_texel(s * LIGHT_SAMPLE));
}

/// None asked: the light's slots read no faces' words (sample_box.light.wgsl `sample_asks`).
fn sample_line(s: vec2<u32>) -> u32 {
  return NONE;
}

/// Off the cell of the box its texel lies in (light_resume.wgsl).
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return light_resumed(aim, slot_texel(s * LIGHT_SAMPLE));
}
