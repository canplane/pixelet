// The colour grade over everything, called by each pass on its linear
// colour before the sRGB encode: a brightness and contrast step then a
// saturation step, the formulas of the post stack three ran
// (archive/20260914-three-draw-paths). The numbers are cosmetic dials.

const GRADE_BRIGHTNESS: f32 = -0.01;
const GRADE_CONTRAST: f32 = 0.08;
const GRADE_SATURATION: f32 = 0.18;

fn grade(input: vec3f) -> vec3f {
  var c = input + GRADE_BRIGHTNESS;
  if (GRADE_CONTRAST > 0.0) {
    c = (c - 0.5) / (1.0 - GRADE_CONTRAST) + 0.5;
  } else {
    c = (c - 0.5) * (1.0 + GRADE_CONTRAST) + 0.5;
  }
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
