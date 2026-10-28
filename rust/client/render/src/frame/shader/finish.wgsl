// Tone, vignette, encode and dither, in that order: nothing grades the
// picture past the tone curve, so a color is the light it was lit by. Each
// world pass tones what it lights (`tone`) into the world's color, linear,
// blended there; the screen's finish encodes it once (`finish`).

// ACES's reference rendering and sRGB output transforms, as Stephen Hill fit
// them (BakingLab): the filmic curve Unreal draws with by default, whose
// shoulder takes the color out of what is bright, as film does. One for every
// pass, so the sky and the ground reach the screen by one curve.
const ACES_IN: mat3x3f = mat3x3f(
  vec3f(0.59719, 0.07600, 0.02840),
  vec3f(0.35458, 0.90834, 0.13383),
  vec3f(0.04823, 0.01566, 0.83777),
);
const ACES_OUT: mat3x3f = mat3x3f(
  vec3f(1.60475, -0.10208, -0.00327),
  vec3f(-0.53108, 1.10813, -0.07276),
  vec3f(-0.07367, -0.00605, 1.07602),
);

fn tone(input: vec3f) -> vec3f {
  let v = ACES_IN * input;
  let a = v * (v + 0.0245786) - 0.000090537;
  let b = v * (0.983729 * v + 0.4329510) + 0.238081;
  return clamp(ACES_OUT * (a / b), vec3f(0.0), vec3f(1.0));
}

fn vignette(frag: vec2f) -> f32 {
  return vignette_of(frag, frame_viewport(), frame_vignette());
}

fn dither(q: vec2f) -> f32 {
  var p = fract(q * vec2f(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

// A toned color as the canvas encodes it, darkened by `by`, the vignette
// where it is drawn: sRGB, before the dither.
fn displayed(toned: vec3f, by: f32) -> vec3f {
  return to_srgb(toned * by);
}

// A linear color as the canvas encodes it (`displayed`), toned first.
fn encoded(color: vec3f, by: f32) -> vec3f {
  return displayed(tone(color), by);
}

// The toned color the world's passes left at `frag`, encoded, half a step of
// noise added (screen_finish.wgsl): the screen keeps 8 bits, and a gentle
// gradient left whole, the sea's toward its horizon, broke into steps one
// apart, drawn out along it into lines.
fn finish(toned: vec3f, frag: vec2f) -> vec3f {
  return displayed(toned, vignette(frag)) + (dither(frag) - 0.5) / 255.0;
}
