// cpp/xpute/link/link.hpp

// The link (`link/wit/types.wit`), as xpute-link's `link.rs` runs it.
// Whatever is called is an object, called by its methods under a handle: an
// interface's named handle, the hash of its name (`frame.json5` `NAMED`), or
// a resource's, named by a URI, a tuple of tokens, and opened by the root's
// `open` to a handle the implementing side issues. A frame is one method
// called: its tag the request, unique among the sender's pending ones, its
// command the handle, the method and a `RES` bit (`frame.hpp`), its packet
// the args in the Canonical ABI's layout (`cabi.hpp`). A frame crosses one
// way through one function, `link_out` from the guest and `link_in` into it,
// and each crossing returns at once what the method came to: its result, or
// EINPROGRESS for one that answers later, by a frame marked `RES` with the
// request's tag. So each side has the same ways:
//
// - `call` takes what the crossing returns, for a method that always answers
//   at once, and stops as a bug on EINPROGRESS;
// - `send` hands it to its answer, and an `async func` is sent;
// - `mount_named` mounts an interface's implementation under its named
//   handle, and `mount` puts the opener of an interface's resources the side
//   implements under a URI's leading tokens, the longest matching winning;
//   `open` and `drop` reach the root's open and a handle's drop on the other
//   side;
// - `cancel` cancels a request sent and pending (the root's `CANCEL`), and
//   `on_cancel` puts what runs if a request the side answers is canceled.
//
// The link holds these on both ends:
//
// - One guest entry at a time: no `link_in` starts before another returns,
//   and the outside never enters the guest nested.
// - An answer is never run inside `send`, and a result at once and a later
//   one take the same path. On the guest, the answers run at the end of the
//   entry, once its handler has returned, just before `link_in` returns. An
//   entry ends when no answer is due and the program's end has no work left:
//   those made due while answers run, or by the end's work, run in it too.
// - A frame's packet is the receiver's for the crossing alone, and memory its
//   args lend by (at, len) is the callee's until the answer: the caller keeps
//   it, as the answer it owns.
// - The callee answers in the frame the request came in: it lays it again,
//   marked RES, with its result and its return data as the packet. A call's
//   return data is readable until the next crossing; a send's answer gets a
//   copy, which it owns.
//
// C++ is written for the guest alone, so a packet is always one its side
// writes: none crosses here already written, as one the UI sent does on a
// host.

#pragma once

#include <cstddef>
#include <cstdint>
#include <deque>
#include <functional>
#include <iterator>
#include <map>
#include <memory>
#include <optional>
#include <span>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

#include "xpute/kit/abi/word.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/abi/types.hpp"
#include "cabi.hpp"
#include "frame.hpp"
#include "mem.hpp"
#include "resource.hpp"

namespace xpute::link {

using Token = ::xpute::abi::link::types::Token;
using Lent = ::xpute::abi::link::types::Lent;

using Tag = std::uint32_t;
using Method = std::uint32_t;

using frame::CANCEL;
using frame::DROP;
using frame::NAMED;
using frame::OPEN;
using frame::ROOT;
using frame::TRAP;
using frame::TRAP_AT;

// What an async method lends: a vector of plain values, whose bytes the
// other side may write, which lie in one run of the guest's memory and stay
// put while it is moved, and so while the answer owns it
// (`link/wit/types.wit` `lent`).
template <class L>
concept Lend = requires(L &l) {
    []<class T, class A>(std::vector<T, A> &)
        requires std::is_trivially_copyable_v<T>
    {}(l);
    std::data(l);
};

// `l` as a lent buffer: where it lies in the guest's memory, and how many
// bytes it takes.
template <class L> Lent lent(L &l) noexcept {
    return mem::lent(std::span<const std::remove_reference_t<decltype(*std::data(l))>>(std::data(l), std::size(l)));
}

// A packet a side writes: its fixed part's size, and what writes it
// (`Lower`).
struct Packet {
    std::size_t fixed_sz;
    std::function<void(Lower &)> write;
};

// A method's args. None is no packet.
using Args = std::optional<Packet>;

// A later answer's return data, written as its frame is laid. None is no
// packet.
using Data = std::optional<Packet>;

// An object a side implements: given a method, the request's tag, for a later
// answer (`task_return`), its args (`args`, empty for none) and the writer of
// its answer's value (`ret`), it answers its result, a status (link.ts
// `LinkObject`). The reader and the writer are over the frame the request
// came in, so a method that answers a value writes it once its args are
// read, starting the packet (`Lower::start`), and one that writes none
// answers no packet.
class Object {
  public:
    virtual ~Object() = default;
    virtual std::int32_t call(Method m, Tag tag, Lift args, Lower &ret) = 0;
};

// What hears a sent method's answer: its result, and a copy of its return
// data, which it owns.
using Answer = std::function<void(std::int32_t, std::optional<std::vector<std::uint8_t>>)>;

// What answers an async method's request once the method has its value, and
// where the method says how it stops if its caller cancels it (`CANCEL`).
template <class T> class Return {
  public:
    // The answer to request `tag` on the side whose `on_cancel` is `on`.
    Return(Tag tag, void (*on)(Tag, std::function<void()>), std::function<void(Result<T>)> answer) noexcept
        : tag_(tag), on_(on), answer_(std::move(answer)) {}

    // Answers the request with `v`.
    void answer(Result<T> v) const { answer_(std::move(v)); }

    // Runs `stop` if the caller cancels the request before its answer; the
    // method then answers ECANCELED once it has stopped. A method that
    // cannot stop says nothing, runs on and answers as it would.
    void on_cancel(std::function<void()> stop) const { on_(tag_, std::move(stop)); }

  private:
    Tag tag_;
    void (*on_)(Tag, std::function<void()>);
    std::function<void(Result<T>)> answer_;
};

// What opens the object a URI names, given the handle it is issued.
using Opener = std::function<Result<std::unique_ptr<Object>>(std::span<const Token>, Handle)>;

// The link's layout: the guest's inbox and a frame's cap. The outbox lies
// right past the inbox.
struct Layout {
    std::uint64_t inbox;
    std::uint32_t cap;

