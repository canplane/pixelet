// @xpute/kit/status/errno.spec.ts
//
// GENERATED from kit/spec/errno.json (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

export const ERRNO = {
  OK: {
    number: 0 as number,
    text: "Success",
  },
  EPERM: {
    number: -1 as number,
    text: "Operation not permitted",
  },
  ENOENT: {
    number: -2 as number,
    text: "No such file or directory / entry not found",
  },
  ESRCH: {
    number: -3 as number,
    text: "No such process",
  },
  EINTR: {
    number: -4 as number,
    text: "Interrupted system call",
  },
  EIO: {
    number: -5 as number,
    text: "I/O error",
  },
  ENXIO: {
    number: -6 as number,
    text: "No such device or address",
  },
  E2BIG: {
    number: -7 as number,
    text: "Argument list too long / object too large for fixed local cap",
  },
  ENOEXEC: {
    number: -8 as number,
    text: "Exec format error",
  },
  EBADF: {
    number: -9 as number,
    text: "Bad file descriptor / bad handle",
  },
  ECHILD: {
    number: -10 as number,
    text: "No child processes",
  },
  EAGAIN: {
    number: -11 as number,
    text: "Try again / Resource temporarily unavailable",
  },
  ENOMEM: {
    number: -12 as number,
    text: "Out of memory",
  },
  EACCES: {
    number: -13 as number,
    text: "Permission denied",
  },
  EFAULT: {
    number: -14 as number,
    text: "Bad address",
  },
  EBUSY: {
    number: -16 as number,
    text: "Device or resource busy",
  },
  EEXIST: {
    number: -17 as number,
    text: "File exists / duplicate bind",
  },
  EXDEV: {
    number: -18 as number,
    text: "Cross-device link",
  },
  ENODEV: {
    number: -19 as number,
    text: "No such device",
  },
  ENOTDIR: {
    number: -20 as number,
    text: "Not a directory",
  },
  EISDIR: {
    number: -21 as number,
    text: "Is a directory",
  },
  EINVAL: {
    number: -22 as number,
    text: "Invalid argument",
  },
  ENFILE: {
    number: -23 as number,
    text: "File table overflow (system-wide)",
  },
  EMFILE: {
    number: -24 as number,
    text: "Too many open files (process)",
  },
  ENOTTY: {
    number: -25 as number,
    text: "Not a tty",
  },
  EFBIG: {
    number: -27 as number,
    text: "File too large / payload too large",
  },
  ENOSPC: {
    number: -28 as number,
    text: "No space left on device / local mailbox full",
  },
  ESPIPE: {
    number: -29 as number,
    text: "Illegal seek",
  },
  EROFS: {
    number: -30 as number,
    text: "Read-only file system",
  },
  EMLINK: {
    number: -31 as number,
    text: "Too many links",
  },
  EPIPE: {
    number: -32 as number,
    text: "Broken pipe",
  },
  EDOM: {
    number: -33 as number,
    text: "Math argument out of domain",
  },
  ERANGE: {
    number: -34 as number,
    text: "Math result not representable",
  },
  ENOSYS: {
    number: -38 as number,
    text: "Function not implemented",
  },
  EPROTO: {
    number: -71 as number,
    text: "Protocol error",
  },
  EBADMSG: {
    number: -74 as number,
    text: "Bad message (protocol/frame/payload invalid)",
  },
  EOVERFLOW: {
    number: -75 as number,
    text: "Value too large for target type / overflow",
  },
  EMSGSIZE: {
    number: -90 as number,
    text: "Message too long / packet too large",
  },
  ENOTSUP: {
    number: -95 as number,
    text: "Operation not supported",
  },
  EILSEQ: {
    number: -84 as number,
    text: "Illegal byte sequence (decode/encoding failure)",
  },
  EADDRINUSE: {
    number: -98 as number,
    text: "Address already in use",
  },
  EADDRNOTAVAIL: {
    number: -99 as number,
    text: "Cannot assign requested address",
  },
  ENETDOWN: {
    number: -100 as number,
    text: "Network is down",
  },
  ENETUNREACH: {
    number: -101 as number,
    text: "Network is unreachable",
  },
  ENETRESET: {
    number: -102 as number,
    text: "Network dropped connection because of reset",
  },
  ECONNABORTED: {
    number: -103 as number,
    text: "Software caused connection abort",
  },
  ECONNRESET: {
    number: -104 as number,
    text: "Connection reset by peer",
  },
  ENOBUFS: {
    number: -105 as number,
    text: "No buffer space available",
  },
  EISCONN: {
    number: -106 as number,
    text: "Transport endpoint is already connected",
  },
  ENOTCONN: {
    number: -107 as number,
    text: "Transport endpoint is not connected",
  },
  ESHUTDOWN: {
    number: -108 as number,
    text: "Cannot send after transport endpoint shutdown",
  },
  ETOOMANYREFS: {
    number: -109 as number,
    text: "Too many references",
  },
  ETIMEDOUT: {
    number: -110 as number,
    text: "Connection timed out",
  },
  ECONNREFUSED: {
    number: -111 as number,
    text: "Connection refused",
  },
  EHOSTDOWN: {
    number: -112 as number,
    text: "Runtime is down",
  },
  EHOSTUNREACH: {
    number: -113 as number,
    text: "No route to runtime",
  },
  EALREADY: {
    number: -114 as number,
    text: "Operation already in progress",
  },
  EINPROGRESS: {
    number: -115 as number,
    text: "Operation now in progress",
  },
  ECANCELED: {
    number: -125 as number,
    text: "Operation canceled",
  },
  ENOTRECOVERABLE: {
    number: -131 as number,
    text: "State not recoverable: an invariant the code stands on does not hold",
  },
} as const;

