# rustc_codegen_spirv, as the shader build takes it

rust-gpu's codegen, [`rustc_codegen_spirv`][upstream] **0.10.0-alpha.1** as
published on crates.io (sha256 `740c626d…36872b`, the checksum the shader's
`Cargo.lock` held for it), with the changes below. The shader workspace takes
this copy in place of the published crate (`../Cargo.toml`,
`[patch.crates-io]`); `spirv-builder` 0.10.0-alpha.1 and pxl-shader-build
(`../build`) build with it. This file is the ledger of what differs from the
published crate, why, and what to do when either side moves. It replaces the
upstream README, and `Cargo.lock`, `.cargo_vcs_info.json`, `.cargo-ok` and
`Cargo.toml.orig` of the published crate are left out; nothing else outside
`src/` differs.

Our whole diff against the published crate, from the checkout's root:

```sh
d=$(mktemp -d) && curl -sL https://static.crates.io/crates/rustc_codegen_spirv/rustc_codegen_spirv-0.10.0-alpha.1.crate | tar -xz -C "$d" && diff -ru "$d/rustc_codegen_spirv-0.10.0-alpha.1/src" rust/client/shader/rustc_codegen_spirv/src
```

Every change site carries `PATCH(pixelet):`, and these are all of them, one
to one (`grep -rn 'PATCH(pixelet)' rust/client/shader/rustc_codegen_spirv/src`
lists the same 10):

| site                                                  | change |
| ----------------------------------------------------- | ------ |
| `linker/mod.rs`, `mod specialize_roots;`              | 2      |
| `linker/mod.rs` `link`, `remove_aborts`'s call         | 1      |
| `linker/mod.rs` `link`, the `specialize_roots` block   | 2, 3, 4 |
| `linker/mod.rs` `link`, `inline::inline(.., None)`     | 4      |
| `linker/inline.rs` `inline`, the `only` parameter      | 4      |
| `linker/inline.rs` `inline`, `Inliner { only, .. }`    | 4      |
| `linker/inline.rs` `Inliner`, the field `only`         | 4      |
| `linker/inline.rs` `Inliner::inline_block`, the filter | 4      |
| `linker/simple_passes.rs` `remove_aborts` (new)        | 1      |
| `linker/specialize_roots.rs` (new file)                | 2, 3   |

Upstream pull requests: none sent; whether to send them is the user's call.

## The numbers

Measured on the tree at 4d5d1a293 (34 entries), each the whole generated WGSL
without the checked-in header, with one change taken out at a time
(pxl-shader-build as it is, `--no-spirv-opt`):

| build                                                    | bytes     | lines  |
| -------------------------------------------------------- | --------- | ------ |
| the published crate, no panic strategy (main before)     | 5,615,187 | 93,014 |
| the unreachable strategy, these changes inert            | 4,558,300 | 83,768 |
| + `remove_aborts`, no `specialize_roots`                 | 4,096,345 | 79,100 |
| all of it (as checked in)                                | 3,298,579 | 69,519 |
| all of it but `copy_pointer_arguments`                   | 3,290,471 | 69,344 |
| all of it but the first inlining (`unspecializable`)     | naga rejects `beam_cs` (`Type InvalidData`) | |
| all of it, `remove_aborts` off, `specialize_roots` on    | build fails (`OpExtInstImport "Rust.rustc_codegen_spirv…"` left after SPIR-T) | |

The last row is why `specialize_roots` runs only where `remove_aborts` does
(`mod.rs`, `aborts_in_place`).

In Chrome on an Apple M-series Mac, a09e60a8d against the same tree with
these changes, two runs each, the view still at one place at noon:

| measure                                                   | without     | with        |
| --------------------------------------------------------- | ----------- | ----------- |
| the frame's GPU time, p50 / p95                           | 3.6–3.7 / 4.1 ms | 3.8–3.9 / 4.2 ms |
| a first load's shaders and pipelines, first made to last settled | 11.1 s | 9.8 s |
| the same, the shader cache warm                           | 1.5 s       | 0.7–0.9 s   |
| pixels that differ in a screenshot                        | 0.71%, all on the moving actors and the toast | |

## Change 1: aborts left in place under the unreachable strategy

- **Sites:** `src/linker/simple_passes.rs` `remove_aborts` (new);
  `src/linker/mod.rs` `link`, the call before inlining (`aborts_in_place`).
- **What it does:** with `--abort-strategy=unreachable`, deletes every custom
  `Abort` instruction (the one before an `OpUnreachable`) in every function
  before inlining, leaving the `OpUnreachable`.
- **What it replaces:** upstream emits the custom `Abort` under every strategy
  (`builder/intrinsics.rs`), and `inline.rs` forces every function that may
  abort, and every caller of one, inlined up to its entry (`should_inline`,
  cause `"panicking"`, the `NOTE(eddyb)` on `functions_that_may_abort`) so
  that `spirt_passes/controlflow.rs` can apply the strategy in the entry. The
  strategy is applied after inlining, so under `unreachable` a function that
  may panic is still inlined everywhere; this is what keeps it a function.
- **If lost:** every function that may panic is force-inlined again (429 on
  30b477576, `world::node` and every `cast::walk` among them); 4,096,345 →
  4,558,300 bytes with `specialize_roots` off, and `specialize_roots` is then
  off too.
- **Check:** build with `#[inline(never)]` on a function that may panic: no
  warning that it "has been inlined" with cause `panicking`.

## Change 2: pointer parameters rewritten away (`specialize_roots`)