    // The layout `init` wrote at `at` (frame.json5 `LAYOUT`).
    static Layout at(std::uint64_t at) noexcept {
        auto w = [at](std::size_t k) { return mem::word(at + 4 * k); };
        return Layout{join64(w(frame::LAYOUT::INBOX_AT), w(frame::LAYOUT::INBOX_AT + 1)), w(frame::LAYOUT::CAP_AT)};
    }

    // Writes the layout at `at`, whose words are the caller's.
    void lay(std::uint64_t at) const noexcept {
        auto set = [at](std::size_t k, std::uint32_t v) { mem::set_word(at + 4 * k, v); };
        auto [lo, hi] = split64(inbox);
        set(frame::LAYOUT::INBOX_AT, lo);
        set(frame::LAYOUT::INBOX_AT + 1, hi);
        set(frame::LAYOUT::CAP_AT, cap);
    }
};

// One side of the link: what both sides do, once. A side lays its frames in
// its own box and crosses by its own function: the guest lays in its outbox
// and crosses by `link_out`. It takes the other side's frame where that side
// laid it (`recv`), and runs the answers due when its owner says the work at
// hand is done (`run_due`): the guest as an entry ends. A later answer this
// side gives while it is crossing is written then and laid with the answers
// due, as xpute-link `link.rs` `Side` holds it.
class Side {
  public:
    Side(std::uint64_t (*out_box)(Layout), std::int32_t (*cross)(std::uint64_t)) noexcept : out_box_(out_box), cross_(cross) {}

    void set_layout(Layout l) noexcept { layout_ = l; }

    Layout layout() const noexcept { return or_bug(layout_, Errno::enotrecoverable); }

    // Whether this side is crossing, its control on the other side.
    bool crossing() const noexcept { return crossing_; }

    // Runs `f`, a crossing of this side's own that is no frame, as crossing.
    template <class F> auto crossed(F &&f) {
        bool was = std::exchange(crossing_, true);
        if constexpr (std::is_void_v<decltype(f())>) {
            f();
            crossing_ = was;
        } else {
            auto v = f();
            crossing_ = was;
            return v;
        }
    }

    // Puts `open` as the opener of the URIs that start with `prefix`, whose
    // objects are the resources of the interface of the named handle `h`.
    void mount(Handle h, std::vector<Token> prefix, Opener open);

    // Mounts `o` under its interface's named handle `h`, which the other
    // side calls with no open.
    void mount_named(Handle h, std::unique_ptr<Object> o);

    // Adds `o` under a handle issued for it, which a method answers for
    // the other side to call `o` by: a resource a function returns.
    Handle add(std::unique_ptr<Object> o) { return objects_.add(std::move(o)); }

    // Opens the object `uri` names on the other side, and answers its
    // handle.
    Result<Handle> open(std::span<const Token> uri);

    // Lets the object `h` names on the other side go.
    void drop(Handle h);

    // Calls `m` of `h`, which answers at once: its result, and its return
    // data, readable until the next crossing.
    std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> call(Handle h, Method m, const Args &args);