export type Errno =
  | typeof ERRNO_OK
  | typeof ERRNO_EPERM
  | typeof ERRNO_ENOENT
  | typeof ERRNO_ESRCH
  | typeof ERRNO_EINTR
  | typeof ERRNO_EIO
  | typeof ERRNO_ENXIO
  | typeof ERRNO_E2BIG
  | typeof ERRNO_ENOEXEC
  | typeof ERRNO_EBADF
  | typeof ERRNO_ECHILD
  | typeof ERRNO_EAGAIN
  | typeof ERRNO_ENOMEM
  | typeof ERRNO_EACCES
  | typeof ERRNO_EFAULT
  | typeof ERRNO_EBUSY
  | typeof ERRNO_EEXIST
  | typeof ERRNO_EXDEV
  | typeof ERRNO_ENODEV
  | typeof ERRNO_ENOTDIR
  | typeof ERRNO_EISDIR
  | typeof ERRNO_EINVAL
  | typeof ERRNO_ENFILE
  | typeof ERRNO_EMFILE
  | typeof ERRNO_ENOTTY
  | typeof ERRNO_EFBIG
  | typeof ERRNO_ENOSPC
  | typeof ERRNO_ESPIPE
  | typeof ERRNO_EROFS
  | typeof ERRNO_EMLINK
  | typeof ERRNO_EPIPE
  | typeof ERRNO_EDOM
  | typeof ERRNO_ERANGE
  | typeof ERRNO_ENOSYS
  | typeof ERRNO_EPROTO
  | typeof ERRNO_EBADMSG
  | typeof ERRNO_EOVERFLOW
  | typeof ERRNO_EMSGSIZE
  | typeof ERRNO_ENOTSUP
  | typeof ERRNO_EILSEQ
  | typeof ERRNO_EADDRINUSE
  | typeof ERRNO_EADDRNOTAVAIL
  | typeof ERRNO_ENETDOWN
  | typeof ERRNO_ENETUNREACH
  | typeof ERRNO_ENETRESET
  | typeof ERRNO_ECONNABORTED
  | typeof ERRNO_ECONNRESET
  | typeof ERRNO_ENOBUFS
  | typeof ERRNO_EISCONN
  | typeof ERRNO_ENOTCONN
  | typeof ERRNO_ESHUTDOWN
  | typeof ERRNO_ETOOMANYREFS
  | typeof ERRNO_ETIMEDOUT
  | typeof ERRNO_ECONNREFUSED
  | typeof ERRNO_EHOSTDOWN
  | typeof ERRNO_EHOSTUNREACH
  | typeof ERRNO_EALREADY
  | typeof ERRNO_EINPROGRESS
  | typeof ERRNO_ECANCELED
  | typeof ERRNO_ENOTRECOVERABLE;
