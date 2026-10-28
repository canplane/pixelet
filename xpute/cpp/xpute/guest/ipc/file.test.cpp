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
