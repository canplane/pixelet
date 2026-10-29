# What the published build gives away, and where to stop

**2026-09-18** — b11 was read back from `plei.me` the way a stranger reads it,
and it turned out to be a description of the project. The module's name section
held 853 function paths, so the crate tree, its modules and its types printed
out of `strings` whole. Every panic location carried the home directory it was
built in. The data section held every shader in full, comments included, and
those comments named source files and a branch. The bundle spelled every command
and every errno as a string.

None of that was decided. It is what a release build hands over when nothing has
asked it not to.

## The rule this settled

**Every measure that paid stopped shipping something the program does not need
to run** — the name section, the producers and target-features sections, the
host ABI's names, the enum names, the shaders' comments, the source paths. Each
deleted information rather than disguising it, and each left the artifact the
same size or smaller.

**Every measure that did not pay disguised something the program does need** —
the WGSL identifiers, the WGSL whitespace. The information stayed and only the
labels moved, and both cost more than they returned.

Hold that line. Removing what a published build has no use for is worth doing
and will go on being worth doing. Scrambling what it runs on is not.

**For the shaders the stopping point is comments**, and that was tested rather
than assumed: the rename was built on naga, it worked, and it was dropped on the
numbers. Do not spend the effort again — the reasons are structural and a better
tool will not change them.

What is still worth doing is not obfuscation at all. The module carries about
1,400 strings its authors wrote — assert messages describing the memory map and
the rings, the tick's step names, the UI's action vocabulary. They say more
about the design than the shaders do, and they go by being written differently,
not by being disguised.

## What it is for, and what it cannot be for

Nothing here keeps a secret and nothing here could. Whatever runs on the client
can be recovered by the client, completely; obfuscation is not a weak form of
that guarantee but a different kind of thing. A real secret belongs on the
server, and the reason none of this reads as alarming is that there was never a
secret in the client to lose — what the module gives away is rendering
technique, not a credential.

What the work buys is time: it raises what reading costs, measured in an
analyst's hours rather than in "cannot". So it is worth something against a
casual copy, a competitor's first afternoon, a bot author — and nothing at all
against someone who has decided to find out.

One thing it bought was not about time. The build was publishing a home
directory and a user name, which is an ordinary information disclosure and
simply a mistake to leave in.

## The layers, and what each cost

| What it gave away                                     | What closed it                                 | Size   |
| ----------------------------------------------------- | ---------------------------------------------- | ------ |
| The name section — 853 function paths, the crate tree | `wasm-opt --strip-debug`                       | −66 KB |
| `producers`, `target_features`                        | `--strip-producers`, `--strip-target-features` | −225 B |
| The host ABI — 40 imports, 12 exports, by name        | `--minify-imports-and-exports`                 | −56 KB |
| Function boundaries                                   | `wasm-opt -O3`                                 | −56 KB |
| The shaders' commentary                               | `pxl-render/build.rs`                          | −11 KB |
| 174 command and errno names                           | a Vite plugin, below                           | −8 KB  |
| 87 source paths                                       | blanked after the link, below                  | 0      |

1,439,261 → 1,229,793 bytes, and nothing in it names a crate, a path of the
machine that built it, or a call the host answers.

## Binaryen owns the module, and it is the only thing that can

`wasm-opt` runs over the linked binary, which is what lets it rename what the
compiler cannot: the symbols embassy and `critical-section` require —
`__pender`, `_critical_section_1_0_acquire`,
`__embassy_time_queue_item_from_waker` — are fixed by those crates, and no
`#[export_name]` here can shorten them. After the link nothing cares where a
name came from.

It answers with the mapping it chose, and that is the part that needs wiring:
renaming an import changes a contract the hand-written shell is the other half
of. `build_wasm.ts` writes the mapping to `src/kernel/abi.ts` and `module.ts`
reads it in both directions, so neither side ever spells a letter. The letters
are the optimizer's and change whenever the module does.

`strip = true` came off the release profile for this. Cargo strips the target
features, and wasm-opt has to read them before it can take them off; with Cargo
doing it first, wasm-opt could not parse the module at all. Stripping belongs in
one pass and the pass is Binaryen's.

## Two claims about the standard tools that were wrong

