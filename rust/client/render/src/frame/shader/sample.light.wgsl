// pxl-render/sample.light.wgsl

// The shadow map as the samples' walks take it, off its list (light_list.wgsl): a sample every
// `LIGHT_SAMPLE` slots, taken off what the cast casts, each ray at its tile's band.

const WALK_PASS: u32 = LIGHT_SAMPLES_PASS;
const VISITS_COUNT: u32 = LIGHT_SAMPLE_COUNT;
const ASKED_COUNT: u32 = LIGHT_ASKED_COUNT;

fn sample_lattice() -> vec2<u32> {
  return vec2<u32>(head_word(LIGHT_HEAD_SAMPLES_ACROSS));
}

/// The sample the thread at `id` walks: its own where the cast begins its epoch, else one of
/// the tiles listed as fed, `NONE` past them (light_list.wgsl `light_item`).
fn sample_item(id: vec2<u32>) -> vec2<u32> {
  return light_item(LEVEL_SAMPLE, id);
}

/// Whether a tile sample `s` may feed, its own and those about it, is one this cast casts
/// (light_list.wgsl `light_needs`).
fn sample_needed(s: vec2<u32>) -> bool {
  return light_needs(LEVEL_SAMPLE, s);
}

/// The first word of sample `s` (light_list.wgsl `light_sample_at`).
fn sample_word(s: vec2<u32>) -> u32 {
  return light_sample_at(s);
}

fn left_at() -> u32 {
  return head_word(LIGHT_HEAD_LEFTS);
}

fn sample_aim(s: vec2<u32>) -> Aim {
  return light_aim(slot_texel(s * LIGHT_SAMPLE), mix_hash(head_word(LIGHT_HEAD_CAST), s.x | (s.y << 16u)));
}

/// Off the cell of the box its texel lies in (light_resume.wgsl).
fn sample_start(aim: Aim, s: vec2<u32>) -> Start {
  return light_resumed(aim, slot_texel(s * LIGHT_SAMPLE));
}

/// Off its cell at `t`, where its walk over the node cache left it.
fn sample_start_at(aim: Aim, s: vec2<u32>, t: f32) -> Start {
  return light_resumed_at(aim, slot_texel(s * LIGHT_SAMPLE), t);
}
