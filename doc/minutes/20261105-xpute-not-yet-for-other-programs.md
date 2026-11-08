# xpute's devices io_uring's way, names that say what each is, and xpute not yet for others

**2026-11-05** — recorded with the user, in the session that worked xpute's
guest and host on `main` while another session worked the renderer on
`small`. It followed the audit of the day before
([20261104](./20261104-kernel-and-shell-as-an-operating-system-means-them.md))
and ended on whether a program outside this tree can take xpute today.

## What the user decided

- **The audit's two duplications go.** The file device's open and close are
  requests io_uring's way: posted with ACKREQ, tagged with the descriptor,
  each completed once by a frame flagged RES on the submission ring;
  `FS_OPENED` goes. A close completes an open not yet completed with
  ECANCELED. A write's bytes are the runtime's until its descriptor's close
  completes, and the runtime reads them there
  ([what xpute answers for](./20261107-what-xpute-answers-for.md)). ENODEV for an unattached device
  reaches the guest as that completion.
- **On native, the guest's own thread decides the next turn**: it runs until
  it has nothing to do and sleeps until its wake time or a kick; the strobe
  paces only the display's frames, and the thread pauses with the app hidden.
- **A name says what the thing is or does, not what happened to it.** A
  method is named by its act (`kick`, not `sent`). A past participle stands
  where it reads as a question about a value: a free helper or a macro-style
  predicate (`WIFEXITED(status)`, 0405's `NODE_IS_NIL(node)`, `signaled(fence,
  ticket)`), a state that reads at once (`paused`), or a record of an event
  (`FsOpened`). On an object, a state's question is `is_x()` in Rust.
  Conventions are taken from Rust and C++ first; TypeScript matters less now.
- **`ring` stays** for the native call that runs a turn and waits for it: it is
  the TypeScript doorbell's `ring` on the other host.
- **TypeScript's members**: a `private` member takes a leading `_`,
  `protected` is for a class that is extended, and no field is `#` (`#` has no
  `protected`). What to hide is the user's judgment by context, not a rule:
  in the user's own code (fx-client's `AttrNode`, `MappedAttrNode`,
  `ServiceStoreBase`; 0405's `Arena`, `Deque`, TLV) what one holds and what
  is complete is shown, and what changes only by a method, or is half done
  (a partial result, a helper that leaves the state half changed), is
  hidden, "like not showing an intermediate result to someone not trusted to
  know it". The naming skill records the spelling only.
- **The commits are rearranged by kind**, each kind its own commit; a commit
  in the middle need not build. `main` and `small` each rearrange theirs
  above the point they parted; `small` rebases onto `main`, and `main` moves
  forward to it; then the unpushed commits before that point are rearranged
  too, the two sessions agreeing, and each session's records name the new
  hashes, the old ones going to `original`.
- **A test another session's commit broke is handed over and not thought
  about here**: `decor::hand_frames_answer_as_the_rust`, broken since
  `3d9aa5b91e` (originally `728020efcc` on `original`), is `small`'s; what it
  was and how it was fixed is in
  [taking rust-gpu out](../experiment/20261103-the-shaders-by-hand/README.md).
- **Frame pacing stays in xpute, whole** (`@xpute/host` `sched/frame_pacing.ts`),
  as the agents advised and the user took. The display's part, the refresh
  read off the animation frames and turns a whole number of refreshes apart,
  is every page program's; the rule of a turn interactive after input or
  where the guest asks the next edge, else spanning `batch_frames`, is every
  interactive page program's; the numbers (`pace_hz`, `margin_share`,
  `settle_ms`, `batch_frames`) are the program's and come in through
  `FramePacingConfig` at boot. The strobe knows only `SchedulingPolicy`, so
  the 20261030 rule, pacing a policy injected at boot, holds. It is split
  only when a second program needs another rhythm than input and the asked
  edge.
- **`Reading::take` keeps its name**: Rust's standard library has `take` for
  taking out what a holder holds, or nothing (`Option::take`, `HashSet::take`,
  `OnceCell::take`), and no `take` that waits sets a `try_` one apart.
- **xpute is not fixed now.** Running the web and the app comes after `main`
  and `small` are merged.

## Can another program take xpute today

Asked by the user; the agents' reading, not a decision.

- **What the design gives**: a program writes its machine and its protocol
  files, a turn, and one `boot(...)`; `xpute-gen` writes both ends' glue; the
  rings, the devices, the scheduling, the native thread, the React Native
  module and the C++ amalgamation are xpute's.
- **What stands in the way**:
  - Nothing is published: the npm packages are private at `0.0.0` and the
    crates `publish = false`; a program depends on the tree by path.
  - The API still moves: in this one day the file protocol, the native
    scheduler, `send_with` and `receive` changed.
  - The native start is heavy (Expo, the NDK, JDK 17, Xcode), and the one
    example runs in Node, with no page or app beside it.
  - No guide goes from an empty project to a first guest on screen.
  - The TypeScript packages ship `.ts`, so a consumer's bundler strips types.
  - None of today's changes has run on a page or a device.
- **What it would take, least first**: fix the protocol and the API and call
  it 0.1; publish, or export a repository to depend on by git; a template of
  one page and one Expo app; a guide that walks it.
- **The user's word**: it cannot be fixed now, so none of this is started.

## What was done

On `main`, above the point `main` and `small` parted, in place of the work
after it, whose originals go to `original` (the point `e2ece951d2`, the work
from `e3bbb79e87`):

- docs: the unikernel model and the documents.
- the specs, the generator (`ackreq`, `reply`) and the generated files; the
  traces complete opens and closes.
- the guest, Rust and C++: the door's requests and completions, `host_turn` (a
  task waits on its host without asking a turn, which ends the native busy
  wait), the C++ reading without `shared_ptr` (the wasm guest imports nothing
  of WASI), names.
- the host: completions in the doorbell and the file server, the native
  scheduler and its pause, names, the `_` spelling.
- the renderer and the GPU: WRITE_BUFFER_AT with no copy, the fence, names.
- the kernel.
- the client's names.
- the seed hook and the naming skill.

On main these are now in `e8d41c585c` (the docs), `c20671be6f` (the specs to the
kernel) and `acc9ec8d4a` (the client's names and the seed hook).

`small` was rearranged into nine commits by kind and rebased onto them, and
`main` moved forward to it, with the user's word. The unpushed commits before
the parting point were then rearranged by kind as well, the world's, the
renderer's, the fix and the test kept as they were, xpute's in one and the
client's in one, both now in `e8d41c585c`; `main` ended with its tree the same as
before.

## Open

- The web and app builds run, now that `small` is merged.
- Fixing xpute for other programs, when the user takes it up.
