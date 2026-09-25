// @xpute/core/status/error.ts

import { Errno, strerror } from "./errno.spec.ts";

/**
 * Error policy:
 * - Internal invariant violation (our bug / impossible state) => FATAL (crash process).
 * - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
 *
 * Notes:
 * - Errno describes the error code, not fatality.
 * - Fatal vs non-fatal is determined by error class (and catch boundary policy).
 * - Origin/context should be attached at catch/log boundary, not encoded in the error.
 *
 * An error is its errno and its class, and nothing else: what a reader is
 * shown is `strerror` of the number, which is the `message` the platform's
 * `Error` asks for.
 */
export class XputeError extends Error {
  readonly errno: Errno;

  constructor(errno: Errno) {
    super(strerror(errno));

    // subclass-safe
    Object.setPrototypeOf(this, new.target.prototype);
    this.name = new.target.name;

    this.errno = errno;

    // V8 nicety (optional, safe). `Function` here matches V8's own real
    // signature (Error.captureStackTrace(target, constructorOpt?: Function))
    // — constructorOpt is never called, only used to find where to truncate
    // the trace, so it accepts a genuinely arbitrary constructor.
    try {
      // deno-lint-ignore ban-types
      (Error as ErrorConstructor & { captureStackTrace?: (target: object, ctor?: Function) => void }).captureStackTrace?.(this, new.target);
    } catch {
      // non-V8 runtime — captureStackTrace isn't there to begin with, ?. already covers it; catch guards a throwing polyfill
    }
  }
}

/** Internal invariant violation / impossible state / our bug (fatal). */
export class InvariantError extends XputeError {}

/** External fault / untrusted input / peer-network-remote failure (normally non-fatal). */
export class FaultError extends XputeError {}

/** Wire/payload decode failure at marshal boundary (not an application-level error, normally non-fatal). */
export class MarshalError extends FaultError {}

/** Network/transport/protocol I/O fault (normally non-fatal). */
export class NetworkFaultError extends FaultError {}
