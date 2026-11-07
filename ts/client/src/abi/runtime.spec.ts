// @pxl/client/abi/runtime.spec.ts
//
// GENERATED from spec/client/core/sys/io/runtime.json
//
// What the platform's text writes into the core's memory as it answers the core's
// calls TEXT_LAYOUT and TEXT_RASTER (rust/client/core/text.json, pxl-core sys/io/text.rs,
// ts/client/src/port/text.ts), before they return.

import * as LAYOUT_ANSWER from "./runtime/layout_answer.spec.ts";
import * as LINE from "./runtime/line.spec.ts";

/** TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece. */
export { LAYOUT_ANSWER };

/** A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits. */
export { LINE };
