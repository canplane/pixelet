// The dissolve a change of what a chunk draws goes through: its two sides —
// two tiers, or a stale chunk and what replaced it, with their decor — each
// drawn over a share of the pixels, on complementary halves of one pattern,
// so a pixel shows one of them and never both or neither.

// Interleaved gradient noise (Jimenez): screen-space on purpose, so a
// cross-fade reads as a fade and not as a texture stuck to the surface.
fn dither_threshold(frag: vec2f) -> f32 {
  return fract(52.9829189 * fract(dot(frag, vec2f(0.06711056, 0.00583715))));
}

// Whether the pixel at `frag` is the other side's: `visible` is the share
// this draw covers, or 2 + the share on the complementary half.
fn dissolved(visible: f32, frag: vec2f) -> bool {
  let inverted = visible > 1.5;
  let amount = select(visible, visible - 2.0, inverted);
  let pattern = dither_threshold(frag);
  return select(pattern >= amount, pattern < amount, inverted);
}
