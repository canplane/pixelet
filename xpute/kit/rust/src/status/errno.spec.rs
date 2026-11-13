// xpute-kit/status/errno.spec.rs
//
// GENERATED from kit/spec/errno.json (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Errno {
    OK = 0,
    EPERM = -1,
    ENOENT = -2,
    ESRCH = -3,
    EINTR = -4,
    EIO = -5,
    ENXIO = -6,
    E2BIG = -7,
    ENOEXEC = -8,
    EBADF = -9,
    ECHILD = -10,
    EAGAIN = -11,
    ENOMEM = -12,
    EACCES = -13,
    EFAULT = -14,
    EBUSY = -16,
    EEXIST = -17,
    EXDEV = -18,
    ENODEV = -19,
    ENOTDIR = -20,
    EISDIR = -21,
    EINVAL = -22,
    ENFILE = -23,
    EMFILE = -24,
    ENOTTY = -25,
    EFBIG = -27,
    ENOSPC = -28,
    ESPIPE = -29,
    EROFS = -30,
    EMLINK = -31,
    EPIPE = -32,
    EDOM = -33,
    ERANGE = -34,
    ENOSYS = -38,
    EPROTO = -71,
    EBADMSG = -74,
    EOVERFLOW = -75,
    EMSGSIZE = -90,
    ENOTSUP = -95,
    EILSEQ = -84,
    EADDRINUSE = -98,
    EADDRNOTAVAIL = -99,
    ENETDOWN = -100,
    ENETUNREACH = -101,
    ENETRESET = -102,
    ECONNABORTED = -103,
    ECONNRESET = -104,
    ENOBUFS = -105,
    EISCONN = -106,
    ENOTCONN = -107,
    ESHUTDOWN = -108,
    ETOOMANYREFS = -109,
    ETIMEDOUT = -110,
    ECONNREFUSED = -111,
    EHOSTDOWN = -112,
    EHOSTUNREACH = -113,
    EALREADY = -114,
    EINPROGRESS = -115,
    ECANCELED = -125,
    ENOTRECOVERABLE = -131,
}

