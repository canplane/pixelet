// @xpute/kit/status/errno.spec.ts
//
// GENERATED from spec/status/errno.json
//
// POSIX/Linux-aligned negative errno

import type { i32 } from "../abi/word.ts";

export const OK = 0; // Success

// ---- Generic ----

export const EPERM = -1; // Operation not permitted
export const ENOENT = -2; // No such file or directory / entry not found
export const ESRCH = -3; // No such process
export const EINTR = -4; // Interrupted system call
export const EIO = -5; // I/O error
export const ENXIO = -6; // No such device or address
export const E2BIG = -7; // Argument list too long / object too large for fixed local cap
export const ENOEXEC = -8; // Exec format error
export const EBADF = -9; // Bad file descriptor / bad handle
export const ECHILD = -10; // No child processes
export const EAGAIN = -11; // Try again / Resource temporarily unavailable
export const ENOMEM = -12; // Out of memory
export const EACCES = -13; // Permission denied
export const EFAULT = -14; // Bad address
export const EBUSY = -16; // Device or resource busy
export const EEXIST = -17; // File exists / duplicate bind
export const EXDEV = -18; // Cross-device link
export const ENODEV = -19; // No such device
export const ENOTDIR = -20; // Not a directory
export const EISDIR = -21; // Is a directory
export const EINVAL = -22; // Invalid argument
export const ENFILE = -23; // File table overflow (system-wide)
export const EMFILE = -24; // Too many open files (process)
export const ENOTTY = -25; // Not a tty
export const EFBIG = -27; // File too large / payload too large
export const ENOSPC = -28; // No space left on device / local mailbox full
export const ESPIPE = -29; // Illegal seek
export const EROFS = -30; // Read-only file system
export const EMLINK = -31; // Too many links
export const EPIPE = -32; // Broken pipe
export const EDOM = -33; // Math argument out of domain
export const ERANGE = -34; // Math result not representable
export const ENOSYS = -38; // Function not implemented

// ---- Protocol / Message / Encoding ----

export const EPROTO = -71; // Protocol error
export const EBADMSG = -74; // Bad message (protocol/frame/payload invalid)
export const EOVERFLOW = -75; // Value too large for target type / overflow
export const EMSGSIZE = -90; // Message too long / packet too large
export const ENOTSUP = -95; // Operation not supported
export const EILSEQ = -84; // Illegal byte sequence (decode/encoding failure)

// ---- Endpoint / Addressing ----

export const EADDRINUSE = -98; // Address already in use
export const EADDRNOTAVAIL = -99; // Cannot assign requested address

// ---- Network / Transport (Linux asm-generic/errno.h aligned) ----

export const ENETDOWN = -100; // Network is down
export const ENETUNREACH = -101; // Network is unreachable
export const ENETRESET = -102; // Network dropped connection because of reset
export const ECONNABORTED = -103; // Software caused connection abort
export const ECONNRESET = -104; // Connection reset by peer
export const ENOBUFS = -105; // No buffer space available
export const EISCONN = -106; // Transport endpoint is already connected
export const ENOTCONN = -107; // Transport endpoint is not connected
export const ESHUTDOWN = -108; // Cannot send after transport endpoint shutdown
export const ETOOMANYREFS = -109; // Too many references
export const ETIMEDOUT = -110; // Connection timed out
export const ECONNREFUSED = -111; // Connection refused
export const EHOSTDOWN = -112; // Host is down
export const EHOSTUNREACH = -113; // No route to host
export const EALREADY = -114; // Operation already in progress
export const EINPROGRESS = -115; // Operation now in progress

// ---- Lifecycle / Cancellation ----

export const ECANCELED = -125; // Operation canceled
export const ENOTRECOVERABLE = -131; // State not recoverable: an invariant the code stands on does not hold

// Result pair: [errno, data]
//
// Conventions:
// - errno == OK  -> success
// - errno != OK  -> failure
// - data === null      -> no payload
//
// `null` is used as the canonical "no value / no payload" sentinel.
// It does not imply pointer semantics; it is just the empty result slot.
export type ErrnoResult<T> = [i32, T | null];

