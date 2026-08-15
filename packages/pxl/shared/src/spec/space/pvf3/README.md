# PVF3: why packed add/sub isn't done as a single raw word op

`ivec`/`fvec` pack 3 lanes (X/Y/Z) into one BigInt word for cheap single-key
addressing and scaling (one comparable/hashable value, one shift-and-mask
projection step) — not to enable SIMD-style packed arithmetic. `add()`/`sub()`
in both modules unpack each lane, compute, and re-`gather()`. This note is the
record of why that's the right call here, not an oversight.

## The naive idea

Because the lanes are adjacent fields in one packed word, the obvious question
was whether `gather()`'s per-lane unpack/mask/repack could be skipped for
`add`/`sub`, doing one raw `(v + w) & WORD_MASK` on the whole packed word
instead. The current format deliberately has no guard bits: every bit in the
24/24/16 integer lanes and 10/10/10 fractional lanes belongs to data.

## What was tried, and why each attempt failed

Three variants were implemented and empirically verified (BigInt test
scripts, not just reasoned through):

1. **Plain packed add/sub** on an immediately adjacent layout
   (`Z_SHAMT=0`, `Y_SHAMT=Z_W`). Fails:
   a negative lane's own two's-complement sign-extension bit pattern produces
   a carry/borrow at that lane's own field boundary as a _normal_ part of
   representing the negative value — nothing to do with real overflow — and
   that bleeds straight into the next lane's LSB.
2. **Biased/offset encoding for subtraction** (bias cancels exactly for an
   isolated single-lane subtraction, unlike addition). Still fails: whenever
   a lane's own difference needs a borrow, that borrow corrupts the next lane
   up by exactly 1, regardless of biasing.
3. **Pre-primed "guard=1" borrow-firewall** (prime a guard bit to 1 in both
   operands before subtracting, matching a real bit-trick that does work —
   _given an actual dedicated slot_). Fails on the current layout for the
   same structural reason as (1): there is no dedicated slot to prime.

Common root cause, all three: **the layout has zero physical spacing between
adjacent lanes.** A carry/borrow leaving one field lands directly in the next.

## The wide-guard version (data-width guard, detect + correct)

The real SWAR technique for this — widen each lane's guard region to match
its own data width (not 1 bit; e.g. 8 guard + 8 data for an 8-bit lane),
sign-extend properly, then after the raw op inspect the guard bits to detect
which lanes borrowed/carried and add a correction back in. This is a real,
correct technique (confirmed empirically: a raw wide-guard op alone still
corrupts ~46% of signed test cases — the correction step is not optional,
it's load-bearing).

Two reasons it still doesn't pay off here:

- **Lane count.** SWAR wins by amortizing one detect+correct pass over many
  simultaneous lanes in a tight loop. We have exactly 3, fixed, asymmetric
  widths (24/24/16) — there's no loop to amortize over, and the
  detect+correct masking is already comparable in cost to the 3 scalar
  `X(v)±X(w)` subtractions it would replace.
- **No real register underneath.** SWAR's value proposition assumes the
  packed word is an actual hardware register, so one packed op really is one
  CPU instruction. `IVec`/`FVec` are `BigInt` — a software-emulated
  arbitrary-precision type in V8. A "single 64-bit BigInt add" has no
  guaranteed hardware-instruction advantage over 3 smaller BigInt ops; it may
  cost the same or more.
- **Width doesn't fit anyway.** Full data-width guard would need roughly
  double each lane's footprint (X/Y: 24→48b, Z: 16→32b), which no longer
  fits 3 lanes in one 64-bit word regardless of the above.

## Bottom line

Unpack → compute per-lane → `gather()` stays. Revisit only if profiling ever
shows `add()`/`sub()` as a real hot path _and_ the workload looks like bulk
parallel arrays (where real SIMD/WASM lanes would apply) rather than
one-off coordinate math — nothing calls `add()`/`sub()` that often today.
