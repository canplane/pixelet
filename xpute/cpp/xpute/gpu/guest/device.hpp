// cpp/xpute/gpu/guest/device.hpp

// This reads the device's block, which the guest lends the host to write
// (gpu/wit/gpu.wit `device.lost`), laid out by gpu/link/spec/api.json5's
// DEVICE, as `xpute-gpu-guest`'s `device.rs` reads it.

#pragma once

#include <array>
#include <cstdint>
#include <optional>

#include "xpute/gpu/link/api.spec.hpp"
#include "xpute/gpu/link/enum.spec.hpp"

namespace xpute::gpu {

// The device as its block describes it.
class Device {
  public:
    // Reads the device's block, whose first word is `block`, once the host
    // has stored its STATUS as READY, which this loads with acquire so the
    // whole block is seen; none before then, and none where its format names
    // no texture format. The host writes the limits, the features, the period
    // and the format before READY and none of them after. `block` is the
    // first of `DEVICE::WORDS` live, aligned words, whose STATUS the host
    // stores into only atomically.
    static std::optional<Device> read(const std::uint32_t *block) noexcept;

    std::uint32_t limit(LimitName name) const noexcept;

    bool has(FeatureName feat) const noexcept;

    bool operator==(const Device &) const = default;

    // The timestamp period, in nanoseconds per tick.
    float period_ns;
    // The surface format the host prefers.
    TextureFormat format;

  private:
    Device(const std::uint32_t *block, TextureFormat format) noexcept;

    std::array<std::uint32_t, DEVICE::FEATS_AT> limits_;
    std::array<std::uint32_t, DEVICE::PERIOD_AT - DEVICE::FEATS_AT> feats_;
};

} // namespace xpute::gpu
