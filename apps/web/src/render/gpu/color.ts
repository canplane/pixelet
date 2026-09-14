// @pxl/web/render/gpu/color.ts

/** An sRGB hex colour (`#rrggbb`) in linear light, which is what every
 * colour the shaders light and blend with is in. */
export function linear_rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  const c = (v: number) => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return [c((n >> 16) & 255), c((n >> 8) & 255), c(n & 255)];
}
