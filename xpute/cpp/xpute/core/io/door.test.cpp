// cpp/xpute/core/io/door.test.cpp

#include "door.hpp"

#include <cstring>
#include <deque>
#include <source_location>
#include <vector>

#include "xpute/kit/test.hpp"

using namespace xpute;

namespace {

// A frame's room in the test's front.
constexpr std::uint32_t FRAME = 256;
// Where the test keeps what an import names: past both frames, which a packet
// laid in the send frame would write over.
constexpr std::uint32_t SCRATCH = 2 * FRAME;
constexpr std::uint32_t PROGRAM = frame::PROGRAM_MAJOR << 8 | 1;

// What the core sent: its frame, its packet and the memory it named.
struct Sent {
    Frame frame;
    std::vector<std::uint8_t> pkt;
    std::vector<std::uint8_t> named;
};

struct Waiting {
    Frame frame;
    std::vector<std::uint8_t> pkt;
};

// The runtime's end of the test: what `submit` queued is received, and what
// the core sends is kept for the test to read (`sent`). A call is refused.
struct TestHost {
    std::vector<std::uint64_t> buf;
    std::deque<Waiting> waiting;
    std::vector<Sent> sent;

    Memory mem() { return Memory(reinterpret_cast<std::uintptr_t>(buf.data()), buf.size() * 8); }
};

TestHost runtime;

std::int32_t receive(std::uint32_t at, std::uint32_t) {
    if (runtime.waiting.empty()) return 0;
    Waiting w = std::move(runtime.waiting.front());
    runtime.waiting.pop_front();
    w.frame.pkt_len = static_cast<std::uint32_t>(w.pkt.size());
    w.frame.lay(runtime.mem(), at);
    if (!w.pkt.empty()) std::memcpy(runtime.mem().at(at + frame::WORD_BYTES), w.pkt.data(), w.pkt.size());
    return 1;
}

void send(std::uint32_t at, std::uint32_t named_at, std::uint32_t named_len) {
    Memory mem = runtime.mem();
    Frame f = Frame::at(mem, at);
    Result<std::span<const std::uint8_t>> pkt = f.pkt(mem, at, FRAME);
    ensure(pkt.ok(), Errno::ebadmsg);
    const std::uint8_t *named = named_len == 0 ? nullptr : mem.at(named_at);
    runtime.sent.push_back({f, {pkt->begin(), pkt->end()}, {named, named + named_len}});
}

// A door over a front of its own, with the test's runtime handed to it.
Door open_door() {
    runtime = TestHost{.buf = std::vector<std::uint64_t>((SCRATCH + 64) / 8), .waiting = {}, .sent = {}};
    host::set(Host{.receive = receive, .send = send, .call = [](std::uint32_t) { return static_cast<std::int32_t>(Errno::enosys); }});
    return Door(runtime.mem(), 0, FRAME, FRAME);
}

// Queues a frame for the core to receive.
void submit(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::vector<std::uint8_t> pkt = {}) {
    runtime.waiting.push_back({Frame{.tag = tag, .cmd = cmd, .flags = flags, .result = result, .pkt_len = 0}, std::move(pkt)});
}

// What the core has sent since the door was made, taken.
std::vector<Sent> sent() {
    return std::exchange(runtime.sent, {});
}

// A packet of one u32, `val`: NOP's, or an open's reply.
std::vector<std::uint8_t> one(std::uint32_t val) {
    std::vector<std::uint8_t> buf(FRAME);
    xtp::PacketWriter w(buf);
    w.branch(1, [val](xtp::PacketWriter &b) { b.u32(val); });
    Result<std::uint32_t> n = w.finish();
    ensure(n.ok(), Errno::emsgsize);
    buf.resize(*n);
    return buf;
}

template <class Apply> auto program(Apply apply) {
    return [apply](const Event &sub) mutable {
        if (const Program *p = std::get_if<Program>(&sub)) return apply(p->cmd, p->pkt);
        return Outcome{};
    };
}

struct Ping {
    static constexpr std::uint32_t NUMBER = 0x1201;
    static constexpr std::uint32_t FIELDS = 0;
    static constexpr bool ACKREQ = false;
    void write(xtp::PacketWriter &) const noexcept {}
    std::array<std::uint32_t, 2> named() const noexcept { return {0, 0}; }
};

// An import that names `len` bytes of the test's memory at `at`.
struct Shown {
    std::uint32_t at;
    std::uint32_t len;

