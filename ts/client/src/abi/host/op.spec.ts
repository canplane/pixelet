// @pxl/client/abi/host/op.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json

/** weight, px (f32 bits), max width (f32 bits, 0 for one line), at, lines, then the
 * text's bytes: the text in the platform's own family at the weight and size, broken
 * into lines no wider than the width where it can be; at `at`, how many lines, then
 * a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most */
export const TEXT_LAYOUT = 2147483649 as number;

/** weight, px (f32 bits), color (0xRRGGBBAA), at, width, height, x (f32 bits), then
 * the text's bytes: one line drawn in the platform's own family, starting at x with
 * the middle of its em box on the middle row, into `width` by `height` pixels at
 * `at`, RGBA a byte a channel, top row first, alpha not premultiplied */
export const TEXT_RASTER = 2147483650 as number;
