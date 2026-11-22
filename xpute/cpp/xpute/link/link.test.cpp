// cpp/xpute/link/link.test.cpp

#include <memory>
#include <optional>
#include <string>
#include <string_view>
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
Side guest([](Layout l) { return l.inbox + l.cap; }, [](std::uint64_t at) { return host.recv(at); });
Side host([](Layout l) { return l.inbox; }, [](std::uint64_t at) { return guest.recv(at); });

constexpr std::uint32_t CAP = 2048;

// An interface the host implements: method 0 answers at once with a word,
// method 1 later, method 2 at once with a value past the cap, and methods 3
// and 4 fail at once, with a message and with none.
constexpr Handle INTERFACE = NAMED | 1;

std::optional<Tag> later;

constexpr std::int32_t status(Errno e) noexcept {
    return static_cast<std::int32_t>(e);
}

// The error ENOENT made at line 7 of `a/b.rs`, with the message `msg`.
Error made(std::string_view msg) {
    return make_error(Errno::enoent, "a/b.rs", 7, msg);
}

class Implementation : public Object {
  public:
    std::int32_t call(Method m, Tag tag, Lift, Lower &ret) override {
        if (m == 1) {
            later = tag;
            return status(Errno::einprogress);
        }
        if (m == 3) return store_err(ret, made("no café"));
        if (m == 4) return store_err(ret, made(""));
        ret.start(m == 0 ? 4 : CAP);
        if (m == 0) ret.put<std::uint32_t>(0, 7);
        return 0;
    }
};

// Lays the link and mounts the host's implementation, once, and answers the
// inbox.
std::uint64_t inbox() {
    static std::uint64_t at = [] {
        Layout l{test::alloc(2 * CAP), CAP};
        guest.set_layout(l);
        host.set_layout(l);
        host.mount_named(INTERFACE, std::make_unique<Implementation>());
        return l.inbox;
    }();
    return at;
}

} // namespace

// The answer a side lays in the frame its request came in is marked RES and
// carries the request's tag, a value past the cap refused EMSGSIZE.
XPUTE_TEST(link_recv_answers_res_with_tag) {
    std::uint64_t outbox = inbox() + CAP;
    for (auto [m, result] : {std::pair<Method, std::int32_t>{0, 0}, {2, status(Errno::emsgsize)}}) {
        Tag t = guest.send(INTERFACE, m, std::nullopt, nullptr);
        Frame f = Frame::at(outbox);
        XPUTE_CHECK(f.res && f.tag == t && f.handle == INTERFACE && f.method == m && f.result == result);
    }
}

// A later answer is marked RES and carries the request's tag, and its request
// hears it, a value past the cap refused EMSGSIZE.
XPUTE_TEST(link_task_return_answers_res_with_tag) {
    std::uint64_t at = inbox();
    for (auto [sz, result] : {std::pair<std::size_t, std::int32_t>{4, 0}, {CAP, status(Errno::emsgsize)}}) {
        std::optional<std::pair<std::int32_t, std::optional<std::vector<std::uint8_t>>>> heard;
        Tag t = guest.send(INTERFACE, 1, std::nullopt, [&](std::int32_t r, std::optional<std::vector<std::uint8_t>> d) { heard.emplace(r, std::move(d)); });
        XPUTE_CHECK(later == t);
        host.task_return(t, 0, Packet{sz, [](Lower &w) { w.put<std::uint32_t>(0, 9); }});
        Frame f = Frame::at(at);
        XPUTE_CHECK(f.res && f.tag == t && f.result == result);
        guest.run_due();
        XPUTE_CHECK(heard && heard->first == result);
        if (result == 0) {
            XPUTE_CHECK((heard->second == std::vector<std::uint8_t>{9, 0, 0, 0}));
        } else {
            Error e = answer(heard->first, heard->second).error();
            XPUTE_CHECK(e.errnum == Errno::emsgsize && e.file.ends_with("cabi.hpp") && e.line != 0);
        }
    }
}