**`trim-paths` is not stable.** It is Cargo's own setting for sanitizing the
absolute paths a panic carries — RFC 3127, and exactly the right tool — and
`cargo` refuses it: _"not stabilized in this version of Cargo (1.94.1)"_. It is
listed in the Cargo book under Unstable Features, which is easy to skim past
when the RFC reads as settled. On stable the answer is `--remap-path-prefix`.

**`vite-plugin-const-enum` does not exist.** It was named from memory as the
standard answer and npm has no such package.

The problem it was named for is real. A `const enum` is a compile-time name for
a number, but Vite transpiles each file alone with esbuild and bundles the
results with Rollup, so nothing ever sees a declaration and its use at once.
esbuild inlines a const enum perfectly well _when it bundles_ — measured both
ways — and emits the runtime object, reverse mapping and all, when it transforms
one file. That is not a setting anyone forgot; it is the pipeline's shape, and
it is why TypeScript documents `const enum` as incompatible with
`isolatedModules`.

Forty lines in `vite.config.ts` close it: read the declarations up front,
replace each member access with its value, strike the declarations and the
imports that named them. It refuses rather than guesses — a member whose value
is not a literal or arithmetic over literals leaves its whole enum alone, and an
access left behind after a declaration went is a build error. All 171 members
here are literals, so all 17 enums inline.

## The panic paths, and why Rust has them at all

`--remap-path-prefix` only rewrites a prefix, and a prefix is all it can
rewrite: `pxl/kernel/src/io/fetch.rs` keeps the part that says what the file is.
Mapping each crate root to a letter compiles and works, and leaves
`a/io/fetch.rs`, which is where the information was. Mapping a whole file path
does not even compile — rustc resolves `mod` against the remapped path and
cannot find the submodule.

**Rust embeds these and offers no way not to.** `unwrap`, `expect` and an index
are `#[track_caller]`, so the location reported is the caller's — which is why
`core`'s own paths are absent from the module while 59 of ours were in it.

That is not a defect. C embeds nothing a program did not ask for: `__FILE__` is
a macro someone wrote, `assert` is a call someone made, and `-DNDEBUG` takes
both out of a release. C++ is the same by default, with `std::source_location`
and the hardened standard libraries (`_LIBCPP_HARDENING_MODE`,
`_GLIBCXX_ASSERTIONS`) as things one opts into — turn them on and C++ leaks
paths exactly as Rust does, and needs the same `-ffile-prefix-map` to stop. Rust
embeds them because of what it promises instead: `v[i]` is checked, and the
answer to a failed check is a panic that says where it failed, which means the
file name has to be in the binary as a string. The density follows. C gets a
path per `assert` someone wrote; Rust gets one per file containing anything that
can panic, which is nearly every file that does work — several of our 59 contain
no `assert`, `unwrap` or `expect` at all and are there because they index a
slice.

`process::exit` and `process::abort` carry nothing, and this kernel already
calls `abort` where it gives up (`sched/turn.rs`). But those are calls, and a
failed bounds check is not a call anyone writes: the compiler puts a panic there
and the panic carries the location. So the only switch that removes them is the
one that changes what a panic _is_ (`panic_immediate_abort`), and
`panic = "abort"`, which this build already sets, is not it — it drops the
unwinding and keeps the message on purpose, because the module's hook forwards
that message to the console. The trap carries nothing, so without it a
production panic is one line of `unreachable`. The paths are the price of
knowing where.

None of this is news. RFC 3127 exists to sanitize exactly these paths, the rustc
book documents `--remap-path-prefix` for it, the Linux kernel's Rust support
patched the same leak in its own build, and removing build paths is a standing
concern of reproducible-builds.org. C has `-ffile-prefix-map` and Go has
`-trimpath` for the same reason. It is an information disclosure of the ordinary
kind, filed under hygiene rather than vulnerability, and mostly noticed by the
people who build compilers rather than the people who ship applications.

### So they are blanked after the link, and only in a published build

