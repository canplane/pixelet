// @pxl/client/abi/text_answer.spec.ts
//
// GENERATED from spec/client/core/sys/io/text_answer.json
//
// What the platform's text writes into the core's memory as it answers the core's
// calls TEXT_LAYOUT and TEXT_RASTER (spec/client/core/sys/io/text.json, pxl-core sys/io/text.rs,
// ts/client/src/service/core/text.ts), before they return.

import * as LAYOUT_ANSWER from "./text_answer/layout_answer.spec.ts";
import * as LINE from "./text_answer/line.spec.ts";

/** TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece. */
export { LAYOUT_ANSWER };

/** A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits. */
export { LINE };
