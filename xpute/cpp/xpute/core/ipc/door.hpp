// cpp/xpute/core/ipc/door.hpp

// The core's end of its two rings. A command is started only while the
// completion ring has room for all it may leave (`reserve`), so one started
// never finds the ring full; what is not started waits in order. Overrunning
// the ring is a sizing fault.
//
// The core's imports are frames too: its files (abi/fs.json) and its report
// are posted as any import is, an open and a close with ACKREQ, which the runtime
// completes on a later turn with a frame flagged RES on the submission ring; a
// call answered before it returns is laid unpublished in the next slot and its
// place handed to the runtime's `call`. The runtime routes each by its command's
// major.
//
// Both rings keep a ring's rules (ring.hpp R1 to R3), so the door holds
// whether its runtime takes turns with the core or runs beside it; and two more,
// io_uring's too:
//
// - R3, as the door keeps it: memory an import's frame names by its address is
//   the runtime's until the runtime's head has passed the frame (`last_pos`,
//   `is_consumed`), so the core writes it again, or frees it, only then. Memory a call names
//   is the runtime's only until the call returns, and the bytes an FS_WRITE
//   names are the runtime's until its descriptor's FS_CLOSE is completed.
//
// The last slot of the completion ring is the stop report's alone, so a core
// that stops can say why without taking back a frame it published.
// - R4. A completion finds its request by its tag, never by where it lies in
//   either ring or when it came.
// - R5. A call never nests: the runtime answers it without calling into the
//   core, and the core makes no call while another is unanswered.

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
#include <variant>
#include <vector>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/wire/xtp/encode.hpp"
#include "xpute/core/abi/gpu.spec.hpp"
#include "xpute/core/abi/sys.spec.hpp"
#include "xpute/core/abi/fs.spec.hpp"
#include "xpute/core/call.hpp"
#include "frame.hpp"
#include "ring.hpp"
#include "fs.hpp"

namespace xpute {

// An import a table generates (xpute-tool `spec/cmd`): its number, the
// children it is written as, and its writer.
template <class E>
concept Import = requires(const E &e, xtp::PacketWriter &w) {
    { E::NUMBER } -> std::convertible_to<std::uint32_t>;
    { E::FIELDS } -> std::convertible_to<std::uint32_t>;
    e.write(w);
};

// Errno::enosys for a command the core does not number.
struct Outcome {
    Errno err = Errno::ok;
    std::int32_t val = 0;
};

// The core's life as its runtime tells it (abi/sys.json): what each means to
// keep or reset is the program's.
enum class Life { attach, suspend, resume };

// The GPU the machine asked for answered, the first time or again after a
// loss (GPU_READY).
struct GpuReady {};

// A surface the UI lends the GPU (abi/gpu.json): opened, answered with its
// handle, 0 for none; its box resized; let go.
struct SurfaceOpen {};
using Surface = std::variant<SurfaceOpen, abi::gpu::GpuSurfaceResize, abi::gpu::GpuSurfaceClose>;

// A command of the program's own: its number and its packet.
struct Program {
    std::uint32_t cmd = 0;
    std::span<const std::uint8_t> pkt;
};

// What the door hands the program, as xpute-core's `Submission` does: a turn
// of its life, a turn the scheduling policy grants (SYS_TURN), the GPU
// answered, a surface lent, an open or a close the runtime completed, a file
// changed under it (FS_CHANGED), or a command of the program's own. What the
// program answers a completion is dropped.
using Submission = std::variant<Life, abi::sys::SysTurn, GpuReady, Surface, FsOpened, FsClosed, abi::fs::FsChanged, Program>;

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

// Where an import's frame was posted in the completion ring: asked of `is_consumed`.
struct Pos {
    std::uint32_t pos;

    bool operator==(const Pos &) const = default;
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
// handed over once the runtime's head has passed the read. A reading destroyed
// before then leaves its buffer with the door, which lets it go once the head
// has passed. The door outlives it.
class Reading {
  public:
    Reading(const Door &door, Pos pos) noexcept : door_(&door), pos_(pos) {}
    Reading(Reading &&o) noexcept : door_(std::exchange(o.door_, nullptr)), pos_(o.pos_) {}
    Reading &operator=(Reading &&o) noexcept {
        if (this != &o) {
            drop();
            door_ = std::exchange(o.door_, nullptr);
            pos_ = o.pos_;
        }
        return *this;
    }
    Reading(const Reading &) = delete;
    Reading &operator=(const Reading &) = delete;
    ~Reading() { drop(); }

