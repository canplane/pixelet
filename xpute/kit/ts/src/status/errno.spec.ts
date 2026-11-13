// @xpute/kit/status/errno.spec.ts
//
// GENERATED from kit/spec/errno.json (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

export const ERRNO = {
  OK: {
    text: "Success",
  },
  EPERM: {
    text: "Operation not permitted",
  },
  ENOENT: {
    text: "No such file or directory / entry not found",
  },
  ESRCH: {
    text: "No such process",
  },
  EINTR: {
    text: "Interrupted system call",
  },
  EIO: {
    text: "I/O error",
  },
  ENXIO: {
    text: "No such device or address",
  },
  E2BIG: {
    text: "Argument list too long / object too large for fixed local cap",
  },
  ENOEXEC: {
    text: "Exec format error",
  },
  EBADF: {
    text: "Bad file descriptor / bad handle",
  },
  ECHILD: {
    text: "No child processes",
  },
  EAGAIN: {
    text: "Try again / Resource temporarily unavailable",
  },
  ENOMEM: {
    text: "Out of memory",
  },
  EACCES: {
    text: "Permission denied",
  },
  EFAULT: {
    text: "Bad address",
  },
  EBUSY: {
    text: "Device or resource busy",
  },
  EEXIST: {
    text: "File exists / duplicate bind",
  },
  EXDEV: {
    text: "Cross-device link",
  },
  ENODEV: {
    text: "No such device",
  },
  ENOTDIR: {
    text: "Not a directory",
  },
  EISDIR: {
    text: "Is a directory",
  },
  EINVAL: {
    text: "Invalid argument",
  },
  ENFILE: {
    text: "File table overflow (system-wide)",
  },
  EMFILE: {
    text: "Too many open files (process)",
  },
  ENOTTY: {
    text: "Not a tty",
  },
  EFBIG: {
    text: "File too large / payload too large",
  },
  ENOSPC: {
    text: "No space left on device / local mailbox full",
  },
  ESPIPE: {
    text: "Illegal seek",
  },
  EROFS: {
    text: "Read-only file system",
  },
  EMLINK: {
    text: "Too many links",
  },
  EPIPE: {
    text: "Broken pipe",
  },
  EDOM: {
    text: "Math argument out of domain",
  },
  ERANGE: {
    text: "Math result not representable",
  },
  ENOSYS: {
    text: "Function not implemented",
  },
  EPROTO: {
    text: "Protocol error",
  },
  EBADMSG: {
    text: "Bad message (protocol/frame/payload invalid)",
  },
  EOVERFLOW: {
    text: "Value too large for target type / overflow",
  },
  EMSGSIZE: {
    text: "Message too long / packet too large",
  },
  ENOTSUP: {
    text: "Operation not supported",
  },
  EILSEQ: {
    text: "Illegal byte sequence (decode/encoding failure)",
  },
  EADDRINUSE: {
    text: "Address already in use",
  },
  EADDRNOTAVAIL: {
    text: "Cannot assign requested address",
  },
  ENETDOWN: {
    text: "Network is down",
  },
  ENETUNREACH: {
    text: "Network is unreachable",
  },
  ENETRESET: {
    text: "Network dropped connection because of reset",
  },
  ECONNABORTED: {
    text: "Software caused connection abort",
  },
  ECONNRESET: {
    text: "Connection reset by peer",
  },
  ENOBUFS: {
    text: "No buffer space available",
  },
  EISCONN: {
    text: "Transport endpoint is already connected",
  },
  ENOTCONN: {
    text: "Transport endpoint is not connected",
  },
  ESHUTDOWN: {
    text: "Cannot send after transport endpoint shutdown",
  },
  ETOOMANYREFS: {
    text: "Too many references",
  },
  ETIMEDOUT: {
    text: "Connection timed out",
  },
  ECONNREFUSED: {
    text: "Connection refused",
  },
  EHOSTDOWN: {
    text: "Runtime is down",
  },
  EHOSTUNREACH: {
    text: "No route to runtime",
  },
  EALREADY: {
    text: "Operation already in progress",
  },
  EINPROGRESS: {
    text: "Operation now in progress",
  },
  ECANCELED: {
    text: "Operation canceled",
  },
  ENOTRECOVERABLE: {
    text: "State not recoverable: an invariant the code stands on does not hold",
  },
} as const;

export const Errno = {
  OK: 0,
  EPERM: -1,
  ENOENT: -2,
  ESRCH: -3,
  EINTR: -4,
  EIO: -5,
  ENXIO: -6,
  E2BIG: -7,
  ENOEXEC: -8,
  EBADF: -9,
  ECHILD: -10,
  EAGAIN: -11,
  ENOMEM: -12,
  EACCES: -13,
  EFAULT: -14,
  EBUSY: -16,
  EEXIST: -17,
  EXDEV: -18,
  ENODEV: -19,
  ENOTDIR: -20,
  EISDIR: -21,
  EINVAL: -22,
  ENFILE: -23,
  EMFILE: -24,
  ENOTTY: -25,
  EFBIG: -27,
  ENOSPC: -28,
  ESPIPE: -29,
  EROFS: -30,
  EMLINK: -31,
  EPIPE: -32,
  EDOM: -33,
  ERANGE: -34,
  ENOSYS: -38,
  EPROTO: -71,
  EBADMSG: -74,
  EOVERFLOW: -75,
  EMSGSIZE: -90,
  ENOTSUP: -95,
  EILSEQ: -84,
  EADDRINUSE: -98,
  EADDRNOTAVAIL: -99,
  ENETDOWN: -100,
  ENETUNREACH: -101,
  ENETRESET: -102,
  ECONNABORTED: -103,
  ECONNRESET: -104,
  ENOBUFS: -105,
  EISCONN: -106,
  ENOTCONN: -107,
  ESHUTDOWN: -108,
  ETOOMANYREFS: -109,
  ETIMEDOUT: -110,
  ECONNREFUSED: -111,
  EHOSTDOWN: -112,
  EHOSTUNREACH: -113,
  EALREADY: -114,
  EINPROGRESS: -115,
  ECANCELED: -125,
  ENOTRECOVERABLE: -131,
} as const;
export type Errno = (typeof Errno)[keyof typeof Errno];

/** The name of the `Errno` member numbered `n`, or undefined for a number
 * no member has. */
export function errno_name(n: number): keyof typeof Errno | undefined {
  return (Object.keys(Errno) as (keyof typeof Errno)[]).find(
    (k) => Errno[k] === n,
  );
}

/** The `Errno` member `n` is, or null for a number no member has. */
export function errno_of(n: number): Errno | null {
  return errno_name(n) === undefined ? null : (n as Errno);
}
