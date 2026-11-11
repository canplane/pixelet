// cpp/xpute/core/io/file.test.cpp

#include "file.hpp"

#include <array>
#include <string>
#include <tuple>
#include <vector>

#include "dev/test/test.hpp"

using namespace xpute;

namespace {

using Files = FileTable<std::string, std::string, Buffer, 4>;

// A frame the door has sent: its tag, command and flags.
struct Sent {
    std::uint32_t tag, cmd, flags;
};

std::vector<Sent> &sent() {
    static std::vector<Sent> frames;
    return frames;
}

Memory &host_mem() {
    static Memory mem(0);
    return mem;
}

// The path (7, "a").
std::array<Token, 2> path() { return {std::int32_t{7}, std::string_view("a")}; }

// A door whose runtime receives nothing and records each frame it is sent,
// made the core's.
void open_door(std::vector<std::uint64_t> &words) {
    host::set(Host{
        .receive = [](std::uint32_t, std::uint32_t) { return 0; },
        .send =
            [](std::uint32_t at, std::uint32_t, std::uint32_t) {
                Frame f = Frame::at(host_mem(), at);
                sent().push_back({f.tag, f.cmd, f.flags});
            },
        .call = [](std::uint32_t) { return static_cast<std::int32_t>(Errno::enosys); },
    });
    Memory mem(reinterpret_cast<std::uintptr_t>(words.data()), words.size() * 8);
    host_mem() = mem;
    sent().clear();
    door::set(Door(mem, 0, 4096, 4096));
}

// A runtime's result code as a completion carries it.
Result<std::uint32_t> res(std::int32_t code) { return answer_of(code); }

std::uint32_t cmd(abi::fs::Command c) { return static_cast<std::uint32_t>(c); }

std::vector<std::uint32_t> cmds() {
    std::vector<std::uint32_t> out;
    for (const Sent &s : sent()) out.push_back(s.cmd);
    return out;
}

} // namespace

// A descriptor is let go as its close is sent, so its slot is taken again at
// once, under another tag; the old open's completion reaches no one, and the
// old descriptor is not one to close again.
XPUTE_TEST(file_let_go_at_close) {
    std::vector<std::uint64_t> words(1024);
    open_door(words);
    Files files;
    Fd a = files.open(path(), "first");
    XPUTE_CHECK(files.close(a).ok());
    Fd b = files.open(path(), "second");
    XPUTE_CHECK_EQ(abi::handle_slot(a) == abi::handle_slot(b), true, "the slot is free again");
    XPUTE_CHECK_EQ(a != b, true, "under another tag");
    XPUTE_CHECK(!files.complete_open(a, res(-125), 404).has_value());
    XPUTE_CHECK(files.close(a).error().errnum == Errno::ebadf);
    XPUTE_CHECK(files.find([](const std::string &o) { return o == "second"; }) == b);
    XPUTE_CHECK(!files.complete_close(a, res(0)).has_value());
}

// An open and a close are tagged with the descriptor and ask for completion.
XPUTE_TEST(file_open_and_close_ask_for_completion) {
    std::vector<std::uint64_t> words(1024);
    open_door(words);
    Files files;
    Fd fd = files.open(path(), "first");
    XPUTE_CHECK(files.close(fd).ok());
    XPUTE_CHECK((cmds() == std::vector{cmd(abi::fs::Command::FS_OPEN), cmd(abi::fs::Command::FS_CLOSE)}));
    for (const Sent &s : sent()) XPUTE_CHECK(s.tag == fd && (s.flags & frame::ACKREQ) != 0);
}

XPUTE_TEST(file_failed_open_let_go) {
    std::vector<std::uint64_t> words(1024);
    open_door(words);
    Files files;
    Fd failed = files.open(path(), "first");
    auto got = files.complete_open(failed, res(-2), 404);
    XPUTE_CHECK(got && got->first == "first" && got->second.kind == OpenResult::Kind::absent);
    XPUTE_CHECK_EQ(files.close(failed).error().errnum == Errno::ebadf, true, "a failed open leaves no descriptor");
    Fd open = files.open(path(), "second");
    got = files.complete_open(open, res(5), 404);
    XPUTE_CHECK(got && got->first == "second" && got->second.bytes == 5);
    XPUTE_CHECK(files.find([](const std::string &o) { return o == "second"; }) == open);
}

