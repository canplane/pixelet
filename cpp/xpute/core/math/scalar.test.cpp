// cpp/xpute/core/math/scalar.test.cpp

#include <bit>
#include <string>

#include "../golden.hpp"
#include "../test.hpp"
#include "scalar.hpp"

using namespace xpute;

XPUTE_TEST(scalar_computes_what_the_record_holds) {
    Golden v = Golden::load("spec/xpute/golden/math/scalar.tsv");
    v.each("", [&v](const std::string &k) {
        double x = v.f64(k + ".x");
        double s = v.f64(k + ".s");
        auto bits = [&](const char *name, double got) {
            std::string key = k + "." + name;
            XPUTE_CHECK_EQ(std::bit_cast<std::uint64_t>(got), std::bit_cast<std::uint64_t>(v.f64(key)), key.c_str());
        };
        XPUTE_CHECK_EQ(approx_eq(x, x + s * 1e-6), v.boolean(k + ".approx_eq"), (k + ".approx_eq").c_str());
        XPUTE_CHECK_EQ(near(x, s, 0.3), v.boolean(k + ".near"), (k + ".near").c_str());
        bits("lerp", lerp(x, s, 0.3));
        bits("inv_lerp", inv_lerp(x, -s, s * 2.0));
        bits("remap", remap(x, -1.0, 1.0, 0.0, s));
        bits("clamp", clamp(x, -s, s));
        bits("wrap", wrap(x, -180.0, 180.0));
        bits("floor_to_step", floor_to_step(x, s));
        bits("round_to_step", round_to_step(x, s));
        bits("ceil_to_step", ceil_to_step(x, s));
    });
}

XPUTE_TEST(scalar_a_half_way_case_rounds_toward_positive_infinity) {
    XPUTE_CHECK(round_half_up(2.5) == 3.0);
    XPUTE_CHECK(round_half_up(-2.5) == -2.0);
    XPUTE_CHECK(round_half_up(-2.4) == -2.0);
    XPUTE_CHECK(round_half_up(-2.6) == -3.0);
    XPUTE_CHECK(round_half_up(-0.5) == 0.0);
    XPUTE_CHECK(round_half_up(0.49999999999999994) == 0.0);
}
