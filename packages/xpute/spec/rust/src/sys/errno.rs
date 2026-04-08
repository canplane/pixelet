// @xpute/spec/sys/errno.rs
// POSIX/Linux-aligned negative errno

pub type Errno = i32;

pub const OK: i32 = 0; // Success

// ---- Generic ----

pub const EPERM: i32 = -1; // Operation not permitted
pub const ENOENT: i32 = -2; // No such file or directory / entry not found
pub const ESRCH: i32 = -3; // No such process
pub const EINTR: i32 = -4; // Interrupted system call
pub const EIO: i32 = -5; // I/O error
pub const ENXIO: i32 = -6; // No such device or address
pub const E2BIG: i32 = -7; // Argument list too long / object too large for fixed local cap
pub const ENOEXEC: i32 = -8; // Exec format error
pub const EBADF: i32 = -9; // Bad file descriptor / bad handle
pub const ECHILD: i32 = -10; // No child processes
pub const EAGAIN: i32 = -11; // Try again / Resource temporarily unavailable
pub const ENOMEM: i32 = -12; // Out of memory
pub const EACCES: i32 = -13; // Permission denied
pub const EFAULT: i32 = -14; // Bad address
pub const EBUSY: i32 = -16; // Device or resource busy
pub const EEXIST: i32 = -17; // File exists / duplicate bind
pub const EXDEV: i32 = -18; // Cross-device link
pub const ENODEV: i32 = -19; // No such device
pub const ENOTDIR: i32 = -20; // Not a directory
pub const EISDIR: i32 = -21; // Is a directory
pub const EINVAL: i32 = -22; // Invalid argument
pub const ENFILE: i32 = -23; // File table overflow (system-wide)
pub const EMFILE: i32 = -24; // Too many open files (process)
pub const ENOTTY: i32 = -25; // Not a tty
pub const EFBIG: i32 = -27; // File too large / payload too large
pub const ENOSPC: i32 = -28; // No space left on device / local mailbox full
pub const ESPIPE: i32 = -29; // Illegal seek
pub const EROFS: i32 = -30; // Read-only file system
pub const EMLINK: i32 = -31; // Too many links
pub const EPIPE: i32 = -32; // Broken pipe
pub const EDOM: i32 = -33; // Math argument out of domain
pub const ERANGE: i32 = -34; // Math result not representable
pub const ENOSYS: i32 = -38; // Function not implemented

// ---- Protocol / Message / Encoding ----

pub const EPROTO: i32 = -71; // Protocol error
pub const EBADMSG: i32 = -74; // Bad message (protocol/frame/payload invalid)
pub const EOVERFLOW: i32 = -75; // Value too large for target type / overflow
pub const EMSGSIZE: i32 = -90; // Message too long / packet too large
pub const ENOTSUP: i32 = -95; // Operation not supported
pub const EILSEQ: i32 = -84; // Illegal byte sequence (decode/encoding failure)

// ---- Endpoint / Addressing ----

pub const EADDRINUSE: i32 = -98; // Address already in use
pub const EADDRNOTAVAIL: i32 = -99; // Cannot assign requested address

// ---- Network / Transport (Linux asm-generic/errno.h aligned) ----

pub const ENETDOWN: i32 = -100; // Network is down
pub const ENETUNREACH: i32 = -101; // Network is unreachable
pub const ENETRESET: i32 = -102; // Network dropped connection because of reset
pub const ECONNABORTED: i32 = -103; // Software caused connection abort
pub const ECONNRESET: i32 = -104; // Connection reset by peer
pub const ENOBUFS: i32 = -105; // No buffer space available
pub const EISCONN: i32 = -106; // Transport endpoint is already connected
pub const ENOTCONN: i32 = -107; // Transport endpoint is not connected
pub const ESHUTDOWN: i32 = -108; // Cannot send after transport endpoint shutdown
pub const ETOOMANYREFS: i32 = -109; // Too many references
pub const ETIMEDOUT: i32 = -110; // Connection timed out
pub const ECONNREFUSED: i32 = -111; // Connection refused
pub const EHOSTDOWN: i32 = -112; // Host is down
pub const EHOSTUNREACH: i32 = -113; // No route to host
pub const EALREADY: i32 = -114; // Operation already in progress
pub const EINPROGRESS: i32 = -115; // Operation now in progress

// ---- Lifecycle / Cancellation ----

pub const ECANCELED: i32 = -125; // Operation canceled

// Result pair: [errno, data]
//
// Conventions:
// - errno == Errno.OK  -> success
// - errno != Errno.OK  -> failure
// - data === null      -> no payload
//
// `null` is used as the canonical "no value / no payload" sentinel.
// It does not imply pointer semantics; it is just the empty result slot.
pub type ErrnoResult<T> = (Errno, Option<T>);