A `&'static str` is a pointer and a length held apart, so overwriting the bytes
where they lie, to the same length, leaves a module that runs as it did: nothing
compares a path at runtime. 87 paths, 4,443 bytes, to dots, in `build_wasm.ts`,
for the module `build` bundles (`--published`; until 09-21 it was gated on
`PXL_BUILD_NUMBER` instead, which only the deploy hands out — see below for why
that moved). A build anyone is working on keeps its paths — what is lost is the
file name in a panic's message, and it is lost only where nobody is reading it.
It is the same result `-Z location-detail=none` gives, without the toolchain.

Two things make it safe rather than lucky. **Every match is filled with the same
byte**, which is what an overlap needs: the linker merges a string that is
another's tail, so two matches can cover the same bytes, and one character
leaves the same result whichever order they are written in. And **a match under
no known source root stops the build** rather than being blanked on a guess — a
`.rs` from somewhere unrecognized could be a format string or something a person
typed. That check earned itself immediately, refusing
`/rust/deps/hashbrown-0.16.1/src/raw/mod.rs`, a real path from a dependency
rustc vendors that the roots did not yet name.

It runs after wasm-opt, which it must: the optimizer rewrites the data section,
and anything blanked before it would be moved or merged underneath. What no test
proves is how a real panic reads afterward, since the checks exercise no
panicking path; the argument is structural — the hook formats the location and
hands the text to the host, and nothing parses or compares it — and the module
boots, runs its turns and answers failed fetches with the paths blanked.

## The shaders have a low ceiling, and it is structural

WebGPU compiles a shader from its source text. There is no bytecode form the way
SPIR-V is one — the browser API deliberately left SPIR-V ingestion out — so the
WGSL is in the published module either way, and `createShaderModule` is one hook
away from it however it is written.

Three things follow, and none of them will change:

1. **It arrives as plaintext.** Encrypt it and it does not parse, so string
   encryption and bytecode interpreters — the tiers that make JavaScript
   genuinely painful to read — are impossible in principle rather than merely
   unimplemented. Renaming and minifying are the ceiling, and they are
   JavaScript obfuscation's floor.
2. **The structure has to stay legible to the GPU.** `@group`/`@binding`
   numbers, the vertex locations, the arithmetic. Everything read out of this
   module worth reading — the lane-major instance buffer, the badge's depth
   test, the sky dome's noise — is structure and arithmetic, not names.
3. **A shader is small and does one visible thing.** `_0 = _1 * _2` still reads
   as a rotation followed by a translation, and a mat4 against a position still
   reads as a view projection whatever the matrix is called.

WGSL also has no pointer arithmetic — `ptr<...>` cannot be offset or cast — so
the flattening one might reach for instead ("struct to array plus index") is the
most that can be expressed, and this codebase already writes that way where it
matters (`block[offset + k*count + i]`). It hides the grouping and leaves the
offsets.

**The comments were still worth taking off.** They carried what the code does
not: why a branch exists, and file names (`volume.rs`, `shade.wgsl`) that leaked
the module tree a second time after the name section was stripped. The sources
keep them; `build.rs` strips them on the way in. 77,816 → 46,376 characters
assembled.

### Renaming the identifiers was built, measured and dropped

naga as a build-dependency, `wgsl-in` to parse and `wgsl-out` to write back, the
names replaced in the IR between them. The IR is what made it safe: in the text
a struct field access and a vector swizzle are both an identifier after a dot,
and `x`, `y`, `z`, `w`, `r`, `g`, `b`, `a` are used as both here, so no care
with a regex can tell them apart — only resolved types can. It worked: every
shader compiled and every pipeline record still matched.

It was dropped on this:

- **It is the only measure that made the artifact bigger.** 46,376 → 57,149
  characters, +12 KB on the module. naga writes `vec3<f32>` where the source
  wrote `vec3f` and spells out what was implicit, and a shader carries more type
  notation than names, so shorter names lose.
- **It hides labels, not structure** — the three reasons above.
- **The vocabulary worth hiding survived anyway.** `Frame`'s members —
  `sun_dir`, `fog_color`, `shadow_strength`, `light_view_proj` — are the
  highest-level statement of what a frame does, and the test that holds
  `FRAME_AT` to the struct finds each field by name. Rewriting it to check by
  member position would free fifteen more names; that is a fix, not a reason.
