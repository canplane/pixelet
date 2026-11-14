// cpp/xpute/link/link.test.cpp

#include <memory>
#include <optional>
#include <tuple>
#include <vector>

#include "dev/test/mem.hpp"
#include "dev/test/test.hpp"
#include "link.hpp"

using namespace xpute;
using namespace xpute::link;

namespace {

// A guest's side and its host's, each crossing into the other's `recv`.
extern Side guest, host;
Side guest([](Layout l) { return l.inbox + l.room; }, [](std::uint32_t at) { return host.recv(at); });
Side host([](Layout l) { return l.inbox; }, [](std::uint32_t at) { return guest.recv(at); });

constexpr std::uint32_t ROOM = 64;

// What the host serves: method 0 answers at once with a word, method 1 later,
// and method 2 at once with a value past the room.
constexpr Handle SERVED = NAMED | 1;

std::optional<Tag> later;

constexpr std::int32_t status(Errno e) noexcept {
    return static_cast<std::int32_t>(e);
}

class Served : public Object {
  public:
    Reply call(Method m, Tag tag, std::optional<std::span<const std::uint8_t>>) override {
        if (m == 0) return Reply{0, Packet{4, [](Lower &w) { w.put<std::uint32_t>(0, 7); }}};
        if (m == 1) {
            later = tag;
            return Reply{status(Errno::einprogress)};
        }
        return Reply{0, Packet{ROOM, [](Lower &) {}}};
    }
};

// Lays the link and mounts what the host serves, once, and answers the inbox.
std::uint32_t inbox() {
    static std::uint32_t at = [] {
        Layout l{test::alloc(2 * ROOM), ROOM};
        guest.set_layout(l);
        host.set_layout(l);
        host.mount_named(SERVED, std::make_unique<Served>());
        return l.inbox;
    }();
    return at;
}

} // namespace

// The answer a side lays in the frame its request came in is marked RES and
// carries the request's tag, a value past the room refused EMSGSIZE.
XPUTE_TEST(link_recv_answers_res_with_tag) {
    std::uint32_t outbox = inbox() + ROOM;
    for (auto [m, result] : {std::pair<Method, std::int32_t>{0, 0}, {2, status(Errno::emsgsize)}}) {
        Tag t = guest.send(SERVED, m, std::nullopt, nullptr);
        Frame f = Frame::at(outbox);
        XPUTE_CHECK(f.res && f.tag == t && f.handle == SERVED && f.method == m && f.result == result);
    }
}

// A later answer is marked RES and carries the request's tag, and its request
// hears it, a value past the room refused EMSGSIZE.
XPUTE_TEST(link_task_return_answers_res_with_tag) {
    std::uint32_t at = inbox();
    for (auto [sz, result] : {std::pair<std::size_t, std::int32_t>{4, 0}, {ROOM, status(Errno::emsgsize)}}) {
        std::optional<std::pair<std::int32_t, std::optional<std::vector<std::uint8_t>>>> heard;
        Tag t = guest.send(SERVED, 1, std::nullopt, [&](std::int32_t r, std::optional<std::vector<std::uint8_t>> d) { heard.emplace(r, std::move(d)); });
        XPUTE_CHECK(later == t);
        host.task_return(t, 0, Packet{sz, [](Lower &w) { w.put<std::uint32_t>(0, 9); }});
        Frame f = Frame::at(at);
        XPUTE_CHECK(f.res && f.tag == t && f.result == result);
        guest.run_due();
        XPUTE_CHECK(heard && heard->first == result);
        std::optional<std::vector<std::uint8_t>> value;
        if (result == 0) value = std::vector<std::uint8_t>{9, 0, 0, 0};
        XPUTE_CHECK(heard->second == value);
    }
}

// A cancel runs the stop its request put once, and none once the request has
// answered (`CANCEL`).
XPUTE_TEST(link_cancel_stops_once_and_never_after_answer) {
    inbox();
    int stops = 0;
    auto stop = [&] { stops++; };
    Tag t = guest.send(SERVED, 1, std::nullopt, [](std::int32_t, std::optional<std::vector<std::uint8_t>>) {});
    XPUTE_CHECK(later == t);
    host.on_cancel(t, stop);
    guest.cancel(t);
    guest.cancel(t);
    XPUTE_CHECK(stops == 1);

    t = guest.send(SERVED, 1, std::nullopt, [](std::int32_t, std::optional<std::vector<std::uint8_t>>) {});
    XPUTE_CHECK(later == t);
    host.on_cancel(t, stop);
    host.task_return(t, 0, std::nullopt);
    guest.send(ROOT, CANCEL, Packet{4, [t](Lower &w) { w.put<std::uint32_t>(0, t); }}, nullptr);
    XPUTE_CHECK(stops == 1);
}
