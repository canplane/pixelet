// cpp/xpute/gpu/stream.hpp

// A stream carries the guest's GPU calls to the host as records, as
// xpute-gpu-guest's stream.rs carries them: op, n, then n words; bytes go as
// their count, then the bytes padded to a word (gpu/link/spec/api.json). The
// guest writes the records into a buffer it owns and flushes them by lending
// the buffer to the host's `gpu.run` (gpu/wit/gpu.wit), which answers once
// they have run. From the flush to the answer the records, and any memory a
// record names for the host to read (the bytes a buffer's or an image's
// upload takes), are the host's: nothing in the guest rewrites or frees that
// memory, and no copy of it is made. The host's last read of the guest's
// memory is when it runs a record, and the write that runs it copies the
// bytes before it returns, as WebGPU's `writeBuffer` and `writeTexture` do. A
// host answers whether or not it ran the records, so one with nothing to run
// them on never holds up the guest.
//
// A word that a record names for the host to answer later has one writer
// for good. The ticket of a request is the guest's alone: a sequence the
// guest advances for that request's fence, wrapping, with one request
// outstanding on the fence at a time. The fence is the host's alone: once
// what the request guards is written, and after the result beside it where
// the request can fail, the host stores there the ticket it finished, with
// release. The guest takes the request as done when the fence, loaded with
// acquire, equals the ticket it asked with; a late answer to a request that
// was dropped or superseded never does. No word is written by both sides.
//
// While a run is out the stream is not free (`is_free`), and the guest
// writes no records: what it would have recorded waits for a later frame.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <source_location>
#include <span>
#include <utility>
#include <variant>
#include <vector>

#include "xpute/kit/status/error.hpp"
#include "xpute/link/wait.hpp"

namespace xpute::gpu {

inline constexpr std::size_t WORD = sizeof(std::uint32_t);

// A record's words are its op and count, then `args`, then `bytes` as their
// count followed by the bytes in words.
constexpr std::size_t rec_words(std::size_t args, std::optional<std::size_t> bytes) noexcept {
    return 2 + args + (bytes ? 1 + (*bytes + WORD - 1) / WORD : 0);
}

class Stream {
  public:
    // A stream of `room` words.
    explicit Stream(std::size_t room);

    std::size_t left() const noexcept { return (room_ - len()) * WORD; }

    // Whether no run holds the stream, so that it is the guest's to write.
    bool is_free();

    // Hands the records to the host's `gpu` by lending them to its `run`
    // (gpu/wit/gpu.wit), called by its named handle. A stream that recorded
    // none, or that a run still holds, sends nothing.
    void flush();

    // `fits` says whether records of `bytes` bytes fit with
    // `leave` bytes left after them.
    bool fits(std::size_t bytes, std::size_t leave) const noexcept { return left() >= bytes + leave; }

    // Opens a record of `n` words for its writer to fill. A record past the
    // room, or one opened while a run holds the stream (EBUSY), is a bug,
    // reported at the caller, which names the record.
    std::span<std::uint32_t> open(std::uint32_t op, std::size_t n, std::source_location at = std::source_location::current());

    // Begins a transaction: the records from here to `commit` go as one. A
    // record that does not fit drops the whole transaction, and every record
    // reserved after it until `commit` is refused. Outside a transaction, a
    // record that does not fit is a bug.
    void begin() noexcept { mark_ = Mark{Mark::open, len()}; }

    // Answers false when the transaction did not fit, and the stream is then
    // as it was at `begin`.
    bool commit() noexcept;

    // A record of `n` words for its writer to fill, or none when it no
    // longer fits in an open transaction, which is then dropped.
    std::optional<std::span<std::uint32_t>> reserve(std::uint32_t op, std::size_t n, std::source_location at = std::source_location::current());

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt, std::source_location at = std::source_location::current());

  private:
    // A run of the stream's records, out to the host's `gpu.run`, which
    // gives the buffer back with its answer.
    using Run = link::Task<std::pair<std::vector<std::uint32_t>, Result<void>>>;

    // The words recorded since the last flush.
    std::size_t len() const noexcept;

    // Who holds the stream's buffer: the guest, with the records recorded
    // since the last flush, or a run.
    std::variant<std::vector<std::uint32_t>, Run> buf_;
    std::size_t room_;
    // A mark says whether a transaction is open, where it began, and whether
    // a record that did not fit has dropped it.
    struct Mark {
        enum State { off, open, spilled } state = off;
        std::size_t at = 0;
    };
    Mark mark_;
};

} // namespace xpute::gpu
