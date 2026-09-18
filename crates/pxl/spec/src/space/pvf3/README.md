# PVF3: why packed add/sub isn't done as a single raw word op

`ivec`/`fvec` pack 3 lanes (X/Y/Z) into one `u64` for cheap single-key
addressing and scaling — one comparable, hashable value and one
shift-and-mask projection step — not to enable SIMD-style packed arithmetic.
`add()`/`sub()` in both modules unpack each lane, compute, and re-`gather()`.
This note is the record of why that is the right call here, not an oversight.

## The naive idea

Because the lanes are adjacent fields in one packed word, the obvious question
is whether `gather()`'s per-lane unpack/mask/repack could be skipped for
`add`/`sub`, doing one raw `(v + w) & WORD_MASK` on the whole word instead.
The format deliberately has no guard bits: every bit in the 24/24/16 integer
lanes and the 8/8/8 fractional lanes (`FRAC_W`) belongs to data.

And unlike the TypeScript this was first written against, the packed word here
is a machine register, so one packed op really would be one instruction. That
removes the cheapest objection and leaves the structural ones, which are the
whole of the argument below.

## What was tried, and why each attempt failed

Three variants were implemented and empirically checked rather than reasoned
through:

1. **Plain packed add/sub** on an immediately adjacent layout (`Z_SHAMT = 0`,
   `Y_SHAMT = Z_W`). Fails: a negative lane's own two's-complement sign
   extension produces a carry/borrow at that lane's field boundary as a
   _normal_ part of representing the negative value — nothing to do with real
   overflow — and it bleeds straight into the next lane's LSB.
2. **Biased/offset encoding for subtraction**, where the bias cancels exactly
   for an isolated single-lane subtraction as it does not for addition. Still
   fails: whenever a lane's own difference needs a borrow, that borrow
   corrupts the next lane up by exactly 1, whatever the biasing.
3. **Pre-primed "guard = 1" borrow firewall** — prime a guard bit to 1 in both
   operands before subtracting, which is a real trick that does work _given an
   actual dedicated slot_. Fails here for the same structural reason as (1):
   there is no slot to prime.

One root cause under all three: **the layout has zero physical spacing between
adjacent lanes.** A carry or borrow leaving one field lands directly in the
next.

## The wide-guard version

The real SWAR technique for this widens each lane's guard region to match its
own data width — not one bit; eight guard bits for an eight-bit lane — sign
extends properly, then inspects the guard bits after the raw op to detect
which lanes borrowed or carried and adds a correction back in. It is correct,
and the correction step is load-bearing rather than a refinement: a raw
wide-guard op on its own still corrupts a large fraction of signed cases.

Two reasons it does not pay off here, and neither is about the language:

- **Lane count.** SWAR wins by amortizing one detect-and-correct pass over
  many lanes in a tight loop. There are exactly 3 here, fixed, at asymmetric
  widths (24/24/16). There is no loop to amortize over, and the
  detect-and-correct masking already costs about what the three scalar
  `X(v) ± X(w)` operations it would replace cost.
- **The width does not fit.** A full data-width guard needs roughly double
  each lane's footprint — X/Y 24 → 48 bits, Z 16 → 32 — which no longer fits
  three lanes in a 64-bit word at all.

## Bottom line

Unpack, compute per lane, `gather()`. Revisit only if profiling ever shows
`add()`/`sub()` as a real hot path **and** the workload looks like bulk
parallel arrays, where actual SIMD lanes would apply, rather than one-off
coordinate math — nothing calls `add()`/`sub()` that often today.
