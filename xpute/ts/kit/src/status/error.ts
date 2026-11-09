// @xpute/kit/status/error.ts

import { strerror } from "./errno.spec.ts";
import type { i32 } from "../abi/word.ts";

/**
 * Error policy:
 * - Internal invariant violation (our bug / impossible state) => FATAL (the core is ended; never caught to carry on).
 * - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
 *
 * Notes:
 * - Errno describes the error code, not fatality.
 * - Fatal vs non-fatal is determined by error class (and catch boundary policy).
 * - Origin/context should be attached at catch/log boundary, not encoded in message prefixes.
 */
export class XputeError extends Error {
  readonly errnum: i32;

  constructor(errnum: i32) {
    super(strerror(errnum));

    // subclass-safe
    Object.setPrototypeOf(this, new.target.prototype);
    this.name = new.target.name;

    this.errnum = errnum;

    try {
      (
        Error as ErrorConstructor & {
          captureStackTrace?: (target: object, ctor?: Function) => void;
        }
      ).captureStackTrace?.(this, new.target);
    } catch {
      // A polyfilled `captureStackTrace` may throw; the stack is then left
      // as it is.
    }
  }
}
/** A broken invariant is fatal. */
export class InvariantError extends XputeError {}
/** A fault comes from outside the program, such as untrusted input, and is not
 * fatal. */
export class FaultError extends XputeError {}
/** A payload does not decode where it crosses the wire. This is a fault of the
 * input, not of the program's logic, and is not fatal. */
export class MarshalError extends FaultError {}
/** A fault of the network, its transport or its protocol is not fatal. */
export class NetworkFaultError extends FaultError {}
