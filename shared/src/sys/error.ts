// @pixelet/shared/sys/error.ts

import { Errno } from "./errno.ts";

export type XputeErrorOptions = {
  cause?: unknown;
};

/**
 * Error policy:
 * - Internal invariant violation (our bug / impossible state) => FATAL (crash process).
 * - External fault (peer/network/remote/env/input) => NON-FATAL (catch, isolate, degrade, drop).
 *
 * Notes:
 * - Errno describes the error code, not fatality.
 * - Fatal vs non-fatal is determined by error class (and catch boundary policy).
 * - Origin/context should be attached at catch/log boundary, not encoded in message prefixes.
 */
export class XputeError extends Error {
  readonly errno: Errno;

  constructor(errno: Errno, message?: string, opts?: XputeErrorOptions) {
    const msg = message || `errno: ${errno}`;
    super(msg);

    // subclass-safe
    Object.setPrototypeOf(this, new.target.prototype);
    this.name = new.target.name;

    this.errno = errno;

    // portable-ish cause support (works even on runtimes without native ErrorOptions)
    if (opts && Object.prototype.hasOwnProperty.call(opts, "cause")) {
      try {
        Object.defineProperty(this, "cause", {
          value: opts.cause,
          writable: false,
          enumerable: false,
          configurable: true,
        });
      } catch {
        try {
          (this as Error & { cause?: unknown }).cause = opts.cause;
        } catch {}
      }
    }

    // V8 nicety (optional, safe)
    try {
      (Error as ErrorConstructor & { captureStackTrace?: (target: object, ctor?: Function) => void }).captureStackTrace?.(this, new.target);
    } catch {}
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
