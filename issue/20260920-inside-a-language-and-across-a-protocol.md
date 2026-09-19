# Inside a language, and across a protocol

**2026-09-20** — The Rust migration held the two sides in lockstep: every Rust
module was read against the TypeScript it came from, and the golden records the
TypeScript had written were the proof that the port had not drifted. That was
right while the port was in flight. It is over now, and holding every value in
both languages to the same bits is no longer what the project wants — it costs
Rust the right to be Rust. This records the line that replaces it, what it made
legitimate, and the half of it that nothing checks yet.

## The rule

**Inside a language, each follows its own. Across a protocol, the results must
match exactly.**

A value that never leaves the side that computed it is that side's business.
Rust rounds through `libm` and the standard library; JavaScript rounds through
`Math`. Where they differ neither is wrong, and forcing one to imitate the
other means hand-writing what the platform already gives — which is the policy
the project has retired.

A value that crosses is not. XTP's packets and TLV's bytes are read by the side
that did not write them, so both sides have to produce the same bytes from the
same input, to the last one. The same holds for anything else whose meaning is
its number: the errno table, the command numbers, the frame's four words.

The line is not "Rust is the source of truth". It is that the protocol is the
contract and the implementations are free everywhere else.

## What it made legitimate

Two changes landed under it, both inside a language:

- **`round` is half to even.** It had been half up — JavaScript's `Math.round`,
  ported by hand. To even is IEEE 754's and WGSL's, and it is unbiased, which
  is what a kit meant to outlive this project should be. Fifteen files take it,
  `geo-bake` among them, so a page baked before and after can differ where a
  coordinate lands exactly on a boundary; the pages are re-baked rather than
  the rounding kept.
- **`hypot` is `libm`'s.** The hand-written n-ary one is gone. `libm` has only
  the binary form, so a three-dimensional length is now `hypot(hypot(x, y), z)`
  — one more rounding than `Math.hypot(x, y, z)` does in one pass, and a last
  bit that differs.

Both moved values that stay on the Rust side, so the records that held them to
the TypeScript were re-recorded rather than the code bent back: **32 lines over
three files**, every one of them traceable to one of those two changes.

- `spec/space/v3.tsv` — 9: `norm3` (hypot), `round3n` (rounding)
- `math/mat4.tsv` — 16: `look_at` (hypot), and `direction` and `invert`, which
  carry its last bit into 3e-12 through an inversion
- `math/scalar.tsv` — 7: `round_to_step` (rounding)

Nothing else differed. The list was taken by making the assertions print
instead of fail, so that a real regression could not ride along with them.

## The two kinds of record

There are 21 golden files. They fall in two groups and only one of them may
ever be re-recorded:

|                       | Holds                               | Files                                  |
| --------------------- | ----------------------------------- | -------------------------------------- |
| **Across a protocol** | both languages, to the byte         | `wire/xtp/{cursor,encode}`, `wire/tlv` |
| **Inside a language** | that implementation, to what it did | the other 18                           |

`golden.rs` says this now, and the test names follow it: the three that cross
keep `..._what_the_typescript_...`, because that is still what they claim,
and the other **19 were renamed to `..._what_the_record_holds`**, because
what they claim is that this implementation has not drifted from itself.

`pxl-spec` is entirely in the second group and always will be — it has no
TypeScript twin at all. The shell reads a page through the kernel rather than
decoding one, so the bake's formats have exactly one implementation.

## Standing the other half up

The protocol half was a claim and not a check: **no TypeScript test read a
golden**, so all 21 records only held the Rust to itself. The records had
agreed with the TypeScript on the day they were written, and if the TypeScript
had moved since, nothing would have said so.

Three tests now close it, reading the same three files the Rust reads:

- `wire/tlv.test.ts`
- `wire/xtp/cursor.test.ts`
- `wire/xtp/encode.test.ts`

They needed a reader, so `@xpute/core/golden.ts` is the pair of `golden.rs`:
same line format, same `each`/`len`/`f64`-through-bits. It reads the `.tsv`
out of the Rust crate rather than keeping a copy, because a second copy is how
two sides come to disagree without anything failing. Its own header says which
records this side may read at all — the crossing ones, and not the rest.

All three passed on the first run, which is the answer to the question that
prompted them: the two have not drifted. That they _would_ catch a drift was
checked by making one of them write a 43 where the record holds a 42; it fails.

One thing turned up while classifying: **TLV has no caller in either
language.** It is a codec the kit carries for a use that has not arrived, and
the test above is now the only thing exercising it. That is a separate
question from this one, and nothing was done about it here.

## Where it stands

The rule is written into `golden.rs`, the names follow it, the 32 lines are
re-recorded, and the three crossing records are now read from both sides.
`deno task check`, `test` and `lint` are green. The re-bake has not been run;
two changes of the same day call for it — this rounding, and the POI page's
`id_hash`, which is FNV-1a over UTF-8 now
([20260919](./20260919-the-pair-audited-what-left.md)).
