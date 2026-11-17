// @pxl/client/service/core/text.ts

/**
 * The text the host implements for the core (spec/client/core/wit/text.wit),
 * from the platform's text: a width, and a raster. Its methods answer at
 * once: each reads the text the core lends, and what a raster writes into the
 * image the core lends is in place by the time the call returns. The core
 * breaks the lines itself. The web worker mounts it (runtime/worker.ts)
 * and the native text is pxl-runtime's text.rs; it sits in service/core
 * because it implements one of the core's interfaces, as the others here call
 * them.
 */

import { td } from "@xpute/kit/codec/encoding.ts";
import { ERRNO_EINVAL } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Callee, Memory } from "@xpute/link";
import type { Lent } from "@xpute/link/abi/types.ts";

import * as text from "@/abi/text.ts";
import * as platform from "@/service/platform/text.web.ts";

/** Mounts the text on `link`, reading what the core lends in `mem`. */
export function mount_text(link: Callee, mem: Memory): void {
  /** The text a call lends, which the core holds only until the call
   * returns. */
  const text_of = (span: Lent): string =>
    td.decode(mem.bytes(span.at, span.len));

  text.mount(link, {
    width(weight, px, text) {
      return platform.width(weight, px, text_of(text));
    },
    raster(weight, px, color, image, width, height, x, text) {
      const len = width * height * 4;
      if (len > image.len) throw new XputeError(ERRNO_EINVAL);
      platform.raster(
        weight,
        px,
        color,
        text_of(text),
        x,
        width,
        height,
        mem.bytes(image.at, len),
      );
    },
  });
}
