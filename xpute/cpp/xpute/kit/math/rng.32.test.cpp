// cpp/xpute/kit/math/rng.32.test.cpp

#include <bit>
#include <string>

#include "xpute/kit/golden.hpp"
#include "xpute/kit/test.hpp"
#include "rng.hpp"

using namespace xpute;

XPUTE_TEST(rng32_computes_what_the_record_holds) {
    Golden v = Golden::load("spec/golden/math/rng.32.tsv");
    v.each("", [&v](const std::string &k) {
        std::uint32_t seed = v.u32(k + ".seed");
        XPUTE_CHECK_EQ(mix32(seed), v.u32(k + ".mix32"), (k + ".mix32").c_str());
        XPUTE_CHECK_EQ(derive_seed_u32(seed, 77), v.u32(k + ".derive"), (k + ".derive").c_str());
        SplitMix32 stream(seed);
        v.each(k + ".draws", [&](const std::string &d) {
            std::uint32_t got = stream.next_u32();
            XPUTE_CHECK_EQ(got, v.u32(d), d.c_str());
            std::string j = d.substr(d.rfind('.') + 1);
            std::string unit = k + ".unit." + j;
            XPUTE_CHECK_EQ(std::bit_cast<std::uint64_t>(u32_to_unit(got)), std::bit_cast<std::uint64_t>(v.f64(unit)), unit.c_str());
        });
    });
}
