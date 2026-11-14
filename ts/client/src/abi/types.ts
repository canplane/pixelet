// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type ErrorCode,
  type Sensor,
  of_ErrorCode,
  of_Sensor,
} from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";

export function load_Sensor(r: Lift, at: number): Sensor {
  const v = of_Sensor(r.case(at, 2));
  if (v === null) throw new MarshalError(ERRNO_EBADMSG);
  return v;
}

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 6));
  if (v === null) throw new MarshalError(ERRNO_EBADMSG);
  return v;
}
