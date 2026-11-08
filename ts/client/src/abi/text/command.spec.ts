// ts/client/src/abi/text/command.spec.ts
//
// GENERATED from spec/client/core/sys/io/text.json

// Every command, by the side that sends it.

// ---- text, import ----

/** The text in the platform's own family at the weight and size, broken into lines no
 * wider than the width where it can be; at `at` in the core's memory, how many lines,
 * then a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
 * (sys/io/text_answer.json LAYOUT_ANSWER, LINE). A call: answered before it returns. */
export const TEXT_LAYOUT = 0x1c01;
/** One line drawn in the platform's own family, starting at x with the middle of its em
 * box on the middle row, into `width` by `height` pixels at `at` in the core's memory,
 * RGBA a byte a channel, top row first, alpha not premultiplied. A call: answered before
 * it returns. */
export const TEXT_RASTER = 0x1c02;
