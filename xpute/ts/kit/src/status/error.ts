// @xpute/kit/status/error.ts

import { strerror } from "./errno.spec.ts";
import type { i32 } from "../abi/word.ts";

/**
 * An errno raised as an exception. Its class says whether it is fatal, not its
 * errno: an `InvariantError` is fatal, and a `FaultError` is caught, isolated
 * and dropped.
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
/** A broken invariant, which is fatal. */
export class InvariantError extends XputeError {}
/**
 * A fault from outside the program, such as untrusted input; it is not
 * fatal.
 */
export class FaultError extends XputeError {}
/**
 * A payload that does not decode where it crosses the wire. It is a fault of
 * the input, not of the program's logic, and is not fatal.
 */
export class MarshalError extends FaultError {}
/** A fault of the network, its transport or its protocol; it is not fatal. */
export class NetworkFaultError extends FaultError {}