XPUTE_TEST(file_completion_read_once) {
    XPUTE_CHECK((OpenResult::of(res(5), 404) == OpenResult{.kind = OpenResult::Kind::bytes, .bytes = 5}));
    XPUTE_CHECK((OpenResult::of(res(-2), 404) == OpenResult{.kind = OpenResult::Kind::absent}));
    XPUTE_CHECK((OpenResult::of(res(-5), 404) == OpenResult{.kind = OpenResult::Kind::failed, .errnum = Errno::eio, .detail = 404}));
    // A code no errno names breaks the protocol.
    XPUTE_CHECK((OpenResult::of(res(-4095), 404) == OpenResult{.kind = OpenResult::Kind::failed, .errnum = Errno::eproto, .detail = 404}));
}

// A read's request and its buffer are held until its close is completed, and
// handed back then with the close's result, though its descriptor was let go as
// the close was sent. Every frame of the read is tagged with the descriptor.
XPUTE_TEST(file_read_whole_handed_at_close) {
    std::vector<std::uint64_t> words(1024);
    open_door(words);
    Files files;
    Fd fd = files.open(path(), "reader");
    files.read_whole(fd, std::move(*Buffer::make(0)), "read");
    XPUTE_CHECK((cmds() == std::vector{cmd(abi::fs::Command::FS_OPEN), cmd(abi::fs::Command::FS_READ), cmd(abi::fs::Command::FS_CLOSE)}));
    for (const Sent &s : sent()) XPUTE_CHECK(s.tag == fd);
    XPUTE_CHECK_EQ(files.find([](const std::string &) { return true; }).has_value(), false, "let go as its close is sent");
    auto got = files.complete_close(fd, res(0));
    XPUTE_CHECK(got && std::get<0>(*got) == "read" && std::get<1>(*got).has_value() && std::get<2>(*got) == Errno::ok);
    fd = files.open(path(), "reader");
    files.read_whole(fd, std::move(*Buffer::make(0)), "read");
    got = files.complete_close(fd, res(static_cast<std::int32_t>(Errno::ebadf)));
    XPUTE_CHECK(got && std::get<2>(*got) == Errno::ebadf);
}

// A written file is opened to write, written and closed in one turn, each frame
// tagged with its descriptor; it takes no slot, and its request hears the
// close's result once.
XPUTE_TEST(file_write_whole_request_hears_its_close) {
    std::vector<std::uint64_t> words(1024);
    open_door(words);
    Files files;
    std::span<const std::uint8_t> bytes(reinterpret_cast<const std::uint8_t *>(words.data()) + 6000, 2);
    Fd fd = files.write_whole(path(), bytes, "writer");
    XPUTE_CHECK((cmds() == std::vector{cmd(abi::fs::Command::FS_OPEN), cmd(abi::fs::Command::FS_WRITE), cmd(abi::fs::Command::FS_CLOSE)}));
    for (const Sent &s : sent()) XPUTE_CHECK(s.tag == fd);
    XPUTE_CHECK_EQ(abi::handle_slot(fd), 0u, "it takes no slot");
    XPUTE_CHECK_EQ(files.complete_open(fd, res(0), 404).has_value(), false, "it holds no slot");
    Fd next = files.write_whole(path(), bytes.first(0), "next");
    XPUTE_CHECK_EQ(next != fd, true, "under another tag");
    auto got = files.complete_close(fd, res(-30));
    XPUTE_CHECK(got && std::get<0>(*got) == "writer" && !std::get<1>(*got).has_value() && std::get<2>(*got) == Errno::erofs);
    XPUTE_CHECK_EQ(files.complete_close(fd, res(-30)).has_value(), false, "heard once");
}
