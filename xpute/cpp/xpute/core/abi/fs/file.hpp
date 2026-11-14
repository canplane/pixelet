// cpp/xpute/core/abi/fs/file.hpp
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>

namespace xpute::abi::fs::file {

// `[method]file.size`
inline constexpr std::uint32_t FILE_SIZE = 0;
// `[method]file.read`
inline constexpr std::uint32_t FILE_READ = 1;
// `[method]file.write`
inline constexpr std::uint32_t FILE_WRITE = 2;
// `[method]file.changed`
inline constexpr std::uint32_t FILE_CHANGED = 3;
// `[method]file.close`
inline constexpr std::uint32_t FILE_CLOSE = 4;

} // namespace xpute::abi::fs::file
