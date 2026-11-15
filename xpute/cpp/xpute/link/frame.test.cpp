// cpp/xpute/link/frame.test.cpp

#include <string>
#include <vector>

#include "xpute/kit/codec/encoding.hpp"
#include "dev/test/golden.hpp"
#include "dev/test/mem.hpp"
#include "dev/test/test.hpp"
#include "frame.hpp"

using namespace xpute;
using link::Frame;
using link::HDR_SZ;

// Each recorded frame's fields lay its words, and its words read back as its
// fields.
XPUTE_TEST(frame_matches_record) {
    Golden g = Golden::load("link/spec/golden/frame.tsv");
    std::uint64_t at = test::alloc(HDR_SZ);
    g.each("", [&](const std::string &k) {
        Frame f{
            .tag = g.u32(k + ".tag"),
            .res = g.boolean(k + ".res"),
            .handle = g.u32(k + ".handle"),
            .method = g.u32(k + ".method"),
            .result = g.i32(k + ".result"),
            .pkt_len = g.u32(k + ".pkt_len"),
        };
        f.lay(at);
        std::string words(hex_len(HDR_SZ), '\0');
        hex_encode(link::mem::bytes(at, HDR_SZ), words);
        XPUTE_CHECK_EQ(words, g.s(k + ".words"), "a frame laid other words");
        std::vector<std::uint8_t> want = g.bytes(k + ".words");
        std::copy(want.begin(), want.end(), link::mem::bytes(at, HDR_SZ).begin());
        Frame back = Frame::at(at);
        XPUTE_CHECK(back.tag == f.tag && back.res == f.res && back.handle == f.handle && back.method == f.method && back.result == f.result &&
                    back.pkt_len == f.pkt_len);
    });
}
