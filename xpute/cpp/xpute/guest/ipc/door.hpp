// cpp/xpute/guest/ipc/door.hpp

// The guest's end of its two rings. A command is started only while the
// completion ring has room for all it may leave (`reserve`), so one started
// never finds the ring full; what is not started waits in order. Overrunning
// the ring is a sizing fault.
//
// The guest's hostcalls are frames too: its files (abi/fs.json) and its report
// are posted as any event is; a call answered before it returns is laid
// unpublished in the next slot and its place handed to the host's `hostcall`.
// The host routes each by its command's major.
//
// Both rings keep a ring's rules (ring.hpp R1 to R3), so the door holds
// whether its host takes turns with the guest or runs beside it; and two more,
// io_uring's too:
//
// - R3, as the door keeps it: memory an event names by its address is the
//   host's until the host's head has passed the event (`posted`, `consumed`),
//   so the guest writes it again, or frees it, only then. Memory a call names
//   is the host's only until the call returns.
//
// The last slot of the completion ring is the stop report's alone, so a guest
// that stops can say why without taking back a frame it published.
// - R4. A completion finds its request by its tag, never by where it lies in
//   either ring or when it came.
// - R5. A call never nests: the host answers it without calling into the
//   guest, and the guest makes no call while another is unanswered.

#pragma once

#include <algorithm>
#include <concepts>
#include <cstdint>
#include <memory>
#include <new>
#include <optional>
#include <source_location>
#include <span>
#include <string_view>
#include <utility>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/guest/abi/cmd.spec.hpp"
#include "xpute/guest/abi/fs.spec.hpp"
#include "xpute/guest/hostcall.hpp"
#include "frame.hpp"
#include "ring.hpp"
#include "fs.hpp"

namespace xpute {

// An event a table generates (xpute-tool `spec/cmd`): its number, the
// children it is written as, and its writer.
template <class E>
concept Event = requires(const E &e, xtp::PacketWriter &w) {
    { E::NUMBER } -> std::convertible_to<std::uint32_t>;
    { E::FIELDS } -> std::convertible_to<std::uint32_t>;
    e.write(w);
};

// Errno::enosys for a command the guest does not number.
struct Outcome {
    Errno err = Errno::ok;
    std::int32_t val = 0;
};

// How full the rings ran since the stat was last taken (`Door::take_stat`):
// each figure the most it reached.
struct DoorStat {
    std::uint32_t cq_peak = 0;
    // The submission ring's length as a drain started: how close it came to full.
    std::uint32_t sq_peak = 0;
    // The submissions a drain left for lack of room.
    std::uint32_t left_peak = 0;

    bool operator==(const DoorStat &) const = default;
};

// Where an event was posted in the completion ring: asked of `consumed`.
struct Posted {
    std::uint32_t pos;

    bool operator==(const Posted &) const = default;
};

// Bytes of the heap, given back by their size: what a read fills.
class Bytes {
  public:
    Bytes() noexcept = default;
    Bytes(Bytes &&o) noexcept : at_(std::exchange(o.at_, nullptr)), cnt_(std::exchange(o.cnt_, 0)) {}
    Bytes &operator=(Bytes &&o) noexcept {
        if (this != &o) {
            release();
            at_ = std::exchange(o.at_, nullptr);
            cnt_ = std::exchange(o.cnt_, 0);
        }
        return *this;
    }
    Bytes(const Bytes &) = delete;
    Bytes &operator=(const Bytes &) = delete;
    ~Bytes() { release(); }

    // None where the heap has no room.
    static std::optional<Bytes> make(std::uint32_t cnt) noexcept {
        Bytes b;
        if (cnt == 0) return b;
        b.at_ = static_cast<std::uint8_t *>(::operator new(cnt, std::nothrow));
        if (b.at_ == nullptr) return std::nullopt;
        b.cnt_ = cnt;
        return b;
    }

    std::uint8_t *data() noexcept { return at_; }
    const std::uint8_t *data() const noexcept { return at_; }
    std::size_t size() const noexcept { return cnt_; }
    std::span<std::uint8_t> span() noexcept { return {at_, cnt_}; }
    std::span<const std::uint8_t> span() const noexcept { return {at_, cnt_}; }

  private:
    void release() noexcept {
        if (at_ != nullptr) ::operator delete(at_, cnt_);
        at_ = nullptr;
        cnt_ = 0;
    }

    std::uint8_t *at_ = nullptr;
    std::size_t cnt_ = 0;
};

class Door;

// A read into a buffer the door holds (`Door::read_buf`): the bytes are
// handed over once the host's head has passed the read. A reading destroyed
// before then leaves its buffer with the door, which lets it go once the head
// has passed.
class Reading {
  public:
    Reading(Posted posted, std::shared_ptr<Bytes> buf) noexcept : posted_(posted), buf_(std::move(buf)) {}

    // The bytes, once the host's head has passed the read; none before then,
    // the reading untouched.
    std::optional<Bytes> take(const Door &door) noexcept;

