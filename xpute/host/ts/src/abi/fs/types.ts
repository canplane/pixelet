// @xpute/host/abi/fs/types.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import { type ErrorCode, of_ErrorCode } from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 7));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}
