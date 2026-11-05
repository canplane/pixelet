// pxl-render/light_resume.wgsl

// Where a ray of the shadow map starts: off the cell of the box its texel lies in
// (pxl-engine `light::cell`), as the eye's rays start off theirs (resume.wgsl).

/// The first word of the start of the cell box texel `texel` lies in.
fn light_cell_word(texel: vec2<u32>) -> u32 {
  let cell = texel / LIGHT_BEAM;
  return head_word(LIGHT_HEAD_CELLS) + (cell.y * head_word(LIGHT_HEAD_CELLS_ACROSS) + cell.x) * CELL_WORDS;
}

/// The ray `aim` of box texel `texel` at its cell's start.
fn light_resumed(aim: Aim, texel: vec2<u32>) -> Start {
  return resumed(aim, light_cell_word(texel), LIGHT_LINE_WORDS, LIGHT_LINE_ORIGIN);
}

/// The ray `aim` of box texel `texel` at `t`, where its walk over the node cache left it
/// (resume.wgsl `resumed_at`).
fn light_resumed_at(aim: Aim, texel: vec2<u32>, t: f32) -> Start {
  return resumed_at(aim, light_cell_word(texel), t, LIGHT_LINE_WORDS, LIGHT_LINE_ORIGIN);
}