- **It carries standing risk for a cosmetic gain.** The round-trip must stay
  semantically faithful across naga versions, and the tests catch "does not
  compile", not "draws differently".

Minifying the rest goes the same way. Removing the whitespace that is left saves
6,316 characters and **392 bytes once gzipped**, on a 1.2 MB module; it needs
the tokenizer the rename needed, it costs the line numbers a driver reports
against, and `mat4x4f(f[0],f[1],f[2],f[3])*position` reads the same either way.
JavaScript minification buys transfer size and parse time; a shader's transfer
size is absorbed by gzip and its parse time is a driver's lexer.

## Two axes, and only one of them is finished

The published module was swept string by string once the paths were gone. **On
the hygiene axis it is done**: no home directory, no user name, no credential,
no key, no token; the only URLs are the `/api/tiles` and geocode prefixes the
client requests anyway. Nothing left in it is private.

**On the black-box axis it is not**, and what is left is neither privacy nor
WGSL. About 850 strings remain, and the ones that matter are panic and assert
messages, which give away the same kind of thing the paths did — the paths named
the module tree, and these describe what is inside it:

- **our own invariants** —
  `what is fixed in this memory leaves the heap less
  than it is budgeted`,
  `ring: advance on an empty ring`, `a chunk's palette
  past a leaf's width`,
  `band shorter than its voxels`
- **the packet contract** — `bad packet: magic mismatch`, `truncated payload`,
  `missing string terminator`, `payload OOB`, so XTP's framing reads off the
  binary without opening a decoder
- **the standard library's** — the B-tree's `idx < CAPACITY` and its height
  invariant, which say less about this project than about which containers it
  uses

Alongside them sit the name tables the spec generates and the shell reads: the
ground channels, the color catalog, the road classes, the tick's phases, the
settings' sections, the UI's fourteen actions. Those are domain vocabulary
rather than internals, and the last of them — the action names — is the one that
hands back what the ring's numbering otherwise hides.

### Most of this is a decision already taken, not a new one

`message.rs` settled the principle a while ago, for what the kernel says to the
user: _the kernel carries the number and the shell owns the words, because a
string that exists to be read has no reason to sit in the kernel's memory._ The
standing rule beside it says the same about identity — the kernel finds nothing
by a string, and a catalog is numbers.

Every authored string above is one of those two shapes. The name tables are
identity and the rule already covers them; a panic message is a string that
exists to be read, and `message.rs`'s reasoning applies to it unchanged. So this
is not an obfuscation measure waiting to be justified. It is the policy carried
the rest of the way, and the fact that it also closes the leak is a consequence
rather than the argument.

Two things bound it.

**The standard library's strings cannot be numbered.**
`assertion failed: idx <
CAPACITY` and `index out of bounds` come from `core`
and `alloc`, and nothing written here reaches them. Only `panic_immediate_abort`
removes those, by dropping the formatting rather than the text — nightly, and it
costs every panic message the console would have shown.

**A panic is not a message.** `Message` numbers what a user should see, and the
shell has the locale file to look it up in. A panic is for whoever is debugging,
there is no locale file for it, and numbering one means the shell needs a table
that maps numbers back to text it can print. That is a real mechanism, not a
rename, and it is worth saying so before anyone starts.

So: as hygiene, finished. As a black box, the compiler-injected half is closed;
the authored half is open, and it is the existing policy's remaining work rather
than a new one — with a residue of standard-library text that only the nightly
switch reaches.

## 2026-09-21 — the settings the build does not offer

Read back again, the bundle named the nine settings a production build
withholds, with their defaults:
`enable_terrain_carving:{kind:"toggle",
default:!1,section:"editing",dev:!0}`
and the eight beside it, each carrying the `dev:!0` that says which rows a
reader is not supposed to see. The kernel module held the same nine names in its
data. And the action that sets one was reachable: `set_setting` checked what
storage brought in against the build's offer and checked nothing that arrived
live, so `kernel_action({type:"set_setting",key:"show_ledger",value:true})` from
the console turned the scheduler ledger on in the published build.

None of it was a decision either. The settings table is generated whole from
`schema.json`, and the sheet filtered it on `__IS_DEV__` at run time — which
removes the row from the screen and leaves it in the artifact, the same shape
the debug section's labels had taken through the locale catalog. The rule above
already says what to do with it: the rows are something a production build does
not need to run, so they go.

