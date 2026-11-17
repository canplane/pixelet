---
name: code
description: How code in pixelet and xpute is written — fewest concepts, orthogonal, one hand's style throughout, as UNIX V6 and K&R are written. Use before writing or rewriting code, and when reviewing one's shape. Names are the naming skill's.
---

# Code

- Reduce first. A concept that is another in disguise (a reply is a frame, an
  abort is an import) is that other. Code is short because its concepts are few.
- Orthogonal. A concept says one thing; changing it reinterprets no other.
- One owner. A fact has one definition; every other form derives from it. A
  fact held in several places is reviewed, and stays apart only where the CPU
  and the GPU each hold it or joining it would add branches. Where they each
  hold it, the CPU's side stays even when no Rust reads it: a shader ports it, a
  test compares against it, or it defines what a buffer holds. An orphan is
  removed only once no caller, port, test or layout reaches it, and never from
  the kit or another published API.
- Before changing a structure, read the documents and the code it lives in,
  and its history: `git log` and `git blame` the code, the commit that made it
  so and when, and what the user decided there. State which of its values are
  independent and which derive or repeat. The change follows from who owns the
  fact, as the design and those decisions place it; its size follows from
  that. Where the design leaves the shape open, take the convention of those
  who solved the same problem, naming it, and first set its premises beside
  this system's: a convention holds only as far as the premises it rests on do
  (a rasterizer's rule is not a ray caster's).
- Layers, as a system is built. A part depends only on what is below it: the
  part below gives the mechanism and declares the interface it needs; the part
  above implements it, sets the policy and injects it; the program's assembly,
  the top, makes the pieces. A part never reaches up for a concrete thing, a
  global or a function, of the part that runs it, and no policy (what to print,
  a limit to measure by) is carried down into a mechanism. The orders `rule.rs`
  (`rust/tool/src/bin/rule.rs`) holds change only on the user's word.
- What goes wrong is an error or a bug, and what is worth telling and is
  neither is a log record. An error is an errno, the place it was made and an
  optional message, as a kernel's is: across a boundary its status (a frame's
  RESULT), inside a language the error object (`XputeError`, C++'s
  `xpute::Error`), returned in Rust and C++ and thrown in TypeScript, for the
  caller to judge. A bug is a broken invariant: the same, at most two numbers,
  handed to the report installed from above, and the part stops; a panic the
  language raises is one too. A log record is a level, a context and a message,
  handed to the log the part's environment gives it. No part writes to the
  platform's output; that is the environment's alone, as a shell prints what a
  kernel returns.
- What depends on a platform lives in the one part that is the platform's
  (xpute's runtime, pixelet's `service/platform/`); every other part is
  portable, using no platform API beyond its language's standard library.
- Data first. Structures and tables, then short straight functions over them.
  Where a branch picks by kind, a table picks.
- Read in the order it runs. No step behind a wrapper or a hook.
- No optional seams: no hook that may be absent, no fallback for a value one
  place could give, no layer split out for a test.
- One hand. The same work in the same shape, the same names for the same
  things. Rewrite a part rather than lay a new manner beside or over the old
  (a second form, an alias, a flag, a shim).
- Keep a symmetry by role and contract. xpute's Rust, C++ and TypeScript are
  also laid out alike, where a part, its file and module names, what a module
  holds and the shape of generated code are concerned, and differ only where
  the language makes them (its casing, a template for inlined code, what it
  already gives). A local gain alone does not weaken one. A contract changed
  on purpose is named as such, with what keeps or replaces what it held.
- Style, as indentation is, not a rule a review holds: pixelet's TypeScript
  tends to write a fixed-width value (a field crossing the link, a stored or a
  GPU word) with the kit's word alias (`u32`, `i32`, `f32`), and any other
  number as `number` or `bigint`.
- A comment holds what the code cannot say: the contract, the reason, the
  invariant. It reads on its own in whole sentences, never says how the code
  came to be and never cites a minute under `doc/`. As style, not a rule a
  review holds, it puts an identifier in backticks and writes a word as prose
  (UI, GPU).
