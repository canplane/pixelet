// @xpute/kit/status/error.ts

import { Errno, strerror } from "./errno.spec.ts";

/**
 * An InvariantError is fatal; a FaultError is caught, isolated and dropped. The
 * class decides which, not the errno.
 */
export class XputeError extends Error {
  readonly errno: Errno;

  constructor(errno: Errno) {
    super(strerror(errno));

    Object.setPrototypeOf(this, new.target.prototype);
    this.name = new.target.name;

    this.errno = errno;

    try {
      // deno-lint-ignore ban-types
      (Error as ErrorConstructor & { captureStackTrace?: (target: object, ctor?: Function) => void }).captureStackTrace?.(this, new.target);
    } catch {
      // a polyfill that throws
    }
  }
}
/** Fatal. */
/** Internal invariant violation / impossible state / our bug (fatal). */
export class InvariantError extends XputeError {}
/** An external fault or untrusted input; normally non-fatal. */
/** External fault / untrusted input / peer-network-remote failure (normally non-fatal). */
export class FaultError extends XputeError {}
/** Wire/payload decode failure at marshal boundary (not an application-level error, normally non-fatal). */
export class MarshalError extends FaultError {}
/** Network/transport/protocol I/O fault (normally non-fatal). */
export class NetworkFaultError extends FaultError {}
