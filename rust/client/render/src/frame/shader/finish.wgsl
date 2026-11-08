// A color reaches the screen through the tone curve, the vignette, the sRGB
// encoding and the dither, in that order. Nothing grades the picture past the
// tone curve, so a color shows only the light it was lit by. Each world pass
// tones what it lights (`tone`) and blends it into the world's color, which is
// not yet encoded. The screen's finish encodes that color once (`finish`).

// ACES's reference rendering and sRGB output transforms, as Stephen Hill fit
// them in BakingLab. Every pass tones by this one curve, so the sky and the
// ground reach the screen alike.
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
  return vignette_of(frag, frame_screen(), frame_vignette());
}

fn dither(q: vec2f) -> f32 {
  var p = fract(q * vec2f(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

// A toned color in the sRGB the canvas takes, darkened by `by`, the vignette
// where the color is drawn. The dither is added after.
fn displayed(toned: vec3f, by: f32) -> vec3f {
  return to_srgb(toned * by);
}

fn encoded(color: vec3f, by: f32) -> vec3f {
  return displayed(tone(color), by);
}

// The levels, 0 to 255, that the canvas writes for a linear color where the
// vignette leaves it whole.
fn levels(c: vec3f) -> vec3f {
  return encoded(c, 1.0) * 255.0;
}

fn most(c: vec3f) -> f32 {
  return max(max(c.r, c.g), c.b);
}

// The toned color the world's passes left at `frag`, encoded for the screen
// (screen_finish.wgsl). The screen keeps 8 bits a channel, so up to half a
// level of noise is added to break up the bands.
fn finish(toned: vec3f, frag: vec2f) -> vec3f {
  return displayed(toned, vignette(frag)) + (dither(frag) - 0.5) / 255.0;
}
