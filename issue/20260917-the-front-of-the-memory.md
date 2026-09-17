# The front of the memory, and the one file its numbers come from

**2026-09-17** — After [the ring's rebuild](./20260917-the-ring-a-slot-and-its-payload.md)
the module's front held three things placed by three different parties and a
31.5 MiB hole nobody owned. This records what the front is now, why `.fixed` is
not `.bss`, and the drift the numbers were in before `memory.json`.

## The 31.5 MiB nobody was using

`MODULE_BYTES` was 64 MiB; the module's data ended at 32.49. The gap was that
size for a reason that had nothing to do with the module — the terrain began at
`REGION_BYTES`, so `MODULE_BYTES` was whatever kept it where it already was.

The linker knows where its data ends and says so. `__data_end` is taken
directly, the fixed ranges begin at the page past it, and the heap at the page
past them. The phone's heap goes 251.57 → 283.07 MiB, the desktop's 763.57 →
795.07, and the number stops being a figure someone has to keep true.

**Not `__heap_base`**, though it sits eight bytes further on and would do.
wasm-ld calls it that because a C runtime starts its `sbrk` there; this
kernel's heap is past 196 MiB of terrain, so borrowing the name would leave
`heap` meaning two things two hundred megabytes apart.

## The layout

```
addr                        bytes         what                 placed by
────────────────────────────────────────────────────────────────────────────
0            1,048,576       1.00 MiB     .stack               rustc
1,048,576    1,159,168      36 KiB × 3    .ring.desc           build.rs
2,097,152   27,262,976       8 MiB × 3    .ring.slot           build.rs
33,554,432  34,070,048       0.49 MiB     .rodata .data .bss   the linker
                                          ↑ __data_end
34,070,528 240,044,576     196.43 MiB     .fixed.terrain       the kernel
240,046,080       end      283.07 MiB     .heap  (phone)       the allocator
                           795.07 MiB     .heap  (desktop)
────────────────────────────────────────────────────────────────────────────
```

The dotted names are borrowed from ELF, and only three of them are ELF's own:
`.stack` and `.heap` are regions a runtime makes, not sections a linker emits.
Embedded linker scripts name output regions this way — `.dtcm`, `.ccmram`,
`._user_heap_stack` — and that is the reference class this project already
picked ([what the other WASM apps do](./20260916-what-the-other-wasm-apps-do-with-their-memory.md)).
The last column is why the notation needs watching: five placers, one dot.

**Rounded at four boundaries, asserted at three.** `.ring.desc`'s stride,
`.bss → .fixed`, between `.fixed`'s ranges and `.fixed → .heap` all go through
`page_up`, the one place the rule is written. `RING_DESC_AT`, `RING_SLOT_AT`
and `GLOBAL_BASE` are chosen constants, so what a rounding would have
guaranteed is a `const assert` instead.

## Why `.fixed` is not `.bss`

A 196 MiB `static [u8; N]` would land in the same place and work. Both are a
size known at compile time, zeroed, never freed, reached by index arithmetic.

What it would lose is that the size is _decided_. `.bss` is what the compiler
happens to put there; a range this large is a choice about how much world is
resident, and the reasoning for it should not have to be recovered from a
linker map. One mechanical difference goes with it: statics count toward the
memory the module declares, so the terrain in `.bss` would raise the declared
minimum to 230 MiB by itself. That turns out to be worth having — see below —
but it is worth having for the whole module, not as a side effect of where one
array was put.

`.fixed` is a group, and which ranges are in it is `memory/fixed.rs`'s, not the
map's. Adding one is a line in that file.

## Three copies of the reservation, compared never

`core_memory.ts` picked 512 or 1024 MiB. `memory.rs` wrote the same two numbers
down beside a comment saying they are "not read here … but written down so the
two sides say the same thing". Written down, and nothing checked.

So a shell that asked for too little instantiated fine and the kernel asserted
its way out of it a moment later — past the point where the mistake could be
named. The page was written twice as well, `PAGE_BYTES` in the map and `PAGE`
in build.rs.

`memory.json` is where all of it is decided now, and `script/spec.ts` writes
both sides: `memory_spec.rs` for the map, `memory_spec.ts` for the shell, and
build.rs takes the same generated Rust by `#[path]` rather than a copy.
`deno task check:spec` fails where any of them drifts. Ten numbers, three
consumers, nothing hardcoded in any of them.

## The module now says what it needs

With `--import-memory` wasm-ld had been declaring a minimum of whatever its own
statics came to:

```
import env.memory: min 520 pages = 32.50 MiB        <- true of the data,
                                                      a lie about the module
```

The module cannot run without 196 MiB of terrain. `--initial-memory=PHONE_BYTES`
declares the floor, and the engine checks it when the shell hands over a memory:

```
import env.memory: min 8192 pages = 536,870,912 B
```

A memory too small is now a `LinkError` at instantiation, before any Rust runs,
which is where io_uring, virtio and NVMe all put the same check — the device
states its requirement and the host either meets it or the attachment fails.
The kernel's own assert stays as a belt.

Two failure points become one contract and one belt:

|                                    | before                      | now                                              |
| ---------------------------------- | --------------------------- | ------------------------------------------------ |
| device will not reserve            | `reserve_memory` → ENOMEM   | unchanged                                        |
| shell asks for less than needed    | kernel asserts at boot      | LinkError at instantiation                       |
| module's data outgrows the ceiling | —                           | boot assert against `MODULE_CEILING`             |
| terrain outgrows a phone           | `const assert` on `HEAP_AT` | `const assert` on `MODULE_CEILING + FIXED_BYTES` |

## The slab that was not one

`74b561b` named the terrain range a slab, because its records are addressed by
arithmetic rather than handed out; `983c686` put `ds/slab.rs` in one file away,
where allocation is the whole point — and a third `sys/slab.ts` is the central
scheduler's transferable memory. The terrain's own doc-comment already named
the right precedent: "the way a clipmap addresses its ring".

It is `.fixed.terrain` now, in `terrain/fixed.rs`. `slab` keeps the allocator
and the ray/AABB slab test.

## What this leaves open

- `.stack` is 1 MiB because rustc passes `-z stack-size=1048576`; `memory.json`
  restates it to place the rings against it, and nothing checks the two agree.
  A stack grown by a future flag would run into `.ring.desc` silently.
- `ds/arena.rs`'s `Arena` and `ArenaGuard` are imported by nobody. Left alone.
- Nothing was run. `cargo clippy -D warnings` and `deno task check`, and no
  more than that.

## The stack, from mem.json (later the same day)

The first item above is closed. build.rs passes `--stack-first` and
`-z stack-size` from mem.json's `STACK_BYTES`, so the stack is a number this
project chooses rather than one rustc's default happens to match, and
`lay_out` asserts at boot that wasm-ld's `__stack_high` is `STACK_BYTES`. The
layout above is otherwise unchanged; `.stack` is placed by build.rs, not by
rustc.

Checked with `cargo clippy` for wasm32, which does not link: that wasm-ld
defines `__stack_high`, and that a `-z stack-size` given after rustc's own
wins, are what the next linked build shows.
