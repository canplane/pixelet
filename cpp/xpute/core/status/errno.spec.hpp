// cpp/xpute/core/status/errno.spec.hpp
//
// GENERATED from spec/xpute/status/errno.json — do not edit.
//
// POSIX/Linux-aligned negative errno

#pragma once

#include <cstdint>
#include <optional>
#include <string_view>
#include <utility>

namespace xpute {

enum class Errno : std::int32_t {
    ok = 0, // Success

    // ---- Generic ----
    eperm = -1, // Operation not permitted
    enoent = -2, // No such file or directory / entry not found
    esrch = -3, // No such process
    eintr = -4, // Interrupted system call
    eio = -5, // I/O error
    enxio = -6, // No such device or address
    e2big = -7, // Argument list too long / object too large for fixed local cap
    enoexec = -8, // Exec format error
    ebadf = -9, // Bad file descriptor / bad handle
    echild = -10, // No child processes
    eagain = -11, // Try again / Resource temporarily unavailable
    enomem = -12, // Out of memory
    eacces = -13, // Permission denied
    efault = -14, // Bad address
    ebusy = -16, // Device or resource busy
    eexist = -17, // File exists / duplicate bind
    exdev = -18, // Cross-device link
    enodev = -19, // No such device
    enotdir = -20, // Not a directory
    eisdir = -21, // Is a directory
    einval = -22, // Invalid argument
    enfile = -23, // File table overflow (system-wide)
    emfile = -24, // Too many open files (process)
    enotty = -25, // Not a tty
    efbig = -27, // File too large / payload too large
    enospc = -28, // No space left on device / local mailbox full
    espipe = -29, // Illegal seek
    erofs = -30, // Read-only file system
    emlink = -31, // Too many links
    epipe = -32, // Broken pipe
    edom = -33, // Math argument out of domain
    erange = -34, // Math result not representable
    enosys = -38, // Function not implemented

    // ---- Protocol / Message / Encoding ----
    eproto = -71, // Protocol error
    ebadmsg = -74, // Bad message (protocol/frame/payload invalid)
    eoverflow = -75, // Value too large for target type / overflow
    emsgsize = -90, // Message too long / packet too large
    enotsup = -95, // Operation not supported
    eilseq = -84, // Illegal byte sequence (decode/encoding failure)

    // ---- Endpoint / Addressing ----
    eaddrinuse = -98, // Address already in use
    eaddrnotavail = -99, // Cannot assign requested address

    // ---- Network / Transport (Linux asm-generic/errno.h aligned) ----
    enetdown = -100, // Network is down
    enetunreach = -101, // Network is unreachable
    enetreset = -102, // Network dropped connection because of reset
    econnaborted = -103, // Software caused connection abort
    econnreset = -104, // Connection reset by peer
    enobufs = -105, // No buffer space available
    eisconn = -106, // Transport endpoint is already connected
    enotconn = -107, // Transport endpoint is not connected
    eshutdown = -108, // Cannot send after transport endpoint shutdown
    etoomanyrefs = -109, // Too many references
    etimedout = -110, // Connection timed out
    econnrefused = -111, // Connection refused
    ehostdown = -112, // Host is down
    ehostunreach = -113, // No route to host
    ealready = -114, // Operation already in progress
    einprogress = -115, // Operation now in progress

    // ---- Lifecycle / Cancellation ----
    ecanceled = -125, // Operation canceled
    enotrecoverable = -131, // State not recoverable: an invariant the code stands on does not hold
};

// An errno's name, as Rust's Display prints an error's.
constexpr std::string_view errno_name(Errno e) noexcept {
    switch (e) {
    case Errno::ok: return "OK";
    case Errno::eperm: return "EPERM";
    case Errno::enoent: return "ENOENT";
    case Errno::esrch: return "ESRCH";
    case Errno::eintr: return "EINTR";
    case Errno::eio: return "EIO";
    case Errno::enxio: return "ENXIO";
    case Errno::e2big: return "E2BIG";
    case Errno::enoexec: return "ENOEXEC";
    case Errno::ebadf: return "EBADF";
    case Errno::echild: return "ECHILD";
    case Errno::eagain: return "EAGAIN";
    case Errno::enomem: return "ENOMEM";
    case Errno::eacces: return "EACCES";
    case Errno::efault: return "EFAULT";
    case Errno::ebusy: return "EBUSY";
    case Errno::eexist: return "EEXIST";
    case Errno::exdev: return "EXDEV";
    case Errno::enodev: return "ENODEV";
    case Errno::enotdir: return "ENOTDIR";
    case Errno::eisdir: return "EISDIR";
    case Errno::einval: return "EINVAL";
    case Errno::enfile: return "ENFILE";
    case Errno::emfile: return "EMFILE";
    case Errno::enotty: return "ENOTTY";
    case Errno::efbig: return "EFBIG";
    case Errno::enospc: return "ENOSPC";
    case Errno::espipe: return "ESPIPE";
    case Errno::erofs: return "EROFS";
    case Errno::emlink: return "EMLINK";
    case Errno::epipe: return "EPIPE";
    case Errno::edom: return "EDOM";
    case Errno::erange: return "ERANGE";
    case Errno::enosys: return "ENOSYS";
    case Errno::eproto: return "EPROTO";
    case Errno::ebadmsg: return "EBADMSG";
    case Errno::eoverflow: return "EOVERFLOW";
    case Errno::emsgsize: return "EMSGSIZE";
    case Errno::enotsup: return "ENOTSUP";
    case Errno::eilseq: return "EILSEQ";
    case Errno::eaddrinuse: return "EADDRINUSE";
    case Errno::eaddrnotavail: return "EADDRNOTAVAIL";
    case Errno::enetdown: return "ENETDOWN";
    case Errno::enetunreach: return "ENETUNREACH";
    case Errno::enetreset: return "ENETRESET";
    case Errno::econnaborted: return "ECONNABORTED";
    case Errno::econnreset: return "ECONNRESET";
    case Errno::enobufs: return "ENOBUFS";
    case Errno::eisconn: return "EISCONN";
    case Errno::enotconn: return "ENOTCONN";
    case Errno::eshutdown: return "ESHUTDOWN";
    case Errno::etoomanyrefs: return "ETOOMANYREFS";
    case Errno::etimedout: return "ETIMEDOUT";
    case Errno::econnrefused: return "ECONNREFUSED";
    case Errno::ehostdown: return "EHOSTDOWN";
    case Errno::ehostunreach: return "EHOSTUNREACH";
    case Errno::ealready: return "EALREADY";
    case Errno::einprogress: return "EINPROGRESS";
    case Errno::ecanceled: return "ECANCELED";
    case Errno::enotrecoverable: return "ENOTRECOVERABLE";
    }
    return "errno";
}

// A result as a pair, as Rust's ErrnoResult is: the errno, and the value
// where there is one.
template <class T> using ErrnoResult = std::pair<Errno, std::optional<T>>;

} // namespace xpute
