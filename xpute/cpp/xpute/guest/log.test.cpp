// cpp/xpute/guest/log.test.cpp

// The guest's log against a host of the test's own: a `link_out` that keeps
// the frame the guest lays and answers 0.

#include <cstdint>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "dev/test/golden.hpp"
#include "dev/test/test.hpp"
#include "log.hpp"
#include "xpute/guest/link.hpp"
#include "xpute/link/frame.hpp"
#include "xpute/link/mem.hpp"

using namespace xpute;

namespace {

// The frame the guest last laid, and its packet.
link::Frame laid;
std::vector<std::uint8_t> laid_pkt;

} // namespace

extern "C" std::int32_t link_out(std::uint64_t at) {
    laid = link::Frame::at(at);
    std::span<std::uint8_t> pkt = link::mem::bytes(at + link::HDR_SZ, laid.pkt_len);
    laid_pkt.assign(pkt.begin(), pkt.end());
    return 0;
}

// A record's context and its message past the cap are cut as the record of
// the cut holds them (kit/spec/golden/status/msg.tsv), before the frame that
// tells them is laid.
XPUTE_TEST(guest_log_cuts_its_context_and_msg) {
    Golden g = Golden::load("kit/spec/golden/status/msg.tsv");
    std::vector<std::uint8_t> in = g.bytes("straddle.in");
    std::vector<std::uint8_t> out = g.bytes("straddle.out");
    std::string_view longer(reinterpret_cast<const char *>(in.data()), in.size());
    std::string_view cut(reinterpret_cast<const char *>(out.data()), out.size());
    guest::init(4096, [] {}, [] { return false; });
    guest::log(abi::log::Level::warn, longer, longer);
    XPUTE_CHECK(laid.handle == abi::log::HANDLE && laid.method == abi::log::LOG);
    link::Lift args{std::span<const std::uint8_t>(laid_pkt)};
    XPUTE_CHECK(args.str(4) == cut);
    XPUTE_CHECK(args.str(12) == cut);
    XPUTE_CHECK(args.finish().ok());
}
