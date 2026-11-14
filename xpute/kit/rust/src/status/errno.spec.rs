// xpute-kit/status/errno.spec.rs
//
// GENERATED from kit/spec/errno.json (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

// A table is generated whole, whether or not this crate calls an entry.
#![allow(dead_code, reason = "generated from the spec")]

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

pub mod errno {
    pub mod ok {
        pub const TEXT: &str = "Success";
    }
    pub mod eperm {
        pub const TEXT: &str = "Operation not permitted";
    }
    pub mod enoent {
        pub const TEXT: &str = "No such file or directory / entry not found";
    }
    pub mod esrch {
        pub const TEXT: &str = "No such process";
    }
    pub mod eintr {
        pub const TEXT: &str = "Interrupted system call";
    }
    pub mod eio {
        pub const TEXT: &str = "I/O error";
    }
    pub mod enxio {
        pub const TEXT: &str = "No such device or address";
    }
    pub mod e2big {
        pub const TEXT: &str = "Argument list too long / object too large for fixed local cap";
    }
    pub mod enoexec {
        pub const TEXT: &str = "Exec format error";
    }
    pub mod ebadf {
        pub const TEXT: &str = "Bad file descriptor / bad handle";
    }
    pub mod echild {
        pub const TEXT: &str = "No child processes";
    }
    pub mod eagain {
        pub const TEXT: &str = "Try again / Resource temporarily unavailable";
    }
    pub mod enomem {
        pub const TEXT: &str = "Out of memory";
    }
    pub mod eacces {
        pub const TEXT: &str = "Permission denied";
    }
    pub mod efault {
        pub const TEXT: &str = "Bad address";
    }
    pub mod ebusy {
        pub const TEXT: &str = "Device or resource busy";
    }
    pub mod eexist {
        pub const TEXT: &str = "File exists / duplicate bind";
    }
    pub mod exdev {
        pub const TEXT: &str = "Cross-device link";
    }
    pub mod enodev {
        pub const TEXT: &str = "No such device";
    }
    pub mod enotdir {
        pub const TEXT: &str = "Not a directory";
    }
    pub mod eisdir {
        pub const TEXT: &str = "Is a directory";
    }
    pub mod einval {
        pub const TEXT: &str = "Invalid argument";
    }
    pub mod enfile {
        pub const TEXT: &str = "File table overflow (system-wide)";
    }
    pub mod emfile {
        pub const TEXT: &str = "Too many open files (process)";
    }
    pub mod enotty {
        pub const TEXT: &str = "Not a tty";
    }
    pub mod efbig {
        pub const TEXT: &str = "File too large / payload too large";
    }
    pub mod enospc {
        pub const TEXT: &str = "No space left on device / local mailbox full";
    }
    pub mod espipe {
        pub const TEXT: &str = "Illegal seek";
    }
    pub mod erofs {
        pub const TEXT: &str = "Read-only file system";
    }
    pub mod emlink {
        pub const TEXT: &str = "Too many links";
    }
    pub mod epipe {
        pub const TEXT: &str = "Broken pipe";
    }
    pub mod edom {
        pub const TEXT: &str = "Math argument out of domain";
    }
    pub mod erange {
        pub const TEXT: &str = "Math result not representable";
    }
    pub mod enosys {
        pub const TEXT: &str = "Function not implemented";
    }
    pub mod eproto {
        pub const TEXT: &str = "Protocol error";
    }
    pub mod ebadmsg {
        pub const TEXT: &str = "Bad message (protocol/frame/payload invalid)";
    }
    pub mod eoverflow {
        pub const TEXT: &str = "Value too large for target type / overflow";
    }
    pub mod emsgsize {
        pub const TEXT: &str = "Message too long / packet too large";
    }
    pub mod enotsup {
        pub const TEXT: &str = "Operation not supported";
    }
    pub mod eilseq {
        pub const TEXT: &str = "Illegal byte sequence (decode/encoding failure)";
    }
    pub mod eaddrinuse {
        pub const TEXT: &str = "Address already in use";
    }
    pub mod eaddrnotavail {
        pub const TEXT: &str = "Cannot assign requested address";
    }
    pub mod enetdown {
        pub const TEXT: &str = "Network is down";
    }
    pub mod enetunreach {
        pub const TEXT: &str = "Network is unreachable";
    }
    pub mod enetreset {
        pub const TEXT: &str = "Network dropped connection because of reset";
    }
    pub mod econnaborted {
        pub const TEXT: &str = "Software caused connection abort";
    }
    pub mod econnreset {
        pub const TEXT: &str = "Connection reset by peer";
    }
    pub mod enobufs {
        pub const TEXT: &str = "No buffer space available";
    }
    pub mod eisconn {
        pub const TEXT: &str = "Transport endpoint is already connected";
    }
    pub mod enotconn {
        pub const TEXT: &str = "Transport endpoint is not connected";
    }
    pub mod eshutdown {
        pub const TEXT: &str = "Cannot send after transport endpoint shutdown";
    }
    pub mod etoomanyrefs {
        pub const TEXT: &str = "Too many references";
    }
    pub mod etimedout {
        pub const TEXT: &str = "Connection timed out";
    }
    pub mod econnrefused {
        pub const TEXT: &str = "Connection refused";
    }
    pub mod ehostdown {
        pub const TEXT: &str = "Runtime is down";
    }
    pub mod ehostunreach {
        pub const TEXT: &str = "No route to runtime";
    }
    pub mod ealready {
        pub const TEXT: &str = "Operation already in progress";
    }
    pub mod einprogress {
        pub const TEXT: &str = "Operation now in progress";
    }
    pub mod ecanceled {
        pub const TEXT: &str = "Operation canceled";
    }
    pub mod enotrecoverable {
        pub const TEXT: &str =
            "State not recoverable: an invariant the code stands on does not hold";
    }
}
