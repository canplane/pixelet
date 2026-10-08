// xpute-kit/status/errno.spec.rs
//
// GENERATED from spec/status/errno.json — do not edit.
//
// POSIX/Linux-aligned negative errno

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Errno {
    OK = 0, // Success

    // ---- Generic ----
    EPERM = -1,    // Operation not permitted
    ENOENT = -2,   // No such file or directory / entry not found
    ESRCH = -3,    // No such process
    EINTR = -4,    // Interrupted system call
    EIO = -5,      // I/O error
    ENXIO = -6,    // No such device or address
    E2BIG = -7,    // Argument list too long / object too large for fixed local cap
    ENOEXEC = -8,  // Exec format error
    EBADF = -9,    // Bad file descriptor / bad handle
    ECHILD = -10,  // No child processes
    EAGAIN = -11,  // Try again / Resource temporarily unavailable
    ENOMEM = -12,  // Out of memory
    EACCES = -13,  // Permission denied
    EFAULT = -14,  // Bad address
    EBUSY = -16,   // Device or resource busy
    EEXIST = -17,  // File exists / duplicate bind
    EXDEV = -18,   // Cross-device link
    ENODEV = -19,  // No such device
    ENOTDIR = -20, // Not a directory
    EISDIR = -21,  // Is a directory
    EINVAL = -22,  // Invalid argument
    ENFILE = -23,  // File table overflow (system-wide)
    EMFILE = -24,  // Too many open files (process)
    ENOTTY = -25,  // Not a tty
    EFBIG = -27,   // File too large / payload too large
    ENOSPC = -28,  // No space left on device / local mailbox full
    ESPIPE = -29,  // Illegal seek
    EROFS = -30,   // Read-only file system
    EMLINK = -31,  // Too many links
    EPIPE = -32,   // Broken pipe
    EDOM = -33,    // Math argument out of domain
    ERANGE = -34,  // Math result not representable
    ENOSYS = -38,  // Function not implemented

    // ---- Protocol / Message / Encoding ----
    EPROTO = -71,    // Protocol error
    EBADMSG = -74,   // Bad message (protocol/frame/payload invalid)
    EOVERFLOW = -75, // Value too large for target type / overflow
    EMSGSIZE = -90,  // Message too long / packet too large
    ENOTSUP = -95,   // Operation not supported
    EILSEQ = -84,    // Illegal byte sequence (decode/encoding failure)

    // ---- Endpoint / Addressing ----
    EADDRINUSE = -98,    // Address already in use
    EADDRNOTAVAIL = -99, // Cannot assign requested address

    // ---- Network / Transport (Linux asm-generic/errno.h aligned) ----
    ENETDOWN = -100,     // Network is down
    ENETUNREACH = -101,  // Network is unreachable
    ENETRESET = -102,    // Network dropped connection because of reset
    ECONNABORTED = -103, // Software caused connection abort
    ECONNRESET = -104,   // Connection reset by peer
    ENOBUFS = -105,      // No buffer space available
    EISCONN = -106,      // Transport endpoint is already connected
    ENOTCONN = -107,     // Transport endpoint is not connected
    ESHUTDOWN = -108,    // Cannot send after transport endpoint shutdown
    ETOOMANYREFS = -109, // Too many references
    ETIMEDOUT = -110,    // Connection timed out
    ECONNREFUSED = -111, // Connection refused
    EHOSTDOWN = -112,    // Host is down
    EHOSTUNREACH = -113, // No route to host
    EALREADY = -114,     // Operation already in progress
    EINPROGRESS = -115,  // Operation now in progress

    // ---- Lifecycle / Cancellation ----
    ECANCELED = -125,       // Operation canceled
    ENOTRECOVERABLE = -131, // State not recoverable: an invariant the code stands on does not hold
}

impl Errno {
    /// The member `code` is, None for a number no member has.
    pub const fn of(code: i32) -> Option<Errno> {
        match code {
            0 => Some(Errno::OK),
            -1 => Some(Errno::EPERM),
            -2 => Some(Errno::ENOENT),
            -3 => Some(Errno::ESRCH),
            -4 => Some(Errno::EINTR),
            -5 => Some(Errno::EIO),
            -6 => Some(Errno::ENXIO),
            -7 => Some(Errno::E2BIG),
            -8 => Some(Errno::ENOEXEC),
            -9 => Some(Errno::EBADF),
            -10 => Some(Errno::ECHILD),
            -11 => Some(Errno::EAGAIN),
            -12 => Some(Errno::ENOMEM),
            -13 => Some(Errno::EACCES),
            -14 => Some(Errno::EFAULT),
            -16 => Some(Errno::EBUSY),
            -17 => Some(Errno::EEXIST),
            -18 => Some(Errno::EXDEV),
            -19 => Some(Errno::ENODEV),
            -20 => Some(Errno::ENOTDIR),
            -21 => Some(Errno::EISDIR),
            -22 => Some(Errno::EINVAL),
            -23 => Some(Errno::ENFILE),
            -24 => Some(Errno::EMFILE),
            -25 => Some(Errno::ENOTTY),
            -27 => Some(Errno::EFBIG),
            -28 => Some(Errno::ENOSPC),
            -29 => Some(Errno::ESPIPE),
            -30 => Some(Errno::EROFS),
            -31 => Some(Errno::EMLINK),
            -32 => Some(Errno::EPIPE),
            -33 => Some(Errno::EDOM),
            -34 => Some(Errno::ERANGE),
            -38 => Some(Errno::ENOSYS),
            -71 => Some(Errno::EPROTO),
            -74 => Some(Errno::EBADMSG),
            -75 => Some(Errno::EOVERFLOW),
            -90 => Some(Errno::EMSGSIZE),
            -95 => Some(Errno::ENOTSUP),
            -84 => Some(Errno::EILSEQ),
            -98 => Some(Errno::EADDRINUSE),
            -99 => Some(Errno::EADDRNOTAVAIL),
            -100 => Some(Errno::ENETDOWN),
            -101 => Some(Errno::ENETUNREACH),
            -102 => Some(Errno::ENETRESET),
            -103 => Some(Errno::ECONNABORTED),
            -104 => Some(Errno::ECONNRESET),
            -105 => Some(Errno::ENOBUFS),
            -106 => Some(Errno::EISCONN),
            -107 => Some(Errno::ENOTCONN),
            -108 => Some(Errno::ESHUTDOWN),
            -109 => Some(Errno::ETOOMANYREFS),
            -110 => Some(Errno::ETIMEDOUT),
            -111 => Some(Errno::ECONNREFUSED),
            -112 => Some(Errno::EHOSTDOWN),
            -113 => Some(Errno::EHOSTUNREACH),
            -114 => Some(Errno::EALREADY),
            -115 => Some(Errno::EINPROGRESS),
            -125 => Some(Errno::ECANCELED),
            -131 => Some(Errno::ENOTRECOVERABLE),
            _ => None,
        }
    }
}

// Result pair: (errno, data)
//
// Conventions:
// - errno == Errno::OK  -> success
// - errno != Errno::OK  -> failure
// - data == None        -> no payload
//
// `None` is the "no value / no payload" sentinel: the empty result slot,
// nothing about pointers.
pub type ErrnoResult<T> = (Errno, Option<T>);