/** An errno as a reader is shown it: its name and what it means. The
 * strings are for a person to read; an error carries the number alone. */
export function strerror(errnum: i32): string {
  return STRERROR[errnum] ?? `errno ${errnum}`;
}

const STRERROR: Record<number, string> = {
  [0]: "OK: Success",
  [-1]: "EPERM: Operation not permitted",
  [-2]: "ENOENT: No such file or directory / entry not found",
  [-3]: "ESRCH: No such process",
  [-4]: "EINTR: Interrupted system call",
  [-5]: "EIO: I/O error",
  [-6]: "ENXIO: No such device or address",
  [-7]: "E2BIG: Argument list too long / object too large for fixed local cap",
  [-8]: "ENOEXEC: Exec format error",
  [-9]: "EBADF: Bad file descriptor / bad handle",
  [-10]: "ECHILD: No child processes",
  [-11]: "EAGAIN: Try again / Resource temporarily unavailable",
  [-12]: "ENOMEM: Out of memory",
  [-13]: "EACCES: Permission denied",
  [-14]: "EFAULT: Bad address",
  [-16]: "EBUSY: Device or resource busy",
  [-17]: "EEXIST: File exists / duplicate bind",
  [-18]: "EXDEV: Cross-device link",
  [-19]: "ENODEV: No such device",
  [-20]: "ENOTDIR: Not a directory",
  [-21]: "EISDIR: Is a directory",
  [-22]: "EINVAL: Invalid argument",
  [-23]: "ENFILE: File table overflow (system-wide)",
  [-24]: "EMFILE: Too many open files (process)",
  [-25]: "ENOTTY: Not a tty",
  [-27]: "EFBIG: File too large / payload too large",
  [-28]: "ENOSPC: No space left on device / local mailbox full",
  [-29]: "ESPIPE: Illegal seek",
  [-30]: "EROFS: Read-only file system",
  [-31]: "EMLINK: Too many links",
  [-32]: "EPIPE: Broken pipe",
  [-33]: "EDOM: Math argument out of domain",
  [-34]: "ERANGE: Math result not representable",
  [-38]: "ENOSYS: Function not implemented",
  [-71]: "EPROTO: Protocol error",
  [-74]: "EBADMSG: Bad message (protocol/frame/payload invalid)",
  [-75]: "EOVERFLOW: Value too large for target type / overflow",
  [-90]: "EMSGSIZE: Message too long / packet too large",
  [-95]: "ENOTSUP: Operation not supported",
  [-84]: "EILSEQ: Illegal byte sequence (decode/encoding failure)",
  [-98]: "EADDRINUSE: Address already in use",
  [-99]: "EADDRNOTAVAIL: Cannot assign requested address",
  [-100]: "ENETDOWN: Network is down",
  [-101]: "ENETUNREACH: Network is unreachable",
  [-102]: "ENETRESET: Network dropped connection because of reset",
  [-103]: "ECONNABORTED: Software caused connection abort",
  [-104]: "ECONNRESET: Connection reset by peer",
  [-105]: "ENOBUFS: No buffer space available",
  [-106]: "EISCONN: Transport endpoint is already connected",
  [-107]: "ENOTCONN: Transport endpoint is not connected",
  [-108]: "ESHUTDOWN: Cannot send after transport endpoint shutdown",
  [-109]: "ETOOMANYREFS: Too many references",
  [-110]: "ETIMEDOUT: Connection timed out",
  [-111]: "ECONNREFUSED: Connection refused",
  [-112]: "EHOSTDOWN: Host is down",
  [-113]: "EHOSTUNREACH: No route to host",
  [-114]: "EALREADY: Operation already in progress",
  [-115]: "EINPROGRESS: Operation now in progress",
  [-125]: "ECANCELED: Operation canceled",
  [-131]:
    "ENOTRECOVERABLE: State not recoverable: an invariant the code stands on does not hold",
};

/** The member `code` is, null for a number no member has. */
export function errno_of(code: number): i32 | null {
  return Object.hasOwn(STRERROR, code) ? code : null;
}
