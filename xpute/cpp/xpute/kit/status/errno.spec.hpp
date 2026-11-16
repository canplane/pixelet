// cpp/xpute/kit/status/errno.spec.hpp
//
// GENERATED from kit/spec/errno.json5 (xpute-gen data.rs)
//
// POSIX/Linux-aligned negative errno

#pragma once

#include <cstdint>
#include <optional>
#include <string_view>

namespace xpute {

// What an errno says happened: the canonical status code of gRPC and Abseil
// (absl::StatusCode), by its number there, each errno's as Abseil's
// ErrnoToStatusCode maps it, save that ENOTRECOVERABLE is internal, and
// EBADMSG and EILSEQ data_loss. It says nothing of whether the error is
// fatal: a broken invariant is a bug, which stops the program.
enum class Kind : std::uint8_t {
    ok = 0,
    cancelled = 1,
    unknown = 2,
    invalid_argument = 3,
    deadline_exceeded = 4,
    not_found = 5,
    already_exists = 6,
    permission_denied = 7,
    resource_exhausted = 8,
    failed_precondition = 9,
    out_of_range = 11,
    unimplemented = 12,
    internal = 13,
    unavailable = 14,
    data_loss = 15,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<Kind> of_Kind(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return Kind::ok;
    case 1: return Kind::cancelled;
    case 2: return Kind::unknown;
    case 3: return Kind::invalid_argument;
    case 4: return Kind::deadline_exceeded;
    case 5: return Kind::not_found;
    case 6: return Kind::already_exists;
    case 7: return Kind::permission_denied;
    case 8: return Kind::resource_exhausted;
    case 9: return Kind::failed_precondition;
    case 11: return Kind::out_of_range;
    case 12: return Kind::unimplemented;
    case 13: return Kind::internal;
    case 14: return Kind::unavailable;
    case 15: return Kind::data_loss;
    }
    return std::nullopt;
}

enum class Errno : std::int32_t {
    ok = 0,
    eperm = -1,
    enoent = -2,
    esrch = -3,
    eintr = -4,
    eio = -5,
    enxio = -6,
    e2big = -7,
    enoexec = -8,
    ebadf = -9,
    echild = -10,
    eagain = -11,
    enomem = -12,
    eacces = -13,
    efault = -14,
    ebusy = -16,
    eexist = -17,
    exdev = -18,
    enodev = -19,
    enotdir = -20,
    eisdir = -21,
    einval = -22,
    enfile = -23,
    emfile = -24,
    enotty = -25,
    efbig = -27,
    enospc = -28,
    espipe = -29,
    erofs = -30,
    emlink = -31,
    epipe = -32,
    edom = -33,
    erange = -34,
    enosys = -38,
    eproto = -71,
    ebadmsg = -74,
    eoverflow = -75,
    emsgsize = -90,
    enotsup = -95,
    eilseq = -84,
    eaddrinuse = -98,
    eaddrnotavail = -99,
    enetdown = -100,
    enetunreach = -101,
    enetreset = -102,
    econnaborted = -103,
    econnreset = -104,
    enobufs = -105,
    eisconn = -106,
    enotconn = -107,
    eshutdown = -108,
    etoomanyrefs = -109,
    etimedout = -110,
    econnrefused = -111,
    ehostdown = -112,
    ehostunreach = -113,
    ealready = -114,
    einprogress = -115,
    ecanceled = -125,
    enotrecoverable = -131,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<Errno> of_Errno(std::int32_t val) noexcept {
    switch (val) {
    case 0: return Errno::ok;
    case -1: return Errno::eperm;
    case -2: return Errno::enoent;
    case -3: return Errno::esrch;
    case -4: return Errno::eintr;
    case -5: return Errno::eio;
    case -6: return Errno::enxio;
    case -7: return Errno::e2big;
    case -8: return Errno::enoexec;
    case -9: return Errno::ebadf;
    case -10: return Errno::echild;
    case -11: return Errno::eagain;
    case -12: return Errno::enomem;
    case -13: return Errno::eacces;
    case -14: return Errno::efault;
    case -16: return Errno::ebusy;
    case -17: return Errno::eexist;
    case -18: return Errno::exdev;
    case -19: return Errno::enodev;
    case -20: return Errno::enotdir;
    case -21: return Errno::eisdir;
    case -22: return Errno::einval;
    case -23: return Errno::enfile;
    case -24: return Errno::emfile;
    case -25: return Errno::enotty;
    case -27: return Errno::efbig;
    case -28: return Errno::enospc;
    case -29: return Errno::espipe;
    case -30: return Errno::erofs;
    case -31: return Errno::emlink;
    case -32: return Errno::epipe;
    case -33: return Errno::edom;
    case -34: return Errno::erange;
    case -38: return Errno::enosys;
    case -71: return Errno::eproto;
    case -74: return Errno::ebadmsg;
    case -75: return Errno::eoverflow;
    case -90: return Errno::emsgsize;
    case -95: return Errno::enotsup;
    case -84: return Errno::eilseq;
    case -98: return Errno::eaddrinuse;
    case -99: return Errno::eaddrnotavail;
    case -100: return Errno::enetdown;
    case -101: return Errno::enetunreach;
    case -102: return Errno::enetreset;
    case -103: return Errno::econnaborted;
    case -104: return Errno::econnreset;
    case -105: return Errno::enobufs;
    case -106: return Errno::eisconn;
    case -107: return Errno::enotconn;
    case -108: return Errno::eshutdown;
    case -109: return Errno::etoomanyrefs;
    case -110: return Errno::etimedout;
    case -111: return Errno::econnrefused;
    case -112: return Errno::ehostdown;
    case -113: return Errno::ehostunreach;
    case -114: return Errno::ealready;
    case -115: return Errno::einprogress;
    case -125: return Errno::ecanceled;
    case -131: return Errno::enotrecoverable;
    }
    return std::nullopt;
}

// The member's `kind`.
constexpr Kind kind_Errno(Errno val) noexcept {
    switch (val) {
    case Errno::ok: return Kind::ok;
    case Errno::eperm: return Kind::permission_denied;
    case Errno::enoent: return Kind::not_found;
    case Errno::esrch: return Kind::not_found;
    case Errno::eintr: return Kind::unavailable;
    case Errno::eio: return Kind::unknown;
    case Errno::enxio: return Kind::not_found;
    case Errno::e2big: return Kind::invalid_argument;
    case Errno::enoexec: return Kind::unknown;
    case Errno::ebadf: return Kind::failed_precondition;
    case Errno::echild: return Kind::failed_precondition;
    case Errno::eagain: return Kind::unavailable;
    case Errno::enomem: return Kind::resource_exhausted;
    case Errno::eacces: return Kind::permission_denied;
    case Errno::efault: return Kind::invalid_argument;
    case Errno::ebusy: return Kind::failed_precondition;
    case Errno::eexist: return Kind::already_exists;
    case Errno::exdev: return Kind::unimplemented;
    case Errno::enodev: return Kind::not_found;
    case Errno::enotdir: return Kind::failed_precondition;
    case Errno::eisdir: return Kind::failed_precondition;
    case Errno::einval: return Kind::invalid_argument;
    case Errno::enfile: return Kind::resource_exhausted;
    case Errno::emfile: return Kind::resource_exhausted;
    case Errno::enotty: return Kind::invalid_argument;
    case Errno::efbig: return Kind::out_of_range;
    case Errno::enospc: return Kind::resource_exhausted;
    case Errno::espipe: return Kind::invalid_argument;
    case Errno::erofs: return Kind::permission_denied;
    case Errno::emlink: return Kind::resource_exhausted;
    case Errno::epipe: return Kind::failed_precondition;
    case Errno::edom: return Kind::invalid_argument;
    case Errno::erange: return Kind::out_of_range;
    case Errno::enosys: return Kind::unimplemented;
    case Errno::eproto: return Kind::unknown;
    case Errno::ebadmsg: return Kind::data_loss;
    case Errno::eoverflow: return Kind::out_of_range;
    case Errno::emsgsize: return Kind::unknown;
    case Errno::enotsup: return Kind::unimplemented;
    case Errno::eilseq: return Kind::data_loss;
    case Errno::eaddrinuse: return Kind::failed_precondition;
    case Errno::eaddrnotavail: return Kind::already_exists;
    case Errno::enetdown: return Kind::unavailable;
    case Errno::enetunreach: return Kind::unavailable;
    case Errno::enetreset: return Kind::unavailable;
    case Errno::econnaborted: return Kind::unavailable;
    case Errno::econnreset: return Kind::unavailable;
    case Errno::enobufs: return Kind::resource_exhausted;
    case Errno::eisconn: return Kind::failed_precondition;
    case Errno::enotconn: return Kind::failed_precondition;
    case Errno::eshutdown: return Kind::failed_precondition;
    case Errno::etoomanyrefs: return Kind::unknown;
    case Errno::etimedout: return Kind::deadline_exceeded;
    case Errno::econnrefused: return Kind::unavailable;
    case Errno::ehostdown: return Kind::unavailable;
    case Errno::ehostunreach: return Kind::unavailable;
    case Errno::ealready: return Kind::already_exists;
    case Errno::einprogress: return Kind::unknown;
    case Errno::ecanceled: return Kind::cancelled;
    case Errno::enotrecoverable: return Kind::internal;
    }
    __builtin_unreachable();
}

// The member's `text`.
constexpr std::string_view text_Errno(Errno val) noexcept {
    switch (val) {
    case Errno::ok: return "Success";
    case Errno::eperm: return "Operation not permitted";
    case Errno::enoent: return "No such file or directory / entry not found";
    case Errno::esrch: return "No such process";
    case Errno::eintr: return "Interrupted system call";
    case Errno::eio: return "I/O error";
    case Errno::enxio: return "No such device or address";
    case Errno::e2big: return "Argument list too long / object too large for fixed local cap";
    case Errno::enoexec: return "Exec format error";
    case Errno::ebadf: return "Bad file descriptor / bad handle";
    case Errno::echild: return "No child processes";
    case Errno::eagain: return "Try again / Resource temporarily unavailable";
    case Errno::enomem: return "Out of memory";
    case Errno::eacces: return "Permission denied";
    case Errno::efault: return "Bad address";
    case Errno::ebusy: return "Device or resource busy";
    case Errno::eexist: return "File exists / duplicate bind";
    case Errno::exdev: return "Cross-device link";
    case Errno::enodev: return "No such device";
    case Errno::enotdir: return "Not a directory";
    case Errno::eisdir: return "Is a directory";
    case Errno::einval: return "Invalid argument";
    case Errno::enfile: return "File table overflow (system-wide)";
    case Errno::emfile: return "Too many open files (process)";
    case Errno::enotty: return "Not a tty";
    case Errno::efbig: return "File too large / payload too large";
    case Errno::enospc: return "No space left on device / local mailbox full";
    case Errno::espipe: return "Illegal seek";
    case Errno::erofs: return "Read-only file system";
    case Errno::emlink: return "Too many links";
    case Errno::epipe: return "Broken pipe";
    case Errno::edom: return "Math argument out of domain";
    case Errno::erange: return "Math result not representable";
    case Errno::enosys: return "Function not implemented";
    case Errno::eproto: return "Protocol error";
    case Errno::ebadmsg: return "Bad message (protocol/frame/payload invalid)";
    case Errno::eoverflow: return "Value too large for target type / overflow";
    case Errno::emsgsize: return "Message too long / packet too large";
    case Errno::enotsup: return "Operation not supported";
    case Errno::eilseq: return "Illegal byte sequence (decode/encoding failure)";
    case Errno::eaddrinuse: return "Address already in use";
    case Errno::eaddrnotavail: return "Cannot assign requested address";
    case Errno::enetdown: return "Network is down";
    case Errno::enetunreach: return "Network is unreachable";
    case Errno::enetreset: return "Network dropped connection because of reset";
    case Errno::econnaborted: return "Software caused connection abort";
    case Errno::econnreset: return "Connection reset by peer";
    case Errno::enobufs: return "No buffer space available";
    case Errno::eisconn: return "Transport endpoint is already connected";
    case Errno::enotconn: return "Transport endpoint is not connected";
    case Errno::eshutdown: return "Cannot send after transport endpoint shutdown";
    case Errno::etoomanyrefs: return "Too many references";
    case Errno::etimedout: return "Connection timed out";
    case Errno::econnrefused: return "Connection refused";
    case Errno::ehostdown: return "Host is down";
    case Errno::ehostunreach: return "No route to host";
    case Errno::ealready: return "Operation already in progress";
    case Errno::einprogress: return "Operation now in progress";
    case Errno::ecanceled: return "Operation canceled";
    case Errno::enotrecoverable: return "State not recoverable: an invariant the code stands on does not hold";
    }
    __builtin_unreachable();
}

} // namespace xpute
