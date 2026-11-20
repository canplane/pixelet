// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type ErrorCode,
  type TrackingMode,
  of_ErrorCode,
  of_TrackingMode,
} from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 6));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_TrackingMode(r: Lift, at: number): TrackingMode {
  const v = of_TrackingMode(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}
