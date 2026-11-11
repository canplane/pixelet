// cpp/xpute/core/io/stream.hpp

// A stream carries a turn's calls from the core to the runtime as records
// in one range. The core publishes the records at the end of the turn, and
// the runtime runs them all, so a record may name memory that the runtime
// writes and the core reads once the runtime has run it. Word END counts the
// record words past the head, and word HEAD the words before the first
// record, which a runtime reads from the stream itself; the user's own words
// follow them.
//
// END hands the records from the core to the runtime and back. It keeps
// these rules, whether the two sides take turns on one thread or run at once
// on two:
//
// - R1. END has one writer at a time: the core stores a count into it only
//   while it reads 0, and the runtime stores 0 into it only while it reads a
//   count.
// - R2. END is stored with release and loaded with acquire. The core writes
//   its records, then stores their count. The runtime loads the count before
//   it reads the records, and stores 0 once it is done with them. The core
//   loads END before it writes records again.
// - R3. The records, and any memory a record names for the runtime to read
//   (xpute gpu's WRITE_BUFFER_AT and WRITE_TEXTURE_AT, an image's bytes),
//   are the runtime's from the moment the count is stored until the core
//   loads END as 0. In between, nothing in the core rewrites or frees that
//   memory, and no copy of it is made. END can mark that moment because the
//   runtime's last read of the core's memory is when it runs a record, and
//   the write that runs it copies the bytes before it returns (wgpu's
//   write_buffer, write_texture). Code that writes such memory writes it
//   while the stream is the core's; otherwise it defers only itself, or
//   writes a second buffer the runtime does not hold. A runtime stores 0
//   whether or not it ran the records, so a runtime with nothing to run them
//   on never holds up the core.
// - R4. A word that a record names for the runtime to answer later has one
//   writer for good. The ticket of a request is the core's alone: a sequence
//   the core advances for that request's fence, wrapping, with one request
//   outstanding on the fence at a time. The fence is the runtime's alone:
//   once what the request guards is written, and after the result beside it
//   where the request can fail, the runtime stores there the ticket it
//   finished, with release. The core takes the request as done when the
//   fence, loaded with acquire, equals the ticket it asked with; a late
//   answer to a request that was dropped or superseded never does (xpute gpu
//   spec RESULT). A block's status word (the device's STATUS) is the
//   runtime's alone, stored with release after the block. No word is written
//   by both sides.
//
// A core that finds END not yet 0 writes no records (`free`), and the turn
// leaves what it would have recorded for a later turn.

#pragma once

#include "stream.spec.hpp"
#include <atomic>
#include <cstdint>
#include <optional>
#include <source_location>
#include <span>

#include "xpute/core/mem.hpp"

namespace xpute {

inline constexpr std::uint32_t WORD = sizeof(std::uint32_t);

// A record's words are its op and count, then `args`, then `bytes` as their
// count followed by the bytes in words.
constexpr std::uint64_t rec_words(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return 2 + args + (bytes ? 1 + (*bytes + WORD - 1) / WORD : 0);
}

// `rec_bytes` is a record's size in bytes, the unit a stream's room is
// counted in.
constexpr std::uint64_t rec_bytes(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return rec_words(args, bytes) * WORD;
}

class Stream {
  public:
    static constexpr std::uint32_t END = stream::END;
    static constexpr std::uint32_t HEAD = stream::HEAD;
    static constexpr std::uint32_t OWN_WORDS = stream::OWN_WORDS;

    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = OWN_WORDS) noexcept;

    std::uint32_t at() const noexcept { return at_; }

    Memory mem() const noexcept { return mem_; }

    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * WORD; }

    // `fits` says whether records of `bytes` bytes (`rec_bytes`, or
    // gpu/encode's `call::*_bytes`) fit with `leave` bytes left after them.
    bool fits(std::uint64_t bytes, std::uint32_t leave) const noexcept { return std::uint64_t{left()} >= bytes + leave; }

    // Opens a record of `n` words for its writer to fill, and answers where
    // the words begin. A record past the range, or one opened while the
    // runtime holds the stream (EBUSY), is a bug, reported at the caller.
    std::uint32_t open(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    // A record of `n` words for its writer to fill, or none when it no
    // longer fits in an open transaction, which is then dropped.
    std::optional<std::uint32_t> reserve(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt, std::source_location at = std::source_location::current()) noexcept;

    // Whether the runtime has stored END back to 0 since the last publish,
    // so that the stream is the core's to write (R1, R3).
    bool free() noexcept {
        if (!free_) free_ = end().load(std::memory_order_acquire) == 0;
        return free_;
    }

    // Hands the turn's records to the runtime (R2). A turn that recorded
    // none leaves END as it is.
    void publish() noexcept;

    // Begins a transaction: the records from here to `commit` go as one. A
    // record that does not fit drops the whole transaction, and every record
    // reserved after it until `commit` is refused. Outside a transaction, a
    // record that does not fit is a bug.
    void begin() noexcept { mark_ = {Mark::open, len_}; }

    // Answers false when the transaction did not fit, and the stream is then
    // as it was at `begin`.
    bool commit() noexcept {
        bool fit = mark_.state != Mark::spilled;
        if (!fit) len_ = mark_.at;
        mark_ = {};
        return fit;
    }

  private:
    // END, only ever touched as an atomic (R2).
    std::atomic_ref<std::uint32_t> end() const noexcept {
        return std::atomic_ref<std::uint32_t>(*reinterpret_cast<std::uint32_t *>(mem_.at(at_ + WORD * END)));
    }

    // Fills a record's words past its op and count, from word `w`: `args`,
    // `tail`, then `bytes` as their count followed by the bytes in words.
    void fill(std::uint32_t w, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
              std::optional<std::span<const std::uint8_t>> bytes) noexcept;

    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    std::uint32_t len_ = 0;
    // Whether the runtime has stored END back to 0 since the last publish,
    // as of the last load.
    bool free_ = true;
    // A mark says whether a transaction is open, where it began, and whether
    // a record that did not fit has dropped it.
    struct Mark {
        enum State { off, open, spilled } state = off;
        std::uint32_t at = 0;
    };
    Mark mark_;
};

} // namespace xpute
