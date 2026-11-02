// cpp/xpute/guest/ipc/file.test.cpp

#include "file.hpp"

#include <string>
#include <vector>

#include "xpute/kit/test.hpp"

using namespace xpute;

XPUTE_TEST(file_descriptor_owner) {
    std::vector<std::uint64_t> words(128);
    Sys sys(Memory(reinterpret_cast<std::uintptr_t>(words.data()), words.size() * 8), 0, 256);
    Files<std::string, 8> files;
    Fd a = files.open(sys, 7, "a", "first");
    files.close(sys, a);
    Fd b = files.open(sys, 7, "b", "second");
    XPUTE_CHECK(files.held_for([](const std::string &o) { return o == "second"; }) == b);
}

XPUTE_TEST(file_answer_read_once) {
    auto answer = [](std::int32_t res) { return abi::SysOpened{.fd = 1, .res = res, .detail = 404}; };
    XPUTE_CHECK((Opened::of(answer(5)) == Opened{.kind = Opened::Kind::bytes, .bytes = 5}));
    XPUTE_CHECK((Opened::of(answer(-2)) == Opened{.kind = Opened::Kind::absent}));
    XPUTE_CHECK((Opened::of(answer(-5)) == Opened{.kind = Opened::Kind::failed, .errnum = Errno::eio, .detail = 404}));
    // A code no errno names breaks the protocol.
    XPUTE_CHECK((Opened::of(answer(-4095)) == Opened{.kind = Opened::Kind::failed, .errnum = Errno::eproto, .detail = 404}));
}

XPUTE_TEST(file_read_whole_closes) {
    std::vector<std::uint64_t> words(128);
    Sys sys(Memory(reinterpret_cast<std::uintptr_t>(words.data()), words.size() * 8), 0, 256);
    Files<std::string, 8> files;
    Fd fd = files.open(sys, 7, "a", "first");
    // None of the heap lies in the test's memory, so the file is empty.
    Result<Bytes> bytes = files.read_whole(sys, fd, 0);
    XPUTE_CHECK(bytes && bytes->size() == 0);
    XPUTE_CHECK(files.owner(fd) == nullptr);
}