  private:
    Posted posted_;
    std::shared_ptr<Bytes> buf_;
};

// How much of a report's path is kept where the whole would not fit a slot.
inline constexpr std::size_t ABORT_PATH_BYTES = 32;

class Door {
  public:
    // `most_events` is the most one command raises, or the turn once its drain
    // is done: the program's bound.
    Door(Ring sq, Ring cq, std::uint32_t most_events) noexcept : sq_(sq), cq_(cq), reserve_(most_events + 2) {}

    const Ring &sq() const noexcept { return sq_; }
    const Ring &cq() const noexcept { return cq_; }

    DoorStat take_stat() const noexcept { return std::exchange(stat_, {}); }

    // A turn begins: what the reads and writes the host has taken named is let
    // go, a read's buffer only where its reading was destroyed.
    void rise() const noexcept {
        std::erase_if(writes_, [this](const FileWrite &w) { return consumed(w.posted); });
        std::erase_if(reads_, [this](const FileRead &r) { return consumed(r.posted); });
    }

    // The event posted last.
    Posted posted() const noexcept { return {cq_.tail() - 1}; }

    // Whether the host's head has passed the event posted at `posted`: what it
    // named is the guest's again (R3).
    bool consumed(Posted posted) const noexcept { return cq_.consumed(posted.pos); }

    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        posted(cmd, report_slot_kept() ? cq_.push(tag, cmd, flags, result) : Errno::eagain);
    }

    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        posted(cmd, report_slot_kept() ? cq_.push_in_place(tag, cmd, flags, result, write) : Errno::eagain);
    }

    // Whether a frame can be laid and the stop report's slot still be free.
    bool report_slot_kept() const noexcept { return cq_.len() + 1 < cq_.cap(); }

    // An event tagged `tag`.
    template <class Fill> void send_with(std::uint32_t tag, std::uint32_t cmd, std::uint32_t cnt, Fill &&fill) const noexcept {
        post_in_place(tag, cmd, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(cnt, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // `event`, tagged 0.
    template <Event E> void send(const E &event) const noexcept {
        send_with(0, E::NUMBER, E::FIELDS, [&event](xtp::PacketWriter &w) { event.write(w); });
    }

    // `event`, answered before this returns: laid in the next slot unpublished
    // and handed to the host's `hostcall`, its value or its errno; ENOSPC where
    // no slot is free, EMSGSIZE where it does not fit one.
    template <Event E> Result<std::uint32_t> call(const E &event) const noexcept {
        if (!report_slot_kept()) return make_error(ErrorClass::fault, Errno::enospc);
        Result<std::uint32_t> at = cq_.stage_in_place(0, E::NUMBER, frame::ACKREQ, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(E::FIELDS, [&event](xtp::PacketWriter &b) { event.write(b); });
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
        if (!at) return make_error(ErrorClass::fault, at.error().errnum == Errno::eagain ? Errno::enospc : at.error().errnum);
        return answer_of(xpute::hostcall(*at));
    }

    // Answered by FS_OPENED on a later turn.
    void openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) const noexcept {
        send(abi::fs::FsOpenatEvent{.fd = fd, .root = root, .flags = flags, .path = path});
    }

    // `dst` is filled once the host's head has passed the event, and is kept,
    // unread, until `consumed` says so: where it was posted.
    Posted read(Fd fd, std::span<std::uint8_t> dst) const noexcept {
        send(abi::fs::FsReadEvent{.fd = fd, .at = cq_.mem().off_of(dst), .len = static_cast<std::uint32_t>(dst.size())});
        return posted();
    }

    // `buf` is filled once the host's head has passed the event, and is the
    // door's until then.
    Reading read_buf(Fd fd, Bytes buf) const {
        auto shared = std::make_shared<Bytes>(std::move(buf));
        Posted at = read(fd, shared->span());
        reads_.push_back({at, shared});
        return Reading(at, std::move(shared));
    }

    void write(Fd fd, std::vector<std::uint8_t> bytes) const noexcept {
        send(abi::fs::FsWriteEvent{.fd = fd, .at = cq_.mem().off_of(std::span<const std::uint8_t>(bytes)), .len = static_cast<std::uint32_t>(bytes.size())});
        writes_.push_back({posted(), std::move(bytes)});
    }

    // A write is committed, an open not yet answered cancelled.
    void close(Fd fd) const noexcept { send(abi::fs::FsCloseEvent{.fd = fd}); }

    // Posts the report after what the guest already published, as the guest
    // stops, in the slot kept for it: its file's path cut to its end, or left
    // out, where the whole of it would not fit a slot.
    void abort(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) const noexcept {
        std::string_view path = at.file_name();
        std::string_view tail = path.substr(path.size() > ABORT_PATH_BYTES ? path.size() - ABORT_PATH_BYTES : 0);
        for (std::string_view file : {path, tail, std::string_view{}}) {
            abi::SysAbortEvent report{
                .errnum = static_cast<std::int32_t>(errnum),
                .line = at.line(),
                .column = at.column(),
                .a_low = static_cast<std::uint32_t>(a),
                .a_high = static_cast<std::uint32_t>(a >> 32),
                .b_low = static_cast<std::uint32_t>(b),
                .b_high = static_cast<std::uint32_t>(b >> 32),
                .file = file,
            };
            Errno e = cq_.push_in_place(0, abi::SysAbortEvent::NUMBER, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
                xtp::PacketWriter w(out);
                w.branch(abi::SysAbortEvent::FIELDS, [&report](xtp::PacketWriter &b) { report.write(b); });
                Result<std::uint32_t> n = w.finish();
                return n ? std::optional(*n) : std::nullopt;
            });
            if (e != Errno::emsgsize || file.empty()) return posted(abi::SysAbortEvent::NUMBER, e);
        }
    }

    // Applies waiting commands in order while there is room for what one more may
    // leave; replies to ACKREQ and to failures. Answers how many completions it made.
    // A command under xpute's majors is xpute's own and applied here: NOP
    // answers its value, and an open the host completed goes to `opened`;
    // every command past them goes to `apply`. A drain runs before any phase's
    // budget, so the program records what it is handed and returns.
    template <class Opened, class Apply> std::uint32_t receive(Opened &&opened, Apply &&apply) const noexcept {
        std::uint32_t tail = cq_.tail();
        stat_.sq_peak = std::max(stat_.sq_peak, sq_.len());
        while (has_room()) {
            std::optional<Frame> f = sq_.peek();
            if (!f) break;
            Result<std::span<const std::uint8_t>> pkt = sq_.pkt(*f);
            Outcome a = pkt ? apply_frame(*f, *pkt, opened, apply) : Outcome{.err = pkt.error().errnum};
            sq_.advance();
            if (a.err != Errno::ok) {
                post(f->tag, f->cmd, frame::RES, static_cast<std::int32_t>(a.err));
            } else if (f->flags & frame::ACKREQ) {
                post(f->tag, f->cmd, frame::RES, a.val);
            }
        }
        stat_.left_peak = std::max(stat_.left_peak, sq_.len());
        return cq_.tail() - tail;
    }

    bool pending() const noexcept { return !sq_.empty(); }

  private:
    bool has_room() const noexcept { return cq_.len() + reserve_ <= cq_.cap(); }

    template <class Opened, class Apply> static Outcome apply_frame(const Frame &f, std::span<const std::uint8_t> pkt, Opened &opened, Apply &apply) noexcept {
        if (f.cmd >> 8 >= frame::PROGRAM_MAJOR) return apply(f.cmd, pkt);
        if (f.cmd == static_cast<std::uint32_t>(abi::fs::Command::FS_OPENED)) {
            Result<abi::fs::FsOpened> o = abi::fs::FsOpened::read(pkt);
            if (!o) return {.err = o.error().errnum};
            return opened(*o);
        }
        if (f.cmd == static_cast<std::uint32_t>(abi::Command::NOP)) {
            if (pkt.empty()) return {};
            Result<abi::Nop> nop = abi::Nop::read(pkt);
            if (!nop) return {.err = nop.error().errnum};
            return {.val = static_cast<std::int32_t>(nop->val)};
        }
        // The guest's life (SYS_ATTACH, SYS_SUSPEND, SYS_RESUME) and its GPU
        // grant's answer (GPU_READY): a C++ guest keeps nothing across either.
        if (f.cmd == static_cast<std::uint32_t>(abi::Command::SYS_ATTACH) || f.cmd == static_cast<std::uint32_t>(abi::Command::SYS_SUSPEND) ||
            f.cmd == static_cast<std::uint32_t>(abi::Command::SYS_RESUME) || f.cmd == static_cast<std::uint32_t>(abi::Command::GPU_READY))
            return {.val = 0};
        // The guest's own report and file calls, which the host never sends,
        // and the numbers xpute gives nothing.
        return {.err = Errno::enosys};
    }

    void posted(std::uint32_t cmd, Errno e) const noexcept {
        stat_.cq_peak = std::max(stat_.cq_peak, cq_.len());
        ensure(e == Errno::ok, Errno::enospc, cmd, static_cast<std::uint64_t>(static_cast<std::int64_t>(e)));
    }

    Ring sq_;
    Ring cq_;
    // What one command may leave: its events, or the turn's after the drain, and its reply.
    std::uint32_t reserve_;
    // Kept by the const calls that move the rings, as the rings' own words are.
    mutable DoorStat stat_;
    // Kept until the host's head has passed the `write` that names it.
    struct FileWrite {
        Posted posted;
        std::vector<std::uint8_t> bytes;
    };
    mutable std::vector<FileWrite> writes_;
    // Kept until the host's head has passed the `read_buf` that names it,
    // whether or not its reading is still held.
    struct FileRead {
        Posted posted;
        std::shared_ptr<Bytes> buf;
    };
    mutable std::vector<FileRead> reads_;

    friend class Reading;
};

inline std::optional<Bytes> Reading::take(const Door &door) noexcept {
    if (!door.consumed(posted_)) return std::nullopt;
    std::erase_if(door.reads_, [this](const Door::FileRead &r) { return r.posted == posted_; });
    return std::move(*buf_);
}

} // namespace xpute
