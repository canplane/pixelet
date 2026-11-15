// cpp/xpute/gpu/guest/device.cpp

#include "device.hpp"

#include <bit>
#include <limits>

#include "xpute/link/mem.hpp"

namespace xpute::gpu {

std::optional<Device> Device::read(const std::uint32_t *block) noexcept {
    if (link::mem::load_acquire(link::mem::addr(block[DEVICE::STATUS_AT])) != STATUS::READY) return std::nullopt;
    // A word before STATUS is read only once STATUS is READY, after which the
    // host writes none of those read.
    std::uint32_t f = block[DEVICE::FORMAT_AT];
    if (f > std::numeric_limits<std::uint8_t>::max()) return std::nullopt;
    std::optional<TextureFormat> format = of_TextureFormat(static_cast<std::uint8_t>(f));
    if (!format) return std::nullopt;
    return Device(block, *format);
}

Device::Device(const std::uint32_t *block, TextureFormat format) noexcept
    : period_ns(std::bit_cast<float>(block[DEVICE::PERIOD_AT])), format(format) {
    for (std::size_t k = 0; k < limits_.size(); k++) limits_[k] = block[DEVICE::LIMITS_AT + k];
    for (std::size_t k = 0; k < feats_.size(); k++) feats_[k] = block[DEVICE::FEATS_AT + k];
}

std::uint32_t Device::limit(LimitName name) const noexcept {
    return limits_[static_cast<std::size_t>(name)];
}

bool Device::has(FeatureName feat) const noexcept {
    auto k = static_cast<std::size_t>(feat);
    return (feats_[k / 32] >> (k % 32) & 1) != 0;
}

} // namespace xpute::gpu
