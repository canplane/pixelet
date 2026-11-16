// @xpute/kit/status/errno.spec.ts
//
// GENERATED from kit/spec/errno.json5 (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

/** What an errno says happened: the canonical status code of gRPC and Abseil
 * (absl::StatusCode), by its number there, each errno's as Abseil's
 * ErrnoToStatusCode maps it, save that ENOTRECOVERABLE is internal, and
 * EBADMSG and EILSEQ data_loss. It says nothing of whether the error is
 * fatal: a broken invariant is a bug, which stops the program. */
export type Kind =
  | typeof KIND_OK
  | typeof KIND_CANCELLED
  | typeof KIND_UNKNOWN
  | typeof KIND_INVALID_ARGUMENT
  | typeof KIND_DEADLINE_EXCEEDED
  | typeof KIND_NOT_FOUND
  | typeof KIND_ALREADY_EXISTS
  | typeof KIND_PERMISSION_DENIED
  | typeof KIND_RESOURCE_EXHAUSTED
  | typeof KIND_FAILED_PRECONDITION
  | typeof KIND_OUT_OF_RANGE
  | typeof KIND_UNIMPLEMENTED
  | typeof KIND_INTERNAL
  | typeof KIND_UNAVAILABLE
  | typeof KIND_DATA_LOSS;
export const KIND_OK = 0;
export const KIND_CANCELLED = 1;
export const KIND_UNKNOWN = 2;
export const KIND_INVALID_ARGUMENT = 3;
export const KIND_DEADLINE_EXCEEDED = 4;
export const KIND_NOT_FOUND = 5;
export const KIND_ALREADY_EXISTS = 6;
export const KIND_PERMISSION_DENIED = 7;
export const KIND_RESOURCE_EXHAUSTED = 8;
export const KIND_FAILED_PRECONDITION = 9;
export const KIND_OUT_OF_RANGE = 11;
export const KIND_UNIMPLEMENTED = 12;
export const KIND_INTERNAL = 13;
export const KIND_UNAVAILABLE = 14;
export const KIND_DATA_LOSS = 15;

