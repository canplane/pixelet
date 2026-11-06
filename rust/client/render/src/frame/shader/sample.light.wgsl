// pxl-render/sample.light.wgsl

// The shadow map as the samples' walks take it, off its list (light_list.wgsl): a sample every
// `LIGHT_SAMPLE` slots, taken off what the cast casts, each ray at its tile's band.

const WALK_PASS: u32 = LIGHT_SAMPLES_PASS;
const VISITS_COUNT: u32 = LIGHT_SAMPLE_COUNT;

/// Where its asks are counted (pxl-engine `count::ASKED`).
/// Its walks ask whatever they meet: no budget is theirs.
fn may_ask() -> bool {
  return true;
}

fn asked_at() -> u32 {
  return LIGHT_ASKED_COUNT;
}

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(head_word(LIGHT_HEAD_SAMPLES_ACROSS));
}

/// The sample the thread at `id` walks: its own where the cast begins its epoch, else one of
/// the tiles listed as fed, `NONE` past them (light_list.wgsl `light_item`).
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return light_item(LEVEL_SAMPLE, id);
}

/// The first word of sample `s` (light_list.wgsl `light_sample_at`).
fn sample_word(s: vec2<u32>) -> u32 {
  return light_sample_at(s);
}

fn left_at() -> u32 {
  return head_word(LIGHT_HEAD_LEFTS);
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

/// Off its cell at `t`, where its walk over the node cache left it.
fn sample_start_at(aim: Aim, s: vec2<u32>, t: f32) -> Start {
  return light_resumed_at(aim, slot_texel(s * LIGHT_SAMPLE), t);
}
