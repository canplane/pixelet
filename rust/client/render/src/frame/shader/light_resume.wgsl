// pxl-render/light_resume.wgsl

// A ray of the shadow map starts from the cell of the box its texel lies in (pxl-engine
// `light::cell`), as the eye's rays start from theirs (resume.wgsl).

/// The ray `aim` of box texel `texel` at the start of the cell it lies in.
fn light_resumed(aim: Aim, texel: vec2<u32>) -> Start {
  return resumed(aim, light_cell_at(texel / LIGHT_BEAM), LIGHT_LINE_WORDS, LIGHT_LINE_ORIGIN);
}

/// The ray `aim` of box texel `texel` at `t`, at or past the start of the cell it lies in
/// (resume.wgsl `resumed_at`).
fn light_resumed_at(aim: Aim, texel: vec2<u32>, t: f32) -> Start {
  return resumed_at(aim, light_cell_at(texel / LIGHT_BEAM), t, LIGHT_LINE_WORDS, LIGHT_LINE_ORIGIN);
}
