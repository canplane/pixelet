// @xpute/link/abi/fs/types.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import type { Lift } from "../../cabi.ts";
import { type ErrorCode, of_ErrorCode } from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 7));
  if (v === null) throw new MarshalError(ERRNO_EBADMSG);
  return v;
}
