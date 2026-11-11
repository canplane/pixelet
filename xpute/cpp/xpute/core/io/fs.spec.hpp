// cpp/xpute/core/io/fs.spec.hpp
//
// GENERATED from spec/io/fs.json
//
// The numbers of the file device, whose protocol is abi/fs.json on major FS.

#pragma once

#include <cstdint>

namespace xpute::fs {

// The flags FS_OPEN takes: whether the file is opened to be read whole or written whole.
// Any other value completes the open with EINVAL.
namespace flag {

// Opens the file to be read whole.
inline constexpr std::uint32_t O_RDONLY = 0u;

// Opens the file to be written whole. The file is created if it does not exist, and
// replaced at FS_CLOSE by what was written, as O_CREAT | O_TRUNC would do.
inline constexpr std::uint32_t O_WRONLY = 1u;
} // namespace flag

} // namespace xpute::fs
