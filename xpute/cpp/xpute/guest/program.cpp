// cpp/xpute/guest/program.cpp

#include "program.hpp"

#include <algorithm>
#include <array>
#include <cstddef>
#include <vector>

#include "xpute/guest/abi/cmd.spec.hpp"
#include "xpute/guest/clock.hpp"
#include "xpute/guest/hostcall.hpp"
#include "xpute/guest/ipc/file.hpp"
#include "xpute/guest/ipc/toc.hpp"
#include "xpute/guest/mem.hpp"
#include "xpute/guest/sched/wake.hpp"
#include "xpute/guest/stop.hpp"

namespace xpute {

namespace program_detail {

// The executor `boot` installs: each task resumed once a turn while it is
// woken, a woken one asking for the next turn. Another installed in its place
// (`set_executor`) runs the same tasks.
class Tasks final : public Executor {
  public:
    bool spawn(std::coroutine_handle<> task, Priority) noexcept override {
        woken_.push_back(task);
        return true;
    }

    void wake(std::coroutine_handle<> task) noexcept override {
        if (std::find(woken_.begin(), woken_.end(), task) == woken_.end()) woken_.push_back(task);
    }

    void poll() noexcept override {
        std::vector<std::coroutine_handle<>> now = std::exchange(woken_, {});
        for (std::coroutine_handle<> h : now) h.resume();
    }

    bool pending() const noexcept override { return !woken_.empty(); }

  private:
    std::vector<std::coroutine_handle<>> woken_;
};

// A read's buffer the host may still fill after its `Read` was destroyed: let
// go once the turn `asked_turn` has passed.
struct Left {
    Bytes bytes;
    std::uint32_t asked_turn;
};

struct Program {
    Door door;
    Sys sys;
    Files<Read *, program::MOST_FILES> files{};
    // The reads the host fills after the turn each asked in.
    std::vector<Read *> reading{};
    std::vector<Left> left{};
    // Turns run since boot.
    std::uint32_t turn = 0;
    Tasks tasks{};

    // An open answered: an errno ends its read, a size has the host fill a
    // buffer of it after this turn.
    void complete_open(const abi::SysOpened &opened) noexcept {
        std::optional<Read *> owner = files.complete_open(sys, opened);
        if (!owner) return;
        Read &r = **owner;
        if (opened.res < 0) {
            r.ready(errno_of(opened.res).value_or(Errno::eio));
            return;
        }
        // Kept in `r`, or in `left` once it is gone, until a later turn's rise.
        Result<Bytes> bytes = files.read_whole(sys, opened.fd, static_cast<std::uint32_t>(opened.res));
        if (!bytes) {
            r.ready(bytes.error().errnum);
            return;
        }
        r.bytes_ = std::move(*bytes);
        r.got_ = Read::Got::reading;
        r.asked_ = turn;
        reading.push_back(&r);
    }

    // The reads the host filled after an earlier turn, ready; the buffers of
    // those destroyed meanwhile, let go.
    void rise() noexcept {
        turn++;
        std::erase_if(left, [this](const Left &l) { return turn > l.asked_turn; });
        std::erase_if(reading, [this](Read *r) {
            if (turn <= r->asked_) return false;
            r->ready(Errno::ok);
            return true;
        });
    }

