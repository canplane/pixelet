// cpp/xpute/runtime/ipc/file.test.cpp

#include "file.hpp"

#include <string>
#include <vector>

#include "../../core/test.hpp"

using namespace xpute;

XPUTE_TEST(file_an_answer_to_a_closed_descriptor_reaches_no_one_even_once_its_slot_is_taken_again) {
    std::vector<std::uint64_t> words(128);
    Sys sys(Memory(reinterpret_cast<std::uintptr_t>(words.data()), words.size() * 8), 0, 256);
    Files<std::string> files;
    Fd a = files.open(sys, 7, "a", "first");
    XPUTE_CHECK(*files.owner(a) == "first");
    XPUTE_CHECK(*files.close(sys, a) == "first");
    XPUTE_CHECK_EQ(files.owner(a), nullptr, "a late answer finds no owner");
    Fd b = files.open(sys, 7, "b", "second");
    XPUTE_CHECK_EQ(handle_slot(b), handle_slot(a), "the slot is taken again");
    XPUTE_CHECK(files.owner(a) == nullptr && *files.owner(b) == "second");
    XPUTE_CHECK(files.held_for([](const std::string &o) { return o == "second"; }) == b);
}