    // The bytes, once the runtime's head has passed the read; none before then,
    // the reading untouched.
    std::optional<Bytes> take(const Door &door) noexcept;

  private:
    // Leaves the buffer to the door.
    void drop() noexcept;

    const Door *door_;
    Pos pos_;
};

// How much of a report's path is kept where the whole would not fit a slot.
inline constexpr std::size_t ABORT_PATH_BYTES = 32;

class Door {
  public:
    // `most_frames` is the most one command raises, or the turn once its drain
    // is done: the program's bound.
    Door(Ring sq, Ring cq, std::uint32_t most_frames) noexcept : sq_(sq), cq_(cq), reserve_(most_frames + 2) {}

    const Ring &sq() const noexcept { return sq_; }
    const Ring &cq() const noexcept { return cq_; }

    DoorStat take_stat() const noexcept { return std::exchange(stat_, {}); }

    // A turn begins: what the reads the runtime has taken named is let go, only
    // where its reading was destroyed.
    void rise() const noexcept {
        std::erase_if(reads_, [this](const FileRead &r) { return r.dropped && is_consumed(r.pos); });
    }

    // The import posted last.
    Pos last_pos() const noexcept { return {cq_.tail() - 1}; }

    // Whether the runtime's head has passed the import posted at `pos`: what it
    // named is the core's again (R3).
    bool is_consumed(Pos pos) const noexcept { return cq_.is_consumed(pos.pos); }

    void post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
        check_post(cmd, can_post() ? cq_.push(tag, cmd, flags, result) : Errno::eagain);
    }

    template <class Write> void post_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        check_post(cmd, can_post() ? cq_.push_in_place(tag, cmd, flags, result, write) : Errno::eagain);
    }

    // Whether a frame can be laid and the stop report's slot still be free.
    bool can_post() const noexcept { return cq_.len() + 1 < cq_.cap(); }

    // A frame tagged `tag` and flagged `flags`.
    template <class Fill> void send_with(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::uint32_t cnt, Fill &&fill) const noexcept {
        post_in_place(tag, cmd, flags, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(cnt, fill);
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
    }

    // `e`, tagged 0.
    template <Import E> void send(const E &e) const noexcept {
        send_with(0, E::NUMBER, 0, E::FIELDS, [&e](xtp::PacketWriter &w) { e.write(w); });
    }

    // `e`, answered before this returns: laid in the next slot unpublished
    // and handed to the runtime's `call`, its value or its errno; ENOSPC where
    // no slot is free, EMSGSIZE where it does not fit one.
    template <Import E> Result<std::uint32_t> call(const E &e) const noexcept {
        if (!can_post()) return make_error(ErrorClass::fault, Errno::enospc);
        Result<std::uint32_t> at = cq_.stage_in_place(0, E::NUMBER, frame::ACKREQ, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
            xtp::PacketWriter w(out);
            w.branch(E::FIELDS, [&e](xtp::PacketWriter &b) { e.write(b); });
            Result<std::uint32_t> n = w.finish();
            return n ? std::optional(*n) : std::nullopt;
        });
        if (!at) return make_error(ErrorClass::fault, at.error().errnum == Errno::eagain ? Errno::enospc : at.error().errnum);
        return answer_of(xpute::call(*at));
    }

