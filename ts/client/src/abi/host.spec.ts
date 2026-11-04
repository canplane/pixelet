// @pxl/client/abi/host.spec.ts
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// The kernel's own hostcalls, numbered beside xpute's (xpute/spec/ipc/sys.json,
// pxl-kernel sys/io/host.rs, ts/client/src/sys/text.ts) and made as hostcalls, answered
// before they return; and the range of the stream xpute's calls are recorded in, which
// a hostcall's record lies past while it runs.

import * as stream from "./host/stream.spec.ts";
import * as op from "./host/op.spec.ts";
import * as LAYOUT from "./host/layout.spec.ts";
import * as LAYOUT_ANSWER from "./host/layout_answer.spec.ts";
import * as LINE from "./host/line.spec.ts";
import * as RASTER from "./host/raster.spec.ts";

/** The stream's range, laid out by the kernel's map. */
export { stream };

/** The kernel's calls, each with xpute's PROGRAM bit and made as a hostcall; a record's call, then its arguments. */
export { op };

/** TEXT_LAYOUT's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight, px and max width as f32 bits, where its answer goes and
 * how many lines it may hold. */
export { LAYOUT };

/** TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece. */
export { LAYOUT_ANSWER };

/** A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits. */
export { LINE };

/** TEXT_RASTER's arguments, a word each after its call, then the text's bytes at
 * `WORDS`: the weight and px as f32 bits, the color, where its pixels go, their
 * width and height, and x as f32 bits. */
export { RASTER };
