// @pxl/web/xpute/context.ts

import type { XputeContext } from "@xpute/spec/ipc/link.ts";

// module-scope singleton
let _ctx: XputeContext | undefined;

export function set_xpute(ctx: XputeContext): void {
  if (_ctx && _ctx !== ctx) {
    if (!__IS_DEV__) throw new Error("xpute context already set");
    console.warn("xpute context already set");
  }
  _ctx = ctx;
}

export function get_xpute(): XputeContext {
  if (!_ctx) throw new Error("xpute context not set");
  return _ctx;
}