impl Errno {
    /// Every member, in its source's own order.
    pub const ALL: [Errno; 60] = [
        Errno::OK,
        Errno::EPERM,
        Errno::ENOENT,
        Errno::ESRCH,
        Errno::EINTR,
        Errno::EIO,
        Errno::ENXIO,
        Errno::E2BIG,
        Errno::ENOEXEC,
        Errno::EBADF,
        Errno::ECHILD,
        Errno::EAGAIN,
        Errno::ENOMEM,
        Errno::EACCES,
        Errno::EFAULT,
        Errno::EBUSY,
        Errno::EEXIST,
        Errno::EXDEV,
        Errno::ENODEV,
        Errno::ENOTDIR,
        Errno::EISDIR,
        Errno::EINVAL,
        Errno::ENFILE,
        Errno::EMFILE,
        Errno::ENOTTY,
        Errno::EFBIG,
        Errno::ENOSPC,
        Errno::ESPIPE,
        Errno::EROFS,
        Errno::EMLINK,
        Errno::EPIPE,
        Errno::EDOM,
        Errno::ERANGE,
        Errno::ENOSYS,
        Errno::EPROTO,
        Errno::EBADMSG,
        Errno::EOVERFLOW,
        Errno::EMSGSIZE,
        Errno::ENOTSUP,
        Errno::EILSEQ,
        Errno::EADDRINUSE,
        Errno::EADDRNOTAVAIL,
        Errno::ENETDOWN,
        Errno::ENETUNREACH,
        Errno::ENETRESET,
        Errno::ECONNABORTED,
        Errno::ECONNRESET,
        Errno::ENOBUFS,
        Errno::EISCONN,
        Errno::ENOTCONN,
        Errno::ESHUTDOWN,
        Errno::ETOOMANYREFS,
        Errno::ETIMEDOUT,
        Errno::ECONNREFUSED,
        Errno::EHOSTDOWN,
        Errno::EHOSTUNREACH,
        Errno::EALREADY,
        Errno::EINPROGRESS,
        Errno::ECANCELED,
        Errno::ENOTRECOVERABLE,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: i32) -> Option<Errno> {
        match val {
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

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Errno::OK => "OK",
            Errno::EPERM => "EPERM",
            Errno::ENOENT => "ENOENT",
            Errno::ESRCH => "ESRCH",
            Errno::EINTR => "EINTR",
            Errno::EIO => "EIO",
            Errno::ENXIO => "ENXIO",
            Errno::E2BIG => "E2BIG",
            Errno::ENOEXEC => "ENOEXEC",
            Errno::EBADF => "EBADF",
            Errno::ECHILD => "ECHILD",
            Errno::EAGAIN => "EAGAIN",
            Errno::ENOMEM => "ENOMEM",
            Errno::EACCES => "EACCES",
            Errno::EFAULT => "EFAULT",
            Errno::EBUSY => "EBUSY",
            Errno::EEXIST => "EEXIST",
            Errno::EXDEV => "EXDEV",
            Errno::ENODEV => "ENODEV",
            Errno::ENOTDIR => "ENOTDIR",
            Errno::EISDIR => "EISDIR",
            Errno::EINVAL => "EINVAL",
            Errno::ENFILE => "ENFILE",
            Errno::EMFILE => "EMFILE",
            Errno::ENOTTY => "ENOTTY",
            Errno::EFBIG => "EFBIG",
            Errno::ENOSPC => "ENOSPC",
            Errno::ESPIPE => "ESPIPE",
            Errno::EROFS => "EROFS",
            Errno::EMLINK => "EMLINK",
            Errno::EPIPE => "EPIPE",
            Errno::EDOM => "EDOM",
            Errno::ERANGE => "ERANGE",
            Errno::ENOSYS => "ENOSYS",
            Errno::EPROTO => "EPROTO",
            Errno::EBADMSG => "EBADMSG",
            Errno::EOVERFLOW => "EOVERFLOW",
            Errno::EMSGSIZE => "EMSGSIZE",
            Errno::ENOTSUP => "ENOTSUP",
            Errno::EILSEQ => "EILSEQ",
            Errno::EADDRINUSE => "EADDRINUSE",
            Errno::EADDRNOTAVAIL => "EADDRNOTAVAIL",
            Errno::ENETDOWN => "ENETDOWN",
            Errno::ENETUNREACH => "ENETUNREACH",
            Errno::ENETRESET => "ENETRESET",
            Errno::ECONNABORTED => "ECONNABORTED",
            Errno::ECONNRESET => "ECONNRESET",
            Errno::ENOBUFS => "ENOBUFS",
            Errno::EISCONN => "EISCONN",
            Errno::ENOTCONN => "ENOTCONN",
            Errno::ESHUTDOWN => "ESHUTDOWN",
            Errno::ETOOMANYREFS => "ETOOMANYREFS",
            Errno::ETIMEDOUT => "ETIMEDOUT",
            Errno::ECONNREFUSED => "ECONNREFUSED",
            Errno::EHOSTDOWN => "EHOSTDOWN",
            Errno::EHOSTUNREACH => "EHOSTUNREACH",
            Errno::EALREADY => "EALREADY",
            Errno::EINPROGRESS => "EINPROGRESS",
            Errno::ECANCELED => "ECANCELED",
            Errno::ENOTRECOVERABLE => "ENOTRECOVERABLE",
        }
    }
}