    static constexpr std::uint32_t NUMBER = 0x1202;
    static constexpr std::uint32_t FIELDS = 0;
    static constexpr bool ACKREQ = false;
    void write(xtp::PacketWriter &) const noexcept {}
    std::array<std::uint32_t, 2> named() const noexcept { return {at, len}; }
};

} // namespace

XPUTE_TEST(door_answers_nop) {
    Door d = open_door();
    submit(4, static_cast<std::uint32_t>(abi::sys::Command::NOP), frame::ACKREQ, 0, one(7));
    bool handed = false;
    d.receive([&](const Event &) { return handed = true, Outcome{}; });
    XPUTE_CHECK_EQ(handed, false, "the program is handed no NOP");
    Frame f = sent().at(0).frame;
    XPUTE_CHECK(f.tag == 4 && f.cmd == static_cast<std::uint32_t>(abi::sys::Command::NOP) && (f.flags & frame::RES) != 0 && f.result == 7);
}

XPUTE_TEST(door_every_waiting_frame_applied_in_one_turn) {
    Door d = open_door();
    for (std::uint32_t tag = 1; tag <= 20; tag++) submit(tag, PROGRAM, frame::ACKREQ, 0);
    d.receive(program([](std::uint32_t, std::span<const std::uint8_t>) { return Outcome{}; }));
    std::vector<Sent> got = sent();
    XPUTE_CHECK_EQ(got.size(), std::size_t{20}, "each once");
    for (std::uint32_t k = 0; k < got.size(); k++) XPUTE_CHECK_EQ(got[k].frame.tag, k + 1, "in order");
}

XPUTE_TEST(door_bad_packet_refused) {
    Door d = open_door();
    submit(5, PROGRAM, 0, 0);
    // A packet longer than the frame's room.
    runtime.waiting.back().pkt.resize(FRAME);
    bool applied = false;
    d.receive(program([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Outcome{}; }));
    XPUTE_CHECK(!applied);
    Frame f = sent().at(0).frame;
    XPUTE_CHECK(f.tag == 5 && f.result == static_cast<std::int32_t>(Errno::ebadmsg));
}

XPUTE_TEST(door_completions_handed_over) {
    Door d = open_door();
    submit(3, static_cast<std::uint32_t>(abi::fs::Command::FS_OPEN), frame::RES, 9, one(404));
    submit(3, static_cast<std::uint32_t>(abi::fs::Command::FS_CLOSE), frame::RES, -5);
    std::optional<abi::fs::FsOpenCompletion> opened;
    std::optional<abi::fs::FsCloseCompletion> closed;
    d.receive([&](const Event &sub) {
        const Completion *c = std::get_if<Completion>(&sub);
        std::optional<abi::fs::Completion> done = c ? abi::fs::completion(*c) : std::nullopt;
        if (done && std::holds_alternative<abi::fs::FsOpenCompletion>(*done)) opened = std::get<abi::fs::FsOpenCompletion>(*done);
        if (done && std::holds_alternative<abi::fs::FsCloseCompletion>(*done)) closed = std::get<abi::fs::FsCloseCompletion>(*done);
        return Outcome{};
    });
    XPUTE_CHECK(opened && opened->fd == 3 && opened->res.value() == 9 && opened->reply && opened->reply->detail == 404);
    XPUTE_CHECK(closed && closed->fd == 3 && closed->res.error().errnum == Errno::eio);
    XPUTE_CHECK_EQ(sent().empty(), true, "a completion is answered by nothing");
}

XPUTE_TEST(door_program_command_handed_over) {
    Door d = open_door();
    submit(6, PROGRAM, frame::ACKREQ, 0);
    std::uint32_t got = 0xffff;
    d.receive(program([&](std::uint32_t cmd, std::span<const std::uint8_t>) { return got = cmd, Outcome{.val = 1}; }));
    XPUTE_CHECK(got == PROGRAM);
    Frame f = sent().at(0).frame;
    XPUTE_CHECK_EQ(f.tag == 6 && f.result == 1, true, "the reply goes back to its sender");
}

XPUTE_TEST(door_xpute_number_unknown_refused) {
    Door d = open_door();
    submit(5, 0x01f1, 0, 0);
    bool applied = false;
    d.receive(program([&](std::uint32_t, std::span<const std::uint8_t>) { return applied = true, Outcome{}; }));
    XPUTE_CHECK(!applied);
    Frame f = sent().at(0).frame;
    XPUTE_CHECK(f.tag == 5 && f.result == static_cast<std::int32_t>(Errno::enosys));
}

