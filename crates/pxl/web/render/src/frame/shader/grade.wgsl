// Brightness and contrast in sRGB, then saturation in linear light, as
// postprocessing's effects ran. Contrast in linear light would crush night colors to black.

const GRADE_BRIGHTNESS: f32 = -0.01;
const GRADE_CONTRAST: f32 = 0.08;
const GRADE_SATURATION: f32 = 0.18;
// postprocessing's VignetteEffect defaults.
const VIGNETTE_DARKNESS: f32 = 0.45;
const VIGNETTE_OFFSET: f32 = 0.3;

fn grade(input: vec3f) -> vec3f {
  var c = to_srgb(input) + GRADE_BRIGHTNESS;
  if (GRADE_CONTRAST > 0.0) {
    c = (c - 0.5) / (1.0 - GRADE_CONTRAST) + 0.5;
  } else {
    c = (c - 0.5) * (1.0 + GRADE_CONTRAST) + 0.5;
  }
  c = from_srgb(c);
  let average = (c.r + c.g + c.b) / 3.0;
  if (GRADE_SATURATION > 0.0) {
    c += (average - c) * (1.0 - 1.0 / (1.001 - GRADE_SATURATION));
  } else {
    c += (average - c) * (-GRADE_SATURATION);
  }
  return c;
}

fn to_srgb(c: vec3f) -> vec3f {
  let x = clamp(c, vec3f(0.0), vec3f(1.0));
  let lo = x * 12.92;
  let hi = 1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, x <= vec3f(0.0031308));
}

fn from_srgb(c: vec3f) -> vec3f {
  let x = clamp(c, vec3f(0.0), vec3f(1.0));
  let lo = x / 12.92;
  let hi = pow((x + 0.055) / 1.055, vec3f(2.4));
  return select(hi, lo, x <= vec3f(0.04045));
}

fn vignette_of(frag: vec2f, viewport: vec2f, amount: f32) -> f32 {
  let d = distance(frag / viewport, vec2f(0.5));
  let dark = smoothstep(0.8, VIGNETTE_OFFSET * 0.799, d * (VIGNETTE_DARKNESS + VIGNETTE_OFFSET));
  return mix(1.0, dark, amount);
}
