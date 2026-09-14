// @pxl/web/render/gpu/grade.ts

/**
 * The colour grade over everything, as a WGSL function each pass calls on
 * its linear colour before the sRGB encode: a brightness and contrast
 * step then a saturation step at post_cfg's values, the formulas of the
 * post stack three ran (archive/20260914-three-draw-paths). The vignette (a 4.5 % darken at
 * the corners) waits for a pass of its own.
 */

import { post_cfg } from "@/module/world/environment/config.ts";

const f = (v: number) => (Number.isInteger(v) ? `${v}.0` : `${v}`);

export const GRADE_WGSL: string = /* wgsl */ `
fn grade(input: vec3f) -> vec3f {
  var c = input + ${f(post_cfg.GRADE.BRIGHTNESS)};
  let contrast = ${f(post_cfg.GRADE.CONTRAST)};
  if (contrast > 0.0) {
    c = (c - 0.5) / (1.0 - contrast) + 0.5;
  } else {
    c = (c - 0.5) * (1.0 + contrast) + 0.5;
  }
  let saturation = ${f(post_cfg.GRADE.SATURATION)};
  let average = (c.r + c.g + c.b) / 3.0;
  if (saturation > 0.0) {
    c += (average - c) * (1.0 - 1.0 / (1.001 - saturation));
  } else {
    c += (average - c) * (-saturation);
  }
  return c;
}

fn to_srgb(c: vec3f) -> vec3f {
  let x = clamp(c, vec3f(0.0), vec3f(1.0));
  let lo = x * 12.92;
  let hi = 1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, x <= vec3f(0.0031308));
}
`;