// An error made at a place with a message arrives with both, and one made
// with no message arrives with none: answered at once, later, and later while
// its side is crossing. A packet that holds no error's is EBADMSG, and a
// trapped link's EPIPE, which no frame carried, is made where it is read.
XPUTE_TEST(link_err_arrives_with_its_msg) {
    inbox();
    // Whether `e` is ENOENT made at line 7 of `a/b.rs` with the message `msg`.
    auto arrived = [](const Error &e, std::string_view msg) {
        return e.errnum == Errno::enoent && e.file == "a/b.rs" && e.line == 7 && e.msg == msg;
    };
    auto [r3, d3] = guest.call(INTERFACE, 3, std::nullopt);
    XPUTE_CHECK(arrived(answer(r3, d3).error(), "no café"));
    auto [r4, d4] = guest.call(INTERFACE, 4, std::nullopt);
    XPUTE_CHECK(arrived(answer(r4, d4).error(), ""));

    for (auto [msg, crossing] : {std::pair<std::string_view, bool>{"no café", false}, {"", false}, {"no café", true}}) {
        std::optional<std::pair<std::int32_t, std::optional<std::vector<std::uint8_t>>>> heard;
        Tag t = guest.send(INTERFACE, 1, std::nullopt, [&](std::int32_t r, std::optional<std::vector<std::uint8_t>> d) { heard.emplace(r, std::move(d)); });
        XPUTE_CHECK(later == t);
        if (crossing) {
            host.crossed([&] { host.task_err(t, made(msg)); });
            XPUTE_CHECK(host.has_due());
            host.run_due();
        } else {
            host.task_err(t, made(msg));
        }
        guest.run_due();
        XPUTE_CHECK(heard && arrived(answer(heard->first, heard->second).error(), msg));
    }

    const std::uint8_t malformed[] = {0x0c, 0, 0, 0};
    Error bad = answer(status(Errno::enoent), std::span<const std::uint8_t>(malformed)).error();
    XPUTE_CHECK(bad.errnum == Errno::ebadmsg && bad.msg.empty());
    Error bare = answer(status(Errno::epipe), std::nullopt).error();
    XPUTE_CHECK(bare.errnum == Errno::epipe && bare.file.ends_with("link.cpp"));
}

// A refusal of the link's own answers in a frame as any error does: a method
// of a named handle nothing is mounted under is ENOENT, made where the link
// refused it.
XPUTE_TEST(link_refusal_is_an_error) {
    std::uint64_t outbox = inbox() + CAP;
    auto [r, data] = guest.call(NAMED | 2, 0, std::nullopt);
    Frame f = Frame::at(outbox);
    XPUTE_CHECK(f.res && f.result == status(Errno::enoent) && f.pkt_len >= ErrorPacket::SZ);
    Error e = answer(r, data).error();
    XPUTE_CHECK(e.errnum == Errno::enoent && e.file.ends_with("link.cpp") && e.line != 0 && e.msg.empty());
}

// A cancel runs the stop its request put once, and none once the request has
// answered (`CANCEL`).
XPUTE_TEST(link_cancel_stops_once_and_never_after_answer) {
    inbox();
    int stops = 0;
    auto stop = [&] { stops++; };
    Tag t = guest.send(INTERFACE, 1, std::nullopt, [](std::int32_t, std::optional<std::vector<std::uint8_t>>) {});
    XPUTE_CHECK(later == t);
    host.on_cancel(t, stop);
    guest.cancel(t);
    guest.cancel(t);
    XPUTE_CHECK(stops == 1);

    t = guest.send(INTERFACE, 1, std::nullopt, [](std::int32_t, std::optional<std::vector<std::uint8_t>>) {});
    XPUTE_CHECK(later == t);
    host.on_cancel(t, stop);
    host.task_return(t, 0, std::nullopt);
    guest.send(ROOT, CANCEL, Packet{4, [t](Lower &w) { w.put<std::uint32_t>(0, t); }}, nullptr);
    XPUTE_CHECK(stops == 1);
}

// A table holds an object under a handle the other end issued, as under its
// own, and its slot follows that end's next generation (`Table::put`), as
// xpute-link's link.test.rs holds it.
XPUTE_TEST(table_holds_other_ends_handle) {
    Table<int> issuer;
    Table<int> holder;
    Handle a = issuer.add(1);
    Handle b = issuer.add(2);
    holder.put(b, 2);
    XPUTE_CHECK(holder.get(b) != nullptr && *holder.get(b) == 2);
    XPUTE_CHECK(holder.get(a) == nullptr);
    XPUTE_CHECK(holder.remove(b) == 2);
    XPUTE_CHECK(issuer.remove(b) == 2);
    Handle c = issuer.add(3);
    holder.put(c, 3);
    holder.put(a, 1);
    XPUTE_CHECK(holder.get(b) == nullptr);
    XPUTE_CHECK(holder.get(c) != nullptr && *holder.get(c) == 3);
    XPUTE_CHECK(holder.get(a) != nullptr && *holder.get(a) == 1);
}