pub mod ERRNO {
    pub mod OK {
        pub const text: &str = "Success";
    }
    pub mod EPERM {
        pub const text: &str = "Operation not permitted";
    }
    pub mod ENOENT {
        pub const text: &str = "No such file or directory / entry not found";
    }
    pub mod ESRCH {
        pub const text: &str = "No such process";
    }
    pub mod EINTR {
        pub const text: &str = "Interrupted system call";
    }
    pub mod EIO {
        pub const text: &str = "I/O error";
    }
    pub mod ENXIO {
        pub const text: &str = "No such device or address";
    }
    pub mod E2BIG {
        pub const text: &str = "Argument list too long / object too large for fixed local cap";
    }
    pub mod ENOEXEC {
        pub const text: &str = "Exec format error";
    }
    pub mod EBADF {
        pub const text: &str = "Bad file descriptor / bad handle";
    }
    pub mod ECHILD {
        pub const text: &str = "No child processes";
    }
    pub mod EAGAIN {
        pub const text: &str = "Try again / Resource temporarily unavailable";
    }
    pub mod ENOMEM {
        pub const text: &str = "Out of memory";
    }
    pub mod EACCES {
        pub const text: &str = "Permission denied";
    }
    pub mod EFAULT {
        pub const text: &str = "Bad address";
    }
    pub mod EBUSY {
        pub const text: &str = "Device or resource busy";
    }
    pub mod EEXIST {
        pub const text: &str = "File exists / duplicate bind";
    }
    pub mod EXDEV {
        pub const text: &str = "Cross-device link";
    }
    pub mod ENODEV {
        pub const text: &str = "No such device";
    }
    pub mod ENOTDIR {
        pub const text: &str = "Not a directory";
    }
    pub mod EISDIR {
        pub const text: &str = "Is a directory";
    }
    pub mod EINVAL {
        pub const text: &str = "Invalid argument";
    }
    pub mod ENFILE {
        pub const text: &str = "File table overflow (system-wide)";
    }
    pub mod EMFILE {
        pub const text: &str = "Too many open files (process)";
    }
    pub mod ENOTTY {
        pub const text: &str = "Not a tty";
    }
    pub mod EFBIG {
        pub const text: &str = "File too large / payload too large";
    }
    pub mod ENOSPC {
        pub const text: &str = "No space left on device / local mailbox full";
    }
    pub mod ESPIPE {
        pub const text: &str = "Illegal seek";
    }
    pub mod EROFS {
        pub const text: &str = "Read-only file system";
    }
    pub mod EMLINK {
        pub const text: &str = "Too many links";
    }
    pub mod EPIPE {
        pub const text: &str = "Broken pipe";
    }
    pub mod EDOM {
        pub const text: &str = "Math argument out of domain";
    }
    pub mod ERANGE {
        pub const text: &str = "Math result not representable";
    }
    pub mod ENOSYS {
        pub const text: &str = "Function not implemented";
    }
    pub mod EPROTO {
        pub const text: &str = "Protocol error";
    }
    pub mod EBADMSG {
        pub const text: &str = "Bad message (protocol/frame/payload invalid)";
    }
    pub mod EOVERFLOW {
        pub const text: &str = "Value too large for target type / overflow";
    }
    pub mod EMSGSIZE {
        pub const text: &str = "Message too long / packet too large";
    }
    pub mod ENOTSUP {
        pub const text: &str = "Operation not supported";
    }
    pub mod EILSEQ {
        pub const text: &str = "Illegal byte sequence (decode/encoding failure)";
    }
    pub mod EADDRINUSE {
        pub const text: &str = "Address already in use";
    }
    pub mod EADDRNOTAVAIL {
        pub const text: &str = "Cannot assign requested address";
    }
    pub mod ENETDOWN {
        pub const text: &str = "Network is down";
    }
    pub mod ENETUNREACH {
        pub const text: &str = "Network is unreachable";
    }
    pub mod ENETRESET {
        pub const text: &str = "Network dropped connection because of reset";
    }
    pub mod ECONNABORTED {
        pub const text: &str = "Software caused connection abort";
    }
    pub mod ECONNRESET {
        pub const text: &str = "Connection reset by peer";
    }
    pub mod ENOBUFS {
        pub const text: &str = "No buffer space available";
    }
    pub mod EISCONN {
        pub const text: &str = "Transport endpoint is already connected";
    }
    pub mod ENOTCONN {
        pub const text: &str = "Transport endpoint is not connected";
    }
    pub mod ESHUTDOWN {
        pub const text: &str = "Cannot send after transport endpoint shutdown";
    }
    pub mod ETOOMANYREFS {
        pub const text: &str = "Too many references";
    }
    pub mod ETIMEDOUT {
        pub const text: &str = "Connection timed out";
    }
    pub mod ECONNREFUSED {
        pub const text: &str = "Connection refused";
    }
    pub mod EHOSTDOWN {
        pub const text: &str = "Runtime is down";
    }
    pub mod EHOSTUNREACH {
        pub const text: &str = "No route to runtime";
    }
    pub mod EALREADY {
        pub const text: &str = "Operation already in progress";
    }
    pub mod EINPROGRESS {
        pub const text: &str = "Operation now in progress";
    }
    pub mod ECANCELED {
        pub const text: &str = "Operation canceled";
    }
    pub mod ENOTRECOVERABLE {
        pub const text: &str =
            "State not recoverable: an invariant the code stands on does not hold";
    }
}