    // Sends `m` of `h`, its answer pending from before the crossing, so a
    // later answer given inside it finds its request, and answers the
    // request's tag.
    Tag send(Handle h, Method m, const Args &args, Answer answer);

    // Cancels the request `t` this side sent, while its answer is pending;
    // its answer still comes, and is heard as any other (`CANCEL`).
    void cancel(Tag t);

    // Puts `stop` as how the request `t` this side answers stops if it is
    // canceled (`Return::on_cancel`).
    void on_cancel(Tag t, std::function<void()> stop);

    // Answers the request `t`, whose method answered EINPROGRESS, with
    // `result` and its return data, as the component model's `task.return`.
    // A later answer is found by its tag alone. While this side is crossing,
    // its packet is written now and laid after the work at hand (`run_due`);
    // a packet past the cap is answered EMSGSIZE with no data, as one laid at
    // once is.
    void task_return(Tag t, std::int32_t result, const Data &data);

    // The frame the other side laid at `at`: a later answer, made due, or a
    // method, run by its object, its answer laid in the frame it came in.
    std::int32_t recv(std::uint64_t at);

    // Lays the later answers given while crossing, then runs the answers
    // due, those made due while they run too.
    void run_due();

    // Whether an answer is due.
    bool has_due() const noexcept { return !due_.empty() || !completions_.empty(); }

  private:
    // An answer due: what hears it, its result and its return data.
    struct Due {
        Answer answer;
        std::int32_t result;
        std::optional<std::vector<std::uint8_t>> data;
    };

    // A later answer given while this side is crossing: its request, result
    // and packet, written.
    struct Completion {
        Tag tag;
        std::int32_t result;
        std::optional<std::vector<std::uint8_t>> pkt;
    };

    std::int32_t run(Handle h, Method m, Tag tag, Lift args, Lower &ret);
    Result<Handle> opened(Lift &args);
    Tag tag() noexcept;
    Result<void> lay(std::uint64_t at, Frame words, const Args &pkt);
    std::pair<std::int32_t, std::optional<std::span<const std::uint8_t>>> out(Tag tag, Handle h, Method m, bool res, std::int32_t result, const Args &pkt);

    std::optional<Layout> layout_;
    // The box this side lays its frames in.
    std::uint64_t (*out_box_)(Layout);
    // Crosses with the frame laid at `at`, and answers what the method came
    // to.
    std::int32_t (*cross_)(std::uint64_t);
    // Whether this side is crossing: control is on the other side.
    bool crossing_ = false;
    // Where the next request's tag is sought.
    Tag next_ = 0;
    // The openers, each with the named handle of the interface whose
    // resources it opens, by the URI's leading tokens.
    std::vector<std::tuple<Handle, std::vector<Token>, Opener>> mounts_;
    // The objects this side implements, by the handles it issued.
    Table<std::unique_ptr<Object>> objects_;
    // The interfaces this side implements, by their named handles (`NAMED`).
    std::map<Handle, std::unique_ptr<Object>> named_;
    // The answers of the sent requests that answer later, by tag.
    std::map<Tag, Answer> pending_;
    // How each request this side answers stops if its caller cancels it, by
    // tag, until it answers.
    std::map<Tag, std::function<void()>> cancels_;
    // The answers due, run after the work at hand.
    std::deque<Due> due_;
    // The later answers given while crossing, laid after the work at hand.
    std::deque<Completion> completions_;
};

// The URI of names alone, as a resource's (package, resource).
std::vector<Token> uri(std::initializer_list<std::string_view> names);

// Writes `uri` at `at` as a `list<token>`, as `ROOT` `OPEN`'s args hold it
// (`types.wit`).
void store_uri(Lower &w, std::size_t at, std::span<const Token> uri);

// The URI the `list<token>` at `at` holds (`store_uri`); a read past the
// packet is `r`'s to report (`Lift::finish`).
std::vector<Token> load_uri(Lift &r, std::size_t at);

// A method's result, its status: answered, or the errno it failed with (a
// refusal, or a method's errno where its WIT has no `result`). Its value, if
// it has one, is in its return data.
Result<void> answer(std::int32_t result) noexcept;

// A method's result and the value `load` reads from its return data `data`
// (`Lift`), refused EBADMSG where it has none or does not hold the value.
template <class D, class F>
auto answer(std::int32_t result, const std::optional<D> &data, F &&load) -> Result<decltype(load(std::declval<Lift &>()))> {
    if (Result<void> a = answer(result); !a) return a.error();
    if (!data) return make_error(Errno::ebadmsg);
    Lift r{std::span<const std::uint8_t>(*data)};
    auto v = load(r);
    if (Result<void> f = r.finish(); !f) return f.error();
    return v;
}

} // namespace xpute::link
