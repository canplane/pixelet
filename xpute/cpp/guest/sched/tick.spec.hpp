// cpp/guest/sched/tick.spec.hpp
//
// GENERATED from spec/sched/tick.json — do not edit.
//
// A tick's phases, in the order they run and are prioritized: interaction unbudgeted,
// then what is visible, then content, then the cosmetic and the report. A phase's number
// is its place here, and a report of per-phase time is indexed by it.

#pragma once

#include <array>
#include <cstdint>
#include <string_view>

namespace xpute {

enum class Phase : std::uint8_t {
    interaction = 0,
    visible = 1,
    content = 2,
    cosmetic = 3,
    report = 4,
};

inline constexpr std::array<std::string_view, 5> PHASES{"interaction", "visible", "content", "cosmetic", "report"};

} // namespace xpute
