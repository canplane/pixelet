// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type ErrorCode,
  type LocationMode,
  of_ErrorCode,
  of_LocationMode,
} from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 6));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_LocationMode(r: Lift, at: number): LocationMode {
  const v = of_LocationMode(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}
