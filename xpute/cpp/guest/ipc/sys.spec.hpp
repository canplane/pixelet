// cpp/guest/ipc/sys.spec.hpp
//
// GENERATED from spec/ipc/sys.json — do not edit.
//
// A guest's system calls: what it asks of its host that no command answers, as records
// in a stream (ipc/stream.rs) the host runs once each turn has returned and before the
// next begins. A record may name memory the guest holds: the host reads or writes it
// in that gap, and the guest keeps it that long. The directory says where the stream
// is (key SYS).
//
// Files are reached as Plan 9 reaches them, in POSIX's words. The guest issues the
// descriptor, a generational handle, and names a file by a root the program numbers and
// a path below it; the host binds each root to where it keeps things and refuses a path
// that leaves it. OPENAT is the one call that waits: the host answers it with the
// command SYS_OPENED (abi/cmd.json) once the file can be read whole. READ and WRITE are
// done in the gap; CLOSE ends the descriptor, opened or not, and nothing is said of it
// after.
//
// A program's own calls go in the same stream, numbered with the PROGRAM bit, and a host
// that is not the program's refuses them.

#pragma once

#include <cstdint>

namespace xpute::sys {

// Words before the first record: END, then a word kept at 0.
inline constexpr std::uint32_t HEAD = 2u;

// A record's call, then its arguments.
namespace op {

// fd, root, flags, then the path's bytes: the file at `path` below `root` opened as `fd`,
// answered by SYS_OPENED with its size or -errno.
inline constexpr std::uint32_t OPENAT = 1u;

// fd, at, len: the opened file's first `len` bytes copied to `at`.
inline constexpr std::uint32_t READ = 2u;

// fd, at, len: the `len` bytes at `at` added to what the file opened for writing holds.
inline constexpr std::uint32_t WRITE = 3u;

// fd: the descriptor ended. A file opened for writing is replaced by what was written; an
// open not yet answered is cancelled, and never answered.
inline constexpr std::uint32_t CLOSE = 4u;

// errno, line, column, a (low, high word), b (low, high word), then the file's bytes:
// the broken invariant the guest reports before it stops (xpute-kit status/bug.rs).
inline constexpr std::uint32_t ABORT = 5u;

// The bit every call a program numbers for itself carries.
inline constexpr std::uint32_t PROGRAM = 2147483648u;
} // namespace op

// How OPENAT opens.
namespace flag {

// To read, whole.
inline constexpr std::uint32_t O_RDONLY = 0u;

// To write: created where it is not, and replaced whole at CLOSE (O_CREAT | O_TRUNC).
inline constexpr std::uint32_t O_WRONLY = 1u;
} // namespace flag

} // namespace xpute::sys
