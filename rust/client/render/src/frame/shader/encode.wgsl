// The vignette and the sRGB encoding the canvas is written in.

fn to_srgb(c: vec3f) -> vec3f {
  let x = clamp(c, vec3f(0.0), vec3f(1.0));
  let lo = x * 12.92;
  let hi = 1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, x <= vec3f(0.0031308));
}

fn vignette_of(frag: vec2f, viewport: vec2f, amount: f32) -> f32 {
  let d = distance(frag / viewport, vec2f(0.5));
  let dark = smoothstep(0.8, LENS_VIGNETTE_OFFSET * 0.799, d * (LENS_VIGNETTE_DARKNESS + LENS_VIGNETTE_OFFSET));
  return mix(1.0, dark, amount);
}
