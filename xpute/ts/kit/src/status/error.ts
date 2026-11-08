// @xpute/kit/status/error.ts

import { strerror } from "./errno.spec.ts";
import type { i32 } from "../abi/word.ts";

/**
 * An `XputeError` is an errno raised as an exception. Its class says whether it
 * is fatal, not its errno: an `InvariantError` is fatal, and a `FaultError` is
 * caught, isolated and dropped.
 */
export class XputeError extends Error {
  readonly errnum: i32;

  constructor(errnum: i32) {
    super(strerror(errnum));

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
