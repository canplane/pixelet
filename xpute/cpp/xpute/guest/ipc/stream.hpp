// cpp/xpute/guest/ipc/stream.hpp

// A turn's calls from guest to host as records in one range, published at
// the turn's end and run whole by the host, so a record may name memory the
// host writes and the guest reads once the host has run it. Word END counts
// the record words past the head, word HEAD the words before the first
// record, which a host reads from the stream itself; the user's words follow
// them.
//
// END hands the records from guest to host and back, by the rules a ring's
// position words keep (ring.hpp), whether the two take turns on one thread or
// run at once on two:
//
// - R1. END has one writer at a time: the guest stores a count into it only
//   while it reads 0, and the host stores 0 into it only while it reads a
//   count.
// - R2. END is stored with release and loaded with acquire. The guest writes
//   its records, then stores their count; the host loads the count before it
//   reads them, and stores 0 once it is done with them; the guest loads END
//   before it writes records again.
// - R3. The records, and the memory a record names for the host to read
//   (xpute gpu's WRITE_BUFFER_AT, WRITE_TEXTURE_AT, an image's bytes), are the
//   host's from the count's store until the guest loads END as 0. In between
//   nothing in the guest rewrites or frees that memory, and no copy of it is
//   made. END is that point because the host's last read of the guest's
//   memory is a record's run, whose write copies the bytes before it returns
//   (wgpu's write_buffer, write_texture). A writer of such memory writes it
//   while the stream is the guest's, and otherwise defers itself alone, or
//   writes a second buffer the host does not hold. A host stores 0 whether or
//   not it ran them, so a host with nothing to run them on never holds the
//   guest.
// - R4. A word a record names for the host to answer later has one writer for
//   good. A completion's ticket is the guest's alone, a sequence it advances
//   for its fence, wrapping, one request out on the word at a
//   time; the fence is the host's alone, which stores there the ticket it
//   finished, with release, once what the request guards is written and after
//   the result beside it where the request can fail. The guest holds the
//   request done where the fence, loaded with acquire, equals the ticket
//   it asked with: a late answer to a request dropped or superseded never
//   does (xpute gpu spec RESULT). A block's status (the device's STATUS) is the
//   host's alone, stored with release after the block. No word is written by
//   both.
//
// A guest that finds END not yet 0 writes no records (`free`): the turn
// leaves what it would have recorded for a later one.

#pragma once

#include <atomic>
#include <cstdint>
#include <optional>
#include <source_location>
#include <span>

#include "xpute/guest/mem.hpp"

namespace xpute {

inline constexpr std::uint32_t WORD = sizeof(std::uint32_t);

// A record's words: its op and count, `args`, and `bytes` as their count then words.
constexpr std::uint64_t record_words(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return 2 + args + (bytes ? 1 + (*bytes + WORD - 1) / WORD : 0);
}

// A record's size as a stream's room is counted.
constexpr std::uint64_t record_bytes(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return record_words(args, bytes) * WORD;
}

class Stream {
  public:
    static constexpr std::uint32_t END = 0;
    static constexpr std::uint32_t HEAD = 1;
    // The stream's own words, before the user's.
    static constexpr std::uint32_t OWN_WORDS = 2;

    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = OWN_WORDS) noexcept;

    std::uint32_t head_at() const noexcept { return at_; }

    Memory mem() const noexcept { return mem_; }

    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * WORD; }

    // Whether records of `bytes` (`record_bytes`, gpu/encode's `call::*_bytes`)
    // fit with `leave` bytes left after them.
    bool fits(std::uint64_t bytes, std::uint32_t leave) const noexcept { return std::uint64_t{left()} >= bytes + leave; }

    // Answers where the `n` words begin. Past the range, or while the host
    // holds the stream (EBUSY), is a bug, reported at the caller.
    std::uint32_t open(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    // A record of `n` words for its writer to fill; none where an open
    // transaction no longer fits it, which drops the transaction.
    std::optional<std::uint32_t> reserve(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt, std::source_location at = std::source_location::current()) noexcept;

    // Whether the host has stored END back to 0 since the last publish: the
    // stream is the guest's to write (R1, R3).
    bool free() noexcept {
        if (!free_) free_ = end().load(std::memory_order_acquire) == 0;
        return free_;
    }

    // The turn's records handed to the host (R2); a turn that recorded none
    // leaves END as it is.
    void publish() noexcept;

    // The records from here to `commit` are one: one that does not fit drops
    // them all, and those after it, where outside a transaction it is a bug.
    void begin() noexcept { mark_ = {Mark::open, len_}; }

    // False where the transaction did not fit: the stream is as at `begin`.
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

    // A record's words past its op and count, from `w`: `args`, `tail`, and
    // `bytes` as their count then words.
    void fill(std::uint32_t w, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
              std::optional<std::span<const std::uint8_t>> bytes) noexcept;

    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    std::uint32_t len_ = 0;
    // The host stored END back to 0 since the last publish, as last loaded.
    bool free_ = true;
    // Whether a transaction is open, where it began, and whether a record
    // that did not fit has dropped it.
    struct Mark {
        enum State { off, open, spilled } state = off;
        std::uint32_t at = 0;
    };
    Mark mark_;
};

} // namespace xpute
