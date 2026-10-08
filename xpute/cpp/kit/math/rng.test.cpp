// cpp/kit/math/rng.test.cpp

#include <bit>
#include <string>

#include "../golden.hpp"
#include "../test.hpp"
#include "rng.hpp"

using namespace xpute;

XPUTE_TEST(rng_computes_what_the_record_holds) {
    Golden v = Golden::load("spec/golden/math/rng.tsv");
    v.each("", [&v](const std::string &k) {
        std::uint64_t seed = v.u64(k + ".seed");
        XPUTE_CHECK_EQ(mix64(seed), v.u64(k + ".mix64"), (k + ".mix64").c_str());
        XPUTE_CHECK_EQ(derive_seed_u64(seed, 77), v.u64(k + ".derive"), (k + ".derive").c_str());
        SplitMix64 stream(seed);
        v.each(k + ".draws", [&](const std::string &d) {
            std::uint32_t got = stream.next_u32();
            XPUTE_CHECK_EQ(got, v.u32(d), d.c_str());
            std::string j = d.substr(d.rfind('.') + 1);
            std::string unit = k + ".unit." + j;
            XPUTE_CHECK_EQ(std::bit_cast<std::uint64_t>(u32_to_unit(got)), std::bit_cast<std::uint64_t>(v.f64(unit)), unit.c_str());
        });
    });
}
