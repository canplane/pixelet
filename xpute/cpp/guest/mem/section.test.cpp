// cpp/guest/mem/section.test.cpp

#include "section.hpp"

#include <array>

#include "../../kit/test.hpp"

using namespace xpute;

static constexpr std::uint32_t PAGE = 1 << 12;

XPUTE_TEST(memory_a_range_ends_at_the_boundary_the_next_one_begins_on) {
    XPUTE_CHECK(align_up(0, PAGE) == 0);
    XPUTE_CHECK(align_up(1, PAGE) == PAGE);
    XPUTE_CHECK(align_up(PAGE - 1, PAGE) == PAGE);
    XPUTE_CHECK_EQ(align_up(PAGE, PAGE), PAGE, "a boundary is already one");
    XPUTE_CHECK(align_up(PAGE + 1, PAGE) == 2 * PAGE);
}

XPUTE_TEST(memory_ranges_laid_end_to_end_each_begin_on_the_unit_and_never_overlap) {
    const std::array<std::uint32_t, 4> sizes{PAGE, 1, 3 * PAGE + 1, 0};
    const std::uint32_t base = 8 * PAGE;
    const std::array<std::uint32_t, 4> want{8 * PAGE, 9 * PAGE, 10 * PAGE, 14 * PAGE};
    for (std::size_t i = 0; i < sizes.size(); i++) {
        std::uint32_t a = nth_at(base, sizes, PAGE, i);
        XPUTE_CHECK(a == want[i]);
        XPUTE_CHECK(a % PAGE == 0);
        XPUTE_CHECK_EQ(a + sizes[i] <= nth_at(base, sizes, PAGE, i + 1), true, "runs into the next");
    }
    XPUTE_CHECK_EQ(span_of(sizes, PAGE), 6 * PAGE, "the last one's rounding is in the span");
    XPUTE_CHECK_EQ(nth_at(base, sizes, PAGE, sizes.size()), base + span_of(sizes, PAGE), "and one past the last is where the span ends");
}

XPUTE_TEST(memory_nothing_laid_spans_nothing) {
    XPUTE_CHECK(span_of({}, PAGE) == 0);
    XPUTE_CHECK(nth_at(64, {}, PAGE, 0) == 64);
}

XPUTE_TEST(memory_a_span_inside_a_section_is_reached_only_where_it_ends_by_the_section_s_end) {
    const Section s{4 * PAGE, 2 * PAGE, PAGE};
    XPUTE_CHECK(s.end() == 6 * PAGE);
    XPUTE_CHECK_EQ(s.at(0, 2 * PAGE), std::optional(4 * PAGE), "the whole of it");
    XPUTE_CHECK_EQ(s.at(PAGE, PAGE), std::optional(5 * PAGE), "its last page");
    XPUTE_CHECK_EQ(s.at(2 * PAGE, 0), std::optional(6 * PAGE), "nothing, at its end");
    XPUTE_CHECK_EQ(s.at(PAGE, PAGE + 1), std::nullopt, "a byte past its end");
    XPUTE_CHECK_EQ(s.at(UINT32_MAX, 2), std::nullopt, "past the address space");
}

XPUTE_TEST(memory_a_range_laid_inside_a_section_ends_where_its_own_size_does) {
    const Section s{8 * PAGE, 4 * PAGE, PAGE};
    const std::array<std::uint32_t, 2> sizes{PAGE + 1, PAGE};
    Section second = s.nth(sizes, PAGE, 1);
    XPUTE_CHECK((second == Section{10 * PAGE, PAGE, PAGE}));
    XPUTE_CHECK_EQ(second.at(0, PAGE + 1), std::nullopt, "its own end, not the section's");
}