- **Sites:** `src/linker/specialize_roots.rs` (new file); `src/linker/mod.rs`
  `mod specialize_roots;` and `link`, the block before `link_inline` that
  calls `unspecializable`, `inline::inline(.., Some(&first))`,
  `specialize_roots` and `copy_pointer_arguments`.
- **What it does:** `specialize_roots` clones a function per distinct tuple of
  the root global variables (with constant access chain indices) its pointer
  arguments come from, as Tint's `DirectVariableAccess` does
  (dawn `src/tint/transform/direct_variable_access.cc`): a `StorageBuffer`,
  `Uniform`, … pointer, an aggregate holding such pointers (a `World` by value),
  or a `Function` pointer to one (`&self` of a `World`). In the clone each
  pointer is an access chain from its global, dynamic indices and the
  aggregate's other leaves are parameters; a `Function` pointer only where the
  callee never writes through it nor lets it escape (`read_only`), its pointee
  traced through stores, `OpCopyMemory`, phis and returns (`trace`,
  `trace_mem`). `unspecializable` names what no clone can rebuild (a pointer
  returned, a closure capturing a local's address), inlined first so the rest
  can be traced. A call it cannot trace is left for the inliner, as before.
- **What it replaces:** `inline.rs` `should_inline` forcing every function
  with a pointer parameter of an illegal storage class inlined (`"illegal
  (pointer) parameter type"`, `"illegal parameter type"`), and its
  `FIXME(eddyb) rewriting away the pointer could be another alternative`.
- **If lost:** 3,298,579 → 4,096,345 bytes; `world::node` and the walks are no
  function in any module (written again at each call).
- **Check:** `world::node` one function in each of the modules that read
  `spec/engine/src/world.rs`, `cast::walk` one in each that casts: in a build's
  `*.spv` (`target/spirv-builder/…/pxl_shader.spvs`), count the `OpFunction`s
  whose `OpName` starts `pxl_engine::world::node`.

## Change 3: a field's address passed as a copy (`copy_pointer_arguments`)

- **Sites:** `src/linker/specialize_roots.rs` `copy_pointer_arguments`, called
  from the block of change 2 in `mod.rs`.
- **What it does:** where a call's `Function` pointer argument is an access
  chain into a local or a parameter (`&mut self.field`), passes a fresh local
  filled from it before the call and, where the callee may write through it,
  copied back after.
- **What it replaces:** `inline.rs` `should_inline` forcing such a callee
  inlined (`"illegal (pointer) argument"`: Logical addressing takes only a
  whole variable or a parameter as a pointer argument).
- **If lost:** 3,290,471 bytes, 8 KB less, but the walks (`cast::walk`,
  `walk_from_root`, `resume_walk`) are inlined in all 7 casting modules; and
  pxl-shader-build's `pointers_where_used` is then not needed.
- **Check:** as change 2, `cast::walk` one function per casting module.

## Change 4: `inline`'s `only`

- **Sites:** `src/linker/inline.rs` `inline` (the parameter), `Inliner`'s
  field `only`, its initializer in `inline`, and the filter in
  `Inliner::inline_block`; `src/linker/mod.rs` `link`, `inline::inline(..,
  None)` for the inlining as before.
- **What it does:** one inlining run inlines calls to the given callees only,
  the others left called; `None` is upstream's run.
- **If lost:** change 2 cannot inline what it cannot rewrite first; without
  that first run naga rejects `beam_cs`.
- **Check:** the build succeeds and naga takes every module.

## What pxl-shader-build relies on

- **The unreachable strategy** (`build/src/main.rs`,
  `ShaderPanicStrategy::UNSOUND_DO_NOT_USE_UndefinedBehaviorViaUnreachable`)
  works with the published crate: the abort's branch is dropped (5,615,187 →
  4,558,300 bytes). Only with change 1 does a function that may panic stay a
  function.
- **`CompactIds`** (`main.rs` `optimized`) is spirv-opt's and needs no fork:
  each entry's ids numbered on their own, so naga's names (`phi_N`) in one
  entry don't move when another changes. Without it 3,321,250 bytes, and the
  bytes of one module depend on every other.
- **`pointers_where_used`** (`main.rs`) needs no fork, but only change 3 makes
  it needed: a copy's access chain left by spirv-opt in another block than its
  use, which naga holds in a local of pointer type and rejects (`light_miss_cs`
  without it).

## Upgrading

1. Re-vendor: replace `src/`, `Cargo.toml` and `build.rs` with the new
   published crate's (`.crate` from static.crates.io as above), keep this file,
   and update the versions in `../Cargo.toml`, `../build/Cargo.toml`,
   `../rust-toolchain.toml` (the crate's `build.rs` names its nightly) and
   `../Cargo.lock`.
2. Re-apply each change at its sites (`grep` the old copy for `PATCH(pixelet)`),
   or drop the ones upstream made unneeded:
   - change 1 when upstream skips abort inlining under the unreachable
     strategy, or applies the strategy before inlining;
   - changes 2–4 when upstream rewrites pointer parameters itself (its
     `qptr` legalization landing, or its own specialization of the FIXME in
     `inline.rs`); change 3 alone if Logical addressing's whole-variable rule
     is met upstream.
3. Run, in `rust/`: `cargo shader-wgsl`, then `cargo shader-wgsl --check` (two
   builds, the same bytes); spirv-val without `--relax-logical-pointer` on
   each entry's optimized SPIR-V; `make test-ts` (Tint); `cargo test -p
   pxl-render`; and the checks under each change above. Measure the WGSL as in
   the table, and update it here.

[upstream]: https://github.com/rust-gpu/rust-gpu/tree/main/crates/rustc_codegen_spirv