    // A read waits for the next turn.
    bool waiting() const noexcept { return !reading.empty(); }
};

namespace {
// Raw storage rather than a static `Program`, so nothing runs before the
// module's first call or after its last.
alignas(Program) std::byte program_storage[sizeof(Program)];
Program *booted_program = nullptr;
} // namespace

const Door *door() noexcept {
    return booted_program != nullptr ? &booted_program->door : nullptr;
}

} // namespace program_detail

using program_detail::booted_program;
using program_detail::Program;

std::uint32_t boot(const Layout &layout, std::uint32_t data_end) noexcept {
    Memory mem = mem::core();
    std::uint32_t range = range_at(data_end);
    auto at = [range](std::uint32_t off) { return range + off; };
    Ring sq = Ring::init(mem, at(layout.sq_at()), layout.cap, at(layout.sq_slots_at()), layout.slot_bytes);
    Ring cq = Ring::init(mem, at(layout.cq_at()), layout.cap, at(layout.cq_slots_at()), layout.slot_bytes);
    const std::array<Entry, 3> entries{{
        {toc::key::SQ, at(layout.sq_at())},
        {toc::key::CQ, at(layout.cq_at())},
        {toc::key::SYS, at(layout.sys_at())},
    }};
    write_toc(mem, range, toc_words(entries.size()), entries);
    if (booted_program != nullptr) booted_program->~Program();
    booted_program = new (program_detail::program_storage) Program{
        .door = Door(sq, cq, layout.most_events),
        .sys = Sys(mem, at(layout.sys_at()), layout.sys_words),
    };
    set_executor(booted_program->tasks);
    // A stop published as the turn's one record.
    stop::install([](Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) {
        if (booted_program != nullptr) booted_program->sys.abort(errnum, at, a, b);
    });
    return range;
}

Result<xtp::BranchCursor> Packet::fields() const noexcept {
    std::optional<std::span<const std::uint8_t>> pkt = bytes();
    if (!pkt) return marshal_error(Errno::ebadmsg);
    Result<xtp::TreeReader> t = xtp::TreeReader::make(*pkt);
    if (!t) return t.error();
    return t->read_branch();
}

Result<double> Packet::number(std::uint32_t i) const noexcept {
    Result<xtp::BranchCursor> f = fields();
    if (!f) return f.error();
    Result<xtp::Cursor> c = f->at(i);
    if (!c) return c.error();
    return c->get_number();
}

Result<std::string_view> Packet::str(std::uint32_t i) const noexcept {
    Result<xtp::BranchCursor> f = fields();
    if (!f) return f.error();
    Result<xtp::Cursor> c = f->at(i);
    if (!c) return c.error();
    return c->get_str_ref();
}

Result<void> Turn::spawn(Task<> work) const noexcept {
    if (::xpute::spawn(std::move(work), 0)) return {};
    return make_error(ErrorClass::fault, Errno::eagain);
}

Read::Read(std::uint32_t root, std::string_view path) noexcept : root_(root) {
    std::optional<Bytes> copy = Bytes::make(static_cast<std::uint32_t>(path.size()));
    if (!copy) {
        ready(Errno::enomem);
        return;
    }
    path_ = std::move(*copy);
    std::copy(path.begin(), path.end(), path_.data());
}

Read::Read(Read &&o) noexcept : root_(o.root_), path_(std::move(o.path_)), got_(o.got_), err_(o.err_), bytes_(std::move(o.bytes_)) {
    // Awaited, it is named by its open and its waker.
    ensure(o.got_ == Got::idle || o.got_ == Got::done, Errno::einval, static_cast<std::uint64_t>(o.got_));
    o.got_ = Got::idle;
}

bool Read::open(Waker waker) noexcept {
    if (booted_program == nullptr) {
        ready(Errno::enxio);
        return false;
    }
    Program &p = *booted_program;
    std::string_view path(reinterpret_cast<const char *>(path_.data()), path_.size());
    std::optional<Fd> fd = p.files.try_open(p.sys, root_, path, this);
    if (!fd) {
        ready(Errno::emfile);
        return false;
    }
    fd_ = *fd;
    got_ = Got::opening;
    waker_ = waker;
    return true;
}

void Read::ready(Errno err) noexcept {
    got_ = Got::done;
    err_ = err;
    std::exchange(waker_, {}).wake();
}

Result<Bytes> Read::await_resume() noexcept {
    if (err_ != Errno::ok) return make_error(ErrorClass::fault, err_);
    return std::move(bytes_);
}

Read::~Read() {
    if (booted_program == nullptr) return;
    Program &p = *booted_program;
    switch (got_) {
    case Got::opening:
        p.files.close(p.sys, fd_);
        break;
    case Got::reading:
        std::erase(p.reading, this);
        p.left.push_back(program_detail::Left{std::move(bytes_), asked_});
        break;
    case Got::idle:
    case Got::done:
        break;
    }
}

double turn(std::span<const On> handlers) noexcept {
    if (booted_program == nullptr) return NO_WAKE;
    Program &p = *booted_program;
    p.sys.rise();
    p.rise();
    p.door.drain(
        [&p](const abi::SysOpened &opened) {
            p.complete_open(opened);
            return Outcome{};
        },
        [&p, handlers](std::uint32_t cmd, std::span<const std::uint8_t> pkt) {
            for (const On &on : handlers) {
                if (on.cmd != cmd) continue;
                Turn cx(p.sys, p.door);
                Result<std::int32_t> res = on.handler(cx, Packet(pkt));
                return res ? Outcome{.val = *res} : Outcome{.err = res.error().errnum};
            }
            return Outcome{.err = Errno::enosys};
        });
    // The tasks reach the program through `booted_program` themselves.
    ::xpute::poll();
    p.sys.fall();
    return p.door.pending() || p.waiting() || ::xpute::pending() ? 0.0 : NO_WAKE;
}

#ifdef __wasm__

std::uint32_t boot_module(const Layout &layout, std::uint32_t stack_bytes) noexcept {
    // The stack the module was linked with is the one the program declares.
    ensure(wasm::stack_high() == stack_bytes, Errno::enotrecoverable, wasm::stack_high());
    set_hostcall(wasm::hostcall);
    set_clock(wasm::now);
    return boot(layout, wasm::data_end());
}

#endif

} // namespace xpute
