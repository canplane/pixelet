// cpp/guest/ipc/file.test.cpp

#include "file.hpp"

#include <string>
#include <vector>

#include "../../kit/test.hpp"

using namespace xpute;

XPUTE_TEST(file_a_descriptor_is_found_by_its_owner_once_its_slot_is_taken_again) {
    std::vector<std::uint64_t> words(128);
    Sys sys(Memory(reinterpret_cast<std::uintptr_t>(words.data()), words.size() * 8), 0, 256);
    Files<std::string> files;
    Fd a = files.open(sys, 7, "a", "first");
    files.close(sys, a);
    Fd b = files.open(sys, 7, "b", "second");
    XPUTE_CHECK(files.held_for([](const std::string &o) { return o == "second"; }) == b);
}