    // Completed on a later turn, handed to `receive`'s `opened`.
    void openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) const noexcept {
        abi::fs::FsOpenat e{.fd = fd, .root = root, .flags = flags, .path = path};
        send_with(fd, abi::fs::FsOpenat::NUMBER, frame::ACKREQ, abi::fs::FsOpenat::FIELDS, [&e](xtp::PacketWriter &w) { e.write(w); });
    }

    // `dst` is filled once the runtime's head has passed the import, and is kept,
    // unread, until `is_consumed` says so: where it was posted.
    Pos read(Fd fd, std::span<std::uint8_t> dst) const noexcept {
        send(abi::fs::FsRead{.fd = fd, .at = cq_.mem().off_of(dst), .len = static_cast<std::uint32_t>(dst.size())});
        return last_pos();
    }

    // `buf` is filled once the runtime's head has passed the import, and is the
    // door's until then.
    Reading read_buf(Fd fd, Bytes buf) const {
        Pos at = read(fd, buf.span());
        reads_.push_back({at, std::move(buf), false});
        return Reading(*this, at);
    }

    // `bytes` are the door's until `fd`'s close is completed.
    void write(Fd fd, std::vector<std::uint8_t> bytes) const noexcept {
        send(abi::fs::FsWrite{.fd = fd, .at = cq_.mem().off_of(std::span<const std::uint8_t>(bytes)), .len = static_cast<std::uint32_t>(bytes.size())});
        writes_.push_back({fd, std::move(bytes)});
    }

    // Commits a write, and completes an open not yet completed with
    // ECANCELED; completed on a later turn, handed to `receive`'s `closed`.
    void close(Fd fd) const noexcept {
        abi::fs::FsClose e{.fd = fd};
        send_with(fd, abi::fs::FsClose::NUMBER, frame::ACKREQ, abi::fs::FsClose::FIELDS, [&e](xtp::PacketWriter &w) { e.write(w); });
    }

    // Posts the report after what the core already published, as the core
    // stops, in the slot kept for it: its file's path cut to its end, or left
    // out, where the whole of it would not fit a slot.
    void abort(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) const noexcept {
        std::string_view path = at.file_name();
        std::string_view tail = path.substr(path.size() > ABORT_PATH_BYTES ? path.size() - ABORT_PATH_BYTES : 0);
        for (std::string_view file : {path, tail, std::string_view{}}) {
            abi::sys::SysAbort report{
                .errnum = static_cast<std::int32_t>(errnum),
                .line = at.line(),
                .column = at.column(),
                .a_low = static_cast<std::uint32_t>(a),
                .a_high = static_cast<std::uint32_t>(a >> 32),
                .b_low = static_cast<std::uint32_t>(b),
                .b_high = static_cast<std::uint32_t>(b >> 32),
                .file = file,
            };
            Errno e = cq_.push_in_place(0, abi::sys::SysAbort::NUMBER, 0, 0, [&](std::span<std::uint8_t> out) -> std::optional<std::uint32_t> {
                xtp::PacketWriter w(out);
                w.branch(abi::sys::SysAbort::FIELDS, [&report](xtp::PacketWriter &b) { report.write(b); });
                Result<std::uint32_t> n = w.finish();
                return n ? std::optional(*n) : std::nullopt;
            });
            if (e != Errno::emsgsize || file.empty()) return check_post(abi::sys::SysAbort::NUMBER, e);
        }
    }

    // Applies waiting commands in order while there is room for what one more may
    // leave; replies to ACKREQ and to failures, and to no completion. Answers how
    // many completions it made. NOP answers its value here; every other command
    // goes to `handle` as what it means (`Submission`), a close the runtime
    // completed once the bytes its descriptor's writes named are let go. A drain
    // runs before any phase's budget, so the program records what it is handed
    // and returns.
    template <class Handle> std::uint32_t receive(Handle &&handle) const noexcept {
        std::uint32_t tail = cq_.tail();
        stat_.sq_peak = std::max(stat_.sq_peak, sq_.len());
        while (has_room()) {
            std::optional<Frame> f = sq_.peek();
            if (!f) break;
            if (f->flags & frame::RES) {
                complete(*f, handle);
                sq_.advance();
                continue;
            }
            Result<std::span<const std::uint8_t>> pkt = sq_.pkt(*f);
            Outcome a = pkt ? apply_frame(*f, *pkt, handle) : Outcome{.err = pkt.error().errnum};
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

    // The runtime's completion of an open or a close, found by its tag, the
    // descriptor (R4). Any other is dropped.
    template <class Handle> void complete(const Frame &f, Handle &handle) const noexcept {
        Fd fd = f.tag;
        if (f.cmd == static_cast<std::uint32_t>(abi::fs::Command::FS_OPENAT)) {
            std::uint32_t detail = 0;
            if (Result<std::span<const std::uint8_t>> pkt = sq_.pkt(f); pkt && !pkt->empty()) {
                if (Result<abi::fs::FsOpenatReply> r = abi::fs::FsOpenatReply::read(*pkt)) detail = r->detail;
            }
            (void)handle(Submission{FsOpened{.fd = fd, .res = f.result, .detail = detail}});
        } else if (f.cmd == static_cast<std::uint32_t>(abi::fs::Command::FS_CLOSE)) {
            std::erase_if(writes_, [fd](const FileWrite &w) { return w.fd == fd; });
            (void)handle(Submission{FsClosed{.fd = fd, .res = f.result}});
        }
    }

    // A packet's reader's answer handed over, or its error answered.
    template <class T, class Handle, class Wrap> static Outcome read_then(std::span<const std::uint8_t> pkt, Handle &handle, Wrap wrap) noexcept {
        Result<T> r = T::read(pkt);
        if (!r) return {.err = r.error().errnum};
        return handle(Submission{wrap(*r)});
    }

    template <class Handle> static Outcome apply_frame(const Frame &f, std::span<const std::uint8_t> pkt, Handle &handle) noexcept {
        if (f.cmd >> 8 >= frame::PROGRAM_MAJOR) return handle(Submission{Program{.cmd = f.cmd, .pkt = pkt}});
        using S = abi::sys::Command;
        using G = abi::gpu::Command;
        const auto is = [&f](auto c) { return f.cmd == static_cast<std::uint32_t>(c); };
        if (is(S::NOP)) {
            if (pkt.empty()) return {};
            Result<abi::sys::Nop> nop = abi::sys::Nop::read(pkt);
            if (!nop) return {.err = nop.error().errnum};
            return {.val = static_cast<std::int32_t>(nop->val)};
        }
        if (is(S::SYS_ATTACH)) return handle(Submission{Life::attach});
        if (is(S::SYS_SUSPEND)) return handle(Submission{Life::suspend});
        if (is(S::SYS_RESUME)) return handle(Submission{Life::resume});
        if (is(S::SYS_TURN)) return read_then<abi::sys::SysTurn>(pkt, handle, [](auto t) { return t; });
        if (is(G::GPU_READY)) return handle(Submission{GpuReady{}});
        if (is(G::GPU_SURFACE_OPEN)) return handle(Submission{Surface{SurfaceOpen{}}});
        if (is(G::GPU_SURFACE_RESIZE)) return read_then<abi::gpu::GpuSurfaceResize>(pkt, handle, [](auto c) { return Surface{c}; });
        if (is(G::GPU_SURFACE_CLOSE)) return read_then<abi::gpu::GpuSurfaceClose>(pkt, handle, [](auto c) { return Surface{c}; });
        if (is(abi::fs::Command::FS_CHANGED)) return read_then<abi::fs::FsChanged>(pkt, handle, [](auto c) { return c; });
        // The core's own report and file calls, which the runtime never sends,
        // and the numbers xpute gives nothing.
        return {.err = Errno::enosys};
    }

    void check_post(std::uint32_t cmd, Errno e) const noexcept {
        stat_.cq_peak = std::max(stat_.cq_peak, cq_.len());
        ensure(e == Errno::ok, Errno::enospc, cmd, static_cast<std::uint64_t>(static_cast<std::int64_t>(e)));
    }

    Ring sq_;
    Ring cq_;
    // What one command may leave: its imports, or the turn's after the drain, and its reply.
    std::uint32_t reserve_;
    // Kept by the const calls that move the rings, as the rings' own words are.
    mutable DoorStat stat_;
    // Kept until the FS_CLOSE of the descriptor its `write` named is completed.
    struct FileWrite {
        Fd fd;
        std::vector<std::uint8_t> bytes;
    };
    mutable std::vector<FileWrite> writes_;
    // Kept until its reading takes it, or until the runtime's head has passed
    // the `read_buf` that names it once its reading is destroyed.
    struct FileRead {
        Pos pos;
        Bytes buf;
        bool dropped;
    };
    mutable std::vector<FileRead> reads_;

    friend class Reading;
};

inline std::optional<Bytes> Reading::take(const Door &door) noexcept {
    if (!door.is_consumed(pos_)) return std::nullopt;
    std::optional<Bytes> bytes;
    std::erase_if(door.reads_, [&](Door::FileRead &r) {
        if (r.pos != pos_) return false;
        bytes = std::move(r.buf);
        return true;
    });
    door_ = nullptr;
    return bytes;
}

inline void Reading::drop() noexcept {
    if (door_ == nullptr) return;
    for (Door::FileRead &r : door_->reads_) {
        if (r.pos == pos_) r.dropped = true;
    }
    door_ = nullptr;
}

} // namespace xpute