/** The member `val` is, or null for a number no member has. */
export function of_Kind(val: number): Kind | null {
  return val === KIND_OK ||
    val === KIND_CANCELLED ||
    val === KIND_UNKNOWN ||
    val === KIND_INVALID_ARGUMENT ||
    val === KIND_DEADLINE_EXCEEDED ||
    val === KIND_NOT_FOUND ||
    val === KIND_ALREADY_EXISTS ||
    val === KIND_PERMISSION_DENIED ||
    val === KIND_RESOURCE_EXHAUSTED ||
    val === KIND_FAILED_PRECONDITION ||
    val === KIND_OUT_OF_RANGE ||
    val === KIND_UNIMPLEMENTED ||
    val === KIND_INTERNAL ||
    val === KIND_UNAVAILABLE ||
    val === KIND_DATA_LOSS
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Kind(val: Kind): string {
  switch (val) {
    case KIND_OK:
      return "ok";
    case KIND_CANCELLED:
      return "cancelled";
    case KIND_UNKNOWN:
      return "unknown";
    case KIND_INVALID_ARGUMENT:
      return "invalid_argument";
    case KIND_DEADLINE_EXCEEDED:
      return "deadline_exceeded";
    case KIND_NOT_FOUND:
      return "not_found";
    case KIND_ALREADY_EXISTS:
      return "already_exists";
    case KIND_PERMISSION_DENIED:
      return "permission_denied";
    case KIND_RESOURCE_EXHAUSTED:
      return "resource_exhausted";
    case KIND_FAILED_PRECONDITION:
      return "failed_precondition";
    case KIND_OUT_OF_RANGE:
      return "out_of_range";
    case KIND_UNIMPLEMENTED:
      return "unimplemented";
    case KIND_INTERNAL:
      return "internal";
    case KIND_UNAVAILABLE:
      return "unavailable";
    case KIND_DATA_LOSS:
      return "data_loss";
  }
}

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

/** The member's name, as its source spells it. */
export function name_Errno(val: Errno): string {
  switch (val) {
    case ERRNO_OK:
      return "OK";
    case ERRNO_EPERM:
      return "EPERM";
    case ERRNO_ENOENT:
      return "ENOENT";
    case ERRNO_ESRCH:
      return "ESRCH";
    case ERRNO_EINTR:
      return "EINTR";
    case ERRNO_EIO:
      return "EIO";
    case ERRNO_ENXIO:
      return "ENXIO";
    case ERRNO_E2BIG:
      return "E2BIG";
    case ERRNO_ENOEXEC:
      return "ENOEXEC";
    case ERRNO_EBADF:
      return "EBADF";
    case ERRNO_ECHILD:
      return "ECHILD";
    case ERRNO_EAGAIN:
      return "EAGAIN";
    case ERRNO_ENOMEM:
      return "ENOMEM";
    case ERRNO_EACCES:
      return "EACCES";
    case ERRNO_EFAULT:
      return "EFAULT";
    case ERRNO_EBUSY:
      return "EBUSY";
    case ERRNO_EEXIST:
      return "EEXIST";
    case ERRNO_EXDEV:
      return "EXDEV";
    case ERRNO_ENODEV:
      return "ENODEV";
    case ERRNO_ENOTDIR:
      return "ENOTDIR";
    case ERRNO_EISDIR:
      return "EISDIR";
    case ERRNO_EINVAL:
      return "EINVAL";
    case ERRNO_ENFILE:
      return "ENFILE";
    case ERRNO_EMFILE:
      return "EMFILE";
    case ERRNO_ENOTTY:
      return "ENOTTY";
    case ERRNO_EFBIG:
      return "EFBIG";
    case ERRNO_ENOSPC:
      return "ENOSPC";
    case ERRNO_ESPIPE:
      return "ESPIPE";
    case ERRNO_EROFS:
      return "EROFS";
    case ERRNO_EMLINK:
      return "EMLINK";
    case ERRNO_EPIPE:
      return "EPIPE";
    case ERRNO_EDOM:
      return "EDOM";
    case ERRNO_ERANGE:
      return "ERANGE";
    case ERRNO_ENOSYS:
      return "ENOSYS";
    case ERRNO_EPROTO:
      return "EPROTO";
    case ERRNO_EBADMSG:
      return "EBADMSG";
    case ERRNO_EOVERFLOW:
      return "EOVERFLOW";
    case ERRNO_EMSGSIZE:
      return "EMSGSIZE";
    case ERRNO_ENOTSUP:
      return "ENOTSUP";
    case ERRNO_EILSEQ:
      return "EILSEQ";
    case ERRNO_EADDRINUSE:
      return "EADDRINUSE";
    case ERRNO_EADDRNOTAVAIL:
      return "EADDRNOTAVAIL";
    case ERRNO_ENETDOWN:
      return "ENETDOWN";
    case ERRNO_ENETUNREACH:
      return "ENETUNREACH";
    case ERRNO_ENETRESET:
      return "ENETRESET";
    case ERRNO_ECONNABORTED:
      return "ECONNABORTED";
    case ERRNO_ECONNRESET:
      return "ECONNRESET";
    case ERRNO_ENOBUFS:
      return "ENOBUFS";
    case ERRNO_EISCONN:
      return "EISCONN";
    case ERRNO_ENOTCONN:
      return "ENOTCONN";
    case ERRNO_ESHUTDOWN:
      return "ESHUTDOWN";
    case ERRNO_ETOOMANYREFS:
      return "ETOOMANYREFS";
    case ERRNO_ETIMEDOUT:
      return "ETIMEDOUT";
    case ERRNO_ECONNREFUSED:
      return "ECONNREFUSED";
    case ERRNO_EHOSTDOWN:
      return "EHOSTDOWN";
    case ERRNO_EHOSTUNREACH:
      return "EHOSTUNREACH";
    case ERRNO_EALREADY:
      return "EALREADY";
    case ERRNO_EINPROGRESS:
      return "EINPROGRESS";
    case ERRNO_ECANCELED:
      return "ECANCELED";
    case ERRNO_ENOTRECOVERABLE:
      return "ENOTRECOVERABLE";
  }
}

/** The member's `kind`. */
export function kind_Errno(val: Errno): Kind {
  switch (val) {
    case ERRNO_OK:
      return KIND_OK;
    case ERRNO_EPERM:
      return KIND_PERMISSION_DENIED;
    case ERRNO_ENOENT:
      return KIND_NOT_FOUND;
    case ERRNO_ESRCH:
      return KIND_NOT_FOUND;
    case ERRNO_EINTR:
      return KIND_UNAVAILABLE;
    case ERRNO_EIO:
      return KIND_UNKNOWN;
    case ERRNO_ENXIO:
      return KIND_NOT_FOUND;
    case ERRNO_E2BIG:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_ENOEXEC:
      return KIND_UNKNOWN;
    case ERRNO_EBADF:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_ECHILD:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EAGAIN:
      return KIND_UNAVAILABLE;
    case ERRNO_ENOMEM:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_EACCES:
      return KIND_PERMISSION_DENIED;
    case ERRNO_EFAULT:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_EBUSY:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EEXIST:
      return KIND_ALREADY_EXISTS;
    case ERRNO_EXDEV:
      return KIND_UNIMPLEMENTED;
    case ERRNO_ENODEV:
      return KIND_NOT_FOUND;
    case ERRNO_ENOTDIR:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EISDIR:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EINVAL:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_ENFILE:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_EMFILE:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_ENOTTY:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_EFBIG:
      return KIND_OUT_OF_RANGE;
    case ERRNO_ENOSPC:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_ESPIPE:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_EROFS:
      return KIND_PERMISSION_DENIED;
    case ERRNO_EMLINK:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_EPIPE:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EDOM:
      return KIND_INVALID_ARGUMENT;
    case ERRNO_ERANGE:
      return KIND_OUT_OF_RANGE;
    case ERRNO_ENOSYS:
      return KIND_UNIMPLEMENTED;
    case ERRNO_EPROTO:
      return KIND_UNKNOWN;
    case ERRNO_EBADMSG:
      return KIND_DATA_LOSS;
    case ERRNO_EOVERFLOW:
      return KIND_OUT_OF_RANGE;
    case ERRNO_EMSGSIZE:
      return KIND_UNKNOWN;
    case ERRNO_ENOTSUP:
      return KIND_UNIMPLEMENTED;
    case ERRNO_EILSEQ:
      return KIND_DATA_LOSS;
    case ERRNO_EADDRINUSE:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_EADDRNOTAVAIL:
      return KIND_ALREADY_EXISTS;
    case ERRNO_ENETDOWN:
      return KIND_UNAVAILABLE;
    case ERRNO_ENETUNREACH:
      return KIND_UNAVAILABLE;
    case ERRNO_ENETRESET:
      return KIND_UNAVAILABLE;
    case ERRNO_ECONNABORTED:
      return KIND_UNAVAILABLE;
    case ERRNO_ECONNRESET:
      return KIND_UNAVAILABLE;
    case ERRNO_ENOBUFS:
      return KIND_RESOURCE_EXHAUSTED;
    case ERRNO_EISCONN:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_ENOTCONN:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_ESHUTDOWN:
      return KIND_FAILED_PRECONDITION;
    case ERRNO_ETOOMANYREFS:
      return KIND_UNKNOWN;
    case ERRNO_ETIMEDOUT:
      return KIND_DEADLINE_EXCEEDED;
    case ERRNO_ECONNREFUSED:
      return KIND_UNAVAILABLE;
    case ERRNO_EHOSTDOWN:
      return KIND_UNAVAILABLE;
    case ERRNO_EHOSTUNREACH:
      return KIND_UNAVAILABLE;
    case ERRNO_EALREADY:
      return KIND_ALREADY_EXISTS;
    case ERRNO_EINPROGRESS:
      return KIND_UNKNOWN;
    case ERRNO_ECANCELED:
      return KIND_CANCELLED;
    case ERRNO_ENOTRECOVERABLE:
      return KIND_INTERNAL;
  }
}

/** The member's `text`. */
export function text_Errno(val: Errno): string {
  switch (val) {
    case ERRNO_OK:
      return "Success";
    case ERRNO_EPERM:
      return "Operation not permitted";
    case ERRNO_ENOENT:
      return "No such file or directory / entry not found";
    case ERRNO_ESRCH:
      return "No such process";
    case ERRNO_EINTR:
      return "Interrupted system call";
    case ERRNO_EIO:
      return "I/O error";
    case ERRNO_ENXIO:
      return "No such device or address";
    case ERRNO_E2BIG:
      return "Argument list too long / object too large for fixed local cap";
    case ERRNO_ENOEXEC:
      return "Exec format error";
    case ERRNO_EBADF:
      return "Bad file descriptor / bad handle";
    case ERRNO_ECHILD:
      return "No child processes";
    case ERRNO_EAGAIN:
      return "Try again / Resource temporarily unavailable";
    case ERRNO_ENOMEM:
      return "Out of memory";
    case ERRNO_EACCES:
      return "Permission denied";
    case ERRNO_EFAULT:
      return "Bad address";
    case ERRNO_EBUSY:
      return "Device or resource busy";
    case ERRNO_EEXIST:
      return "File exists / duplicate bind";
    case ERRNO_EXDEV:
      return "Cross-device link";
    case ERRNO_ENODEV:
      return "No such device";
    case ERRNO_ENOTDIR:
      return "Not a directory";
    case ERRNO_EISDIR:
      return "Is a directory";
    case ERRNO_EINVAL:
      return "Invalid argument";
    case ERRNO_ENFILE:
      return "File table overflow (system-wide)";
    case ERRNO_EMFILE:
      return "Too many open files (process)";
    case ERRNO_ENOTTY:
      return "Not a tty";
    case ERRNO_EFBIG:
      return "File too large / payload too large";
    case ERRNO_ENOSPC:
      return "No space left on device / local mailbox full";
    case ERRNO_ESPIPE:
      return "Illegal seek";
    case ERRNO_EROFS:
      return "Read-only file system";
    case ERRNO_EMLINK:
      return "Too many links";
    case ERRNO_EPIPE:
      return "Broken pipe";
    case ERRNO_EDOM:
      return "Math argument out of domain";
    case ERRNO_ERANGE:
      return "Math result not representable";
    case ERRNO_ENOSYS:
      return "Function not implemented";
    case ERRNO_EPROTO:
      return "Protocol error";
    case ERRNO_EBADMSG:
      return "Bad message (protocol/frame/payload invalid)";
    case ERRNO_EOVERFLOW:
      return "Value too large for target type / overflow";
    case ERRNO_EMSGSIZE:
      return "Message too long / packet too large";
    case ERRNO_ENOTSUP:
      return "Operation not supported";
    case ERRNO_EILSEQ:
      return "Illegal byte sequence (decode/encoding failure)";
    case ERRNO_EADDRINUSE:
      return "Address already in use";
    case ERRNO_EADDRNOTAVAIL:
      return "Cannot assign requested address";
    case ERRNO_ENETDOWN:
      return "Network is down";
    case ERRNO_ENETUNREACH:
      return "Network is unreachable";
    case ERRNO_ENETRESET:
      return "Network dropped connection because of reset";
    case ERRNO_ECONNABORTED:
      return "Software caused connection abort";
    case ERRNO_ECONNRESET:
      return "Connection reset by peer";
    case ERRNO_ENOBUFS:
      return "No buffer space available";
    case ERRNO_EISCONN:
      return "Transport endpoint is already connected";
    case ERRNO_ENOTCONN:
      return "Transport endpoint is not connected";
    case ERRNO_ESHUTDOWN:
      return "Cannot send after transport endpoint shutdown";
    case ERRNO_ETOOMANYREFS:
      return "Too many references";
    case ERRNO_ETIMEDOUT:
      return "Connection timed out";
    case ERRNO_ECONNREFUSED:
      return "Connection refused";
    case ERRNO_EHOSTDOWN:
      return "Host is down";
    case ERRNO_EHOSTUNREACH:
      return "No route to host";
    case ERRNO_EALREADY:
      return "Operation already in progress";
    case ERRNO_EINPROGRESS:
      return "Operation now in progress";
    case ERRNO_ECANCELED:
      return "Operation canceled";
    case ERRNO_ENOTRECOVERABLE:
      return "State not recoverable: an invariant the code stands on does not hold";
  }
}