export const ERRNO_OK = 0;
export const ERRNO_EPERM = -1;
export const ERRNO_ENOENT = -2;
export const ERRNO_ESRCH = -3;
export const ERRNO_EINTR = -4;
export const ERRNO_EIO = -5;
export const ERRNO_ENXIO = -6;
export const ERRNO_E2BIG = -7;
export const ERRNO_ENOEXEC = -8;
export const ERRNO_EBADF = -9;
export const ERRNO_ECHILD = -10;
export const ERRNO_EAGAIN = -11;
export const ERRNO_ENOMEM = -12;
export const ERRNO_EACCES = -13;
export const ERRNO_EFAULT = -14;
export const ERRNO_EBUSY = -16;
export const ERRNO_EEXIST = -17;
export const ERRNO_EXDEV = -18;
export const ERRNO_ENODEV = -19;
export const ERRNO_ENOTDIR = -20;
export const ERRNO_EISDIR = -21;
export const ERRNO_EINVAL = -22;
export const ERRNO_ENFILE = -23;
export const ERRNO_EMFILE = -24;
export const ERRNO_ENOTTY = -25;
export const ERRNO_EFBIG = -27;
export const ERRNO_ENOSPC = -28;
export const ERRNO_ESPIPE = -29;
export const ERRNO_EROFS = -30;
export const ERRNO_EMLINK = -31;
export const ERRNO_EPIPE = -32;
export const ERRNO_EDOM = -33;
export const ERRNO_ERANGE = -34;
export const ERRNO_ENOSYS = -38;
export const ERRNO_EPROTO = -71;
export const ERRNO_EBADMSG = -74;
export const ERRNO_EOVERFLOW = -75;
export const ERRNO_EMSGSIZE = -90;
export const ERRNO_ENOTSUP = -95;
export const ERRNO_EILSEQ = -84;
export const ERRNO_EADDRINUSE = -98;
export const ERRNO_EADDRNOTAVAIL = -99;
export const ERRNO_ENETDOWN = -100;
export const ERRNO_ENETUNREACH = -101;
export const ERRNO_ENETRESET = -102;
export const ERRNO_ECONNABORTED = -103;
export const ERRNO_ECONNRESET = -104;
export const ERRNO_ENOBUFS = -105;
export const ERRNO_EISCONN = -106;
export const ERRNO_ENOTCONN = -107;
export const ERRNO_ESHUTDOWN = -108;
export const ERRNO_ETOOMANYREFS = -109;
export const ERRNO_ETIMEDOUT = -110;
export const ERRNO_ECONNREFUSED = -111;
export const ERRNO_EHOSTDOWN = -112;
export const ERRNO_EHOSTUNREACH = -113;
export const ERRNO_EALREADY = -114;
export const ERRNO_EINPROGRESS = -115;
export const ERRNO_ECANCELED = -125;
export const ERRNO_ENOTRECOVERABLE = -131;

/** The member `val` is, or null for a number no member has. */
export function of_Errno(val: number): Errno | null {
  return val === ERRNO_OK ||
    val === ERRNO_EPERM ||
    val === ERRNO_ENOENT ||
    val === ERRNO_ESRCH ||
    val === ERRNO_EINTR ||
    val === ERRNO_EIO ||
    val === ERRNO_ENXIO ||
    val === ERRNO_E2BIG ||
    val === ERRNO_ENOEXEC ||
    val === ERRNO_EBADF ||
    val === ERRNO_ECHILD ||
    val === ERRNO_EAGAIN ||
    val === ERRNO_ENOMEM ||
    val === ERRNO_EACCES ||
    val === ERRNO_EFAULT ||
    val === ERRNO_EBUSY ||
    val === ERRNO_EEXIST ||
    val === ERRNO_EXDEV ||
    val === ERRNO_ENODEV ||
    val === ERRNO_ENOTDIR ||
    val === ERRNO_EISDIR ||
    val === ERRNO_EINVAL ||
    val === ERRNO_ENFILE ||
    val === ERRNO_EMFILE ||
    val === ERRNO_ENOTTY ||
    val === ERRNO_EFBIG ||
    val === ERRNO_ENOSPC ||
    val === ERRNO_ESPIPE ||
    val === ERRNO_EROFS ||
    val === ERRNO_EMLINK ||
    val === ERRNO_EPIPE ||
    val === ERRNO_EDOM ||
    val === ERRNO_ERANGE ||
    val === ERRNO_ENOSYS ||
    val === ERRNO_EPROTO ||
    val === ERRNO_EBADMSG ||
    val === ERRNO_EOVERFLOW ||
    val === ERRNO_EMSGSIZE ||
    val === ERRNO_ENOTSUP ||
    val === ERRNO_EILSEQ ||
    val === ERRNO_EADDRINUSE ||
    val === ERRNO_EADDRNOTAVAIL ||
    val === ERRNO_ENETDOWN ||
    val === ERRNO_ENETUNREACH ||
    val === ERRNO_ENETRESET ||
    val === ERRNO_ECONNABORTED ||
    val === ERRNO_ECONNRESET ||
    val === ERRNO_ENOBUFS ||
    val === ERRNO_EISCONN ||
    val === ERRNO_ENOTCONN ||
    val === ERRNO_ESHUTDOWN ||
    val === ERRNO_ETOOMANYREFS ||
    val === ERRNO_ETIMEDOUT ||
    val === ERRNO_ECONNREFUSED ||
    val === ERRNO_EHOSTDOWN ||
    val === ERRNO_EHOSTUNREACH ||
    val === ERRNO_EALREADY ||
    val === ERRNO_EINPROGRESS ||
    val === ERRNO_ECANCELED ||
    val === ERRNO_ENOTRECOVERABLE
    ? val
    : null;
}