XPUTE_TEST(door_call_sends_nothing) {
    Door d = open_door();
    d.send(Ping{});
    Result<std::uint32_t> res = d.call(Ping{});
    XPUTE_CHECK_EQ(!res.ok() && res.error().errnum == Errno::enosys, true, "the test's runtime refuses it");
    XPUTE_CHECK_EQ(sent().size(), std::size_t{1}, "a call is answered, not sent");
}

// The memory an import names goes with its frame, as it was when sent.
XPUTE_TEST(door_named_memory_taken_with_the_frame) {
    Door d = open_door();
    std::uint32_t at = SCRATCH;
    std::uint8_t *p = runtime.mem().at(at);
    p[0] = 7, p[1] = 8, p[2] = 9;
    d.send(Shown{.at = at, .len = 3});
    p[0] = 0;
    XPUTE_CHECK((sent().at(0).named == std::vector<std::uint8_t>{7, 8, 9}));
}

// A stop report with no file path takes `ABORT_PKT_BYTES`, as the writer
// lays it.
XPUTE_TEST(door_abort_pkt_bytes_of_the_format) {
    abi::sys::SysAbort report{
        .errnum = -1,
        .line = UINT32_MAX,
        .column = UINT32_MAX,
        .a_low = UINT32_MAX,
        .a_high = UINT32_MAX,
        .b_low = UINT32_MAX,
        .b_high = UINT32_MAX,
        .file = std::string_view{},
    };
    std::vector<std::uint8_t> buf(256);
    xtp::PacketWriter w(buf);
    w.branch(abi::sys::SysAbort::FIELDS, [&](xtp::PacketWriter &b) { report.write(b); });
    Result<std::uint32_t> n = w.finish();
    XPUTE_CHECK(n && *n == ABORT_PKT_BYTES);
}

XPUTE_TEST(door_abort_sends_the_report_after_what_was_sent) {
    Door d = open_door();
    d.send(Ping{});
    d.send(Ping{});
    d.abort(Errno::einval, std::source_location::current(), 1, 2);
    std::vector<Sent> got = sent();
    XPUTE_CHECK_EQ(got.size(), std::size_t{3}, "nothing sent is taken back");
    XPUTE_CHECK(got[2].frame.tag == 0 && got[2].frame.cmd == static_cast<std::uint32_t>(abi::sys::Command::SYS_ABORT));
    XPUTE_CHECK_EQ(got[2].pkt.empty(), false, "the report carries where it stopped");
}

// A path's token is a string or a number that is a u32; any other node, a
// fraction or a branch, is refused as it is read.
XPUTE_TEST(door_path_takes_u32_and_str_tokens) {
    // The path `fill` writes, as child 0 of a packet in a buffer kept for the
    // test's life.
    auto read = [](auto &&fill) -> Result<Path> {
        static std::array<std::uint8_t, 128> buf;
        xtp::PacketWriter w(buf);
        w.branch(1, fill);
        std::uint32_t n = or_bug(w.finish(), Errno::ebadmsg);
        xtp::TreeReader t = or_bug(xtp::TreeReader::make(std::span<const std::uint8_t>(buf.data(), n)), Errno::ebadmsg);
        return Path::read(or_bug(or_bug(t.read_branch(), Errno::ebadmsg).at(0), Errno::ebadmsg));
    };
    Result<Path> ok = read([](xtp::PacketWriter &w) {
        w.branch(3, [](xtp::PacketWriter &w) { w.u32(0xffffffffu).str("0123").i32(4); });
    });
    XPUTE_CHECK(ok && ok->len() == 3);
    XPUTE_CHECK((ok->at(0) == Token{std::uint32_t{0xffffffffu}} && ok->at(1) == Token{std::string_view("0123")} &&
                 ok->at(2) == Token{std::uint32_t{4}}));
    XPUTE_CHECK(!read([](xtp::PacketWriter &w) { w.branch(1, [](xtp::PacketWriter &w) { w.f64(0.5); }); }));
    XPUTE_CHECK(!read([](xtp::PacketWriter &w) { w.branch(1, [](xtp::PacketWriter &w) { w.i32(-1); }); }));
    XPUTE_CHECK(!read([](xtp::PacketWriter &w) {
        w.branch(1, [](xtp::PacketWriter &w) { w.branch(0, [](xtp::PacketWriter &) {}); });
    }));
}