**The shell takes a second table.** `script/spec.ts` writes `SETTINGS_SHIPPED`
beside `SETTINGS`, the same literal without its `dev` entries, and
`feature/setting/schema.ts` picks one with `__IS_DEV__ ? EVERY : SHIPPED`. The
bundler folds the constant and drops the literal a production build never reads,
so a dev row is absent rather than hidden. The type stays the whole table's — a
dev key is a `SettingKey` in every build; what a production build lacks is its
row — which is what lets the kernel's test still name `show_poi`.

**The kernel takes a feature.** `pxl-kernel`'s `dev`, on by default and
forwarded through `pxl-wasm`; the dev rows of the table and the store's arms for
them sit under `#[cfg(feature = "dev")]`, and `build-wasm.ts` builds the
published module `--no-default-features` — under its `--published`, the same
flag the panic paths are blanked on. A setting out of the table is not offered,
not stored and not named, and what it does the module still does: the store's
defaults are written from the spec's constants rather than from the table, so
the POI layer is on in a build that cannot turn it off. Two tests hold the seams
— that a module without the feature has no `dev` row at all, which is what
catches a `dev: true` added to the JSON without its `cfg`, and that the
hand-written defaults agree with the table on either device.

**The live path checks the offer.** `set_setting` refuses a key `is_offered`
says no to, as `load` already did; the row is gone from the sheet, but the
action that sets it is not gone from the console.

**`check-bundle.ts` reads for all three.** A dev row in the JavaScript — the
name as a property whose value is an object, not the bare name, since the ledger
overlay reads `settings.show_ledger` and the ledger report has an `msaa` of its
own — with `show_structures`'s row as the sentinel; and a dev name in the
kernel's data.

**One flag says what the published shape is.** Both of the module's omissions
were first gated on `PXL_BUILD_NUMBER`, the number the deploy's gate hands out,
on the reading that a number is what says a module is going out. It says who
built it, not what shape it is: a local `deno task build` came out with the dev
names and the panic paths in, so its `check:bundle` had to skip the kernel and
the unnumbered deploy `deploy.ts` allows would have published both. Now `build`
bundles the module `build-wasm.ts` makes with `--published`, whoever runs it,
and the number is a stamp on the build info and nothing more: a local build and
the deploy's are the same bytes, and the kernel is checked every time. What this
costs is the file name in a panic under `vite preview`, which is the deploy's
own behavior reproduced rather than a loss.

One row nearly changed sides and did not. `show_ledger` turns on the scheduler
ledger's panel, and the panel's hook — `settings.show_ledger`, read by an
overlay mounted in every build — was still in the production bundle, which read
as the ledger being meant for a published build, measured on a phone. It is not:
the overlay returns nothing outside `import.meta.env.DEV`, which `vite build`
folds to false whatever `VITE_APP_ENV` says, and the panel's markup is absent
from the published bundle. Taking the setting out of `dev` on that reading
shipped a toggle that did nothing, for one build; it is `dev` again, and what
was dev stays dev unless the thing it switches is made to work outside it first.

| What it gave away                                  | What closed it                         | Size    |
| -------------------------------------------------- | -------------------------------------- | ------- |
| 9 dev setting rows in the bundle, defaults and all | `SETTINGS_SHIPPED`, folded on `define` | −100 B  |
| The same 9 names in the kernel's data              | `pxl-kernel`'s `dev` feature, off      | −1.3 KB |
| `set_setting` from the console, in production      | `is_offered` on the live path          | 0       |

The kernel's figure is the published module against the same source with the
feature on: 1,048,892 → 1,047,548 bytes, and 1.5 KB less gzipped. The build with
the feature on is itself 91 B under the head it was cut from, the live guard
included. Measured twice, because the first reading came out as growth: another
change had bumped `libm` underneath it between the baseline and the build, and
the module moved 1.6 KB on that alone. A size that goes the wrong way for no
reason the diff shows is a baseline to re-cut, not a number to explain.

## Where it stands

Deployed, b14 being the first build that carries the 09-18 work. The settings
are closed in the tree and not yet in a build.
