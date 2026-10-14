# Agent Instructions

## The main axis

Every structural choice is judged by whether it moves the code further
down one picture; a user-level loader, kept in the agents' memory outside
the tree, is that picture at the size of one loader.

- **The eye decides.** The view is a grid of angular cells about the eye,
  one ray a cell; only what those rays observe is drawn, by the same rule
  wherever the eye is. A chunk is a cube of world cells at one zoom; the
  zoom it is observed at is the one whose block, drawn whole, spans at least
  a cell of the camera grid at its distance.
- **The query is the only seam.** Every reader of the world — a ray, a
  chunk, a walk, an edit — asks the same query directly for what is in a
  node at its zoom. Each source answers from its own level of detail; its
  storage representation stays behind the query.
- **Drawing and mechanics compose the same blocks** (`pxl-spec block/`):
  the ground's grid, then what overwrites it
  (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md). Control is
  headless; render only colors.
- **Level of detail is the world there.** At each zoom the world is what
  its fold defines; what the fold discards is absent at that zoom, and finer
  detail is not recovered to preserve it.
- **Descent carries what it knows**: a ray passes empty nodes whole and
  descends only where needed; what the descent has established is reused,
  never recomputed from the root. Speed is sought only inside the
  hierarchy; a fast path beside it is refused even when measured faster. A
  kind of thing is drawn wholly on the CPU or wholly on the GPU.
- **Dependencies point one way:** turn, door and shell (the UI's way in)
  wire everything from the top, and nothing below names them; control,
  which the door's events enter, moves the eye over the view and asks the
  world's query, naming neither scene nor chunk, whose share of what it
  decides the turn hands down; scene → chunk → view → query → layer, an
  order and not a path: the dependencies form a DAG, not a pipeline, and an
  edge may skip a step but never points up; io and the turn's edge and
  phases below everyone, io as transport; leaves (settings, models,
  billboards) below what reads them. The profiler is a
  debug path beside the axis: it reads anything, and production reaches it
  only where it records. An edge up the axis is a defect.

**Data structures before code** (Linus Torvalds). The structure is named
first, then asked whether one structure with different sources covers every
case. Good taste is a special case rewritten until it is the general one and
its branch is gone; finding a structure more reducible than the one at hand
is the work worth praising. Structure has an aesthetic, and it is not
decoration: many cases shown to be one, symmetry, orthogonal concepts,
bounds that follow from the shape — the problem's structure carried into
the code with little lost, as in Kernighan and Ritchie's _The C Programming
Language_, where what seemed complicated turns out simple. A patch, a duty
held twice, or a cache, skip, threshold or gate that makes a wrong
representation fast is called crude, whoever wrote it; below seventy percent
of what Torvalds would take is garbage. Rationalizing it — by industry
habit, by unchecked facts, by retracting a true fact under pushback — is not
tolerated. A change is one its author understands and owns, split so each
part reads alone, nothing unsolved left for its reviewer. "Talk is cheap.
Show me the code": a document, a plan or an explanation counts for nothing
beside the structure in the code, and existing code is judged against that
structure, never defended by why it came to be.

**A reviewer is independent, not obedient.** Structural elegance is what
the user weighs first, and their decisions are hypotheses about it, revised
when a real counterexample shows one fails. Where a design holds, nothing is
added to it. Where it fails, the counterexample is shown: the concrete case,
and which two of its own conditions cannot both hold. Where a more reducible
structure is seen, it is proposed with its proof: what disappears — states,
owners, paths — and which guarantee stays. Complexity without that argument
is garbage: a state, margin, exception, parameter or interface added to a
problem the existing invariants already solve. Stopping thought is no cure
for it; arguing is.

**Reducing is entropy reduced**, measured by the states, owners, names,
paths and copies that disappear, never by lines. In order:

1. **One fact, one representation.** Two expressions of one fact fold into
   one; done repeatedly, it is what leaves the dependencies a tree. A rebake
   or a broken old artifact weighs nothing against it. Two voices for one
   role means one goes; the one exception is an interface whose
   implementations genuinely differ, as each layer answers the query.
2. **One fact, one owner.** A fact the bake and the client both need — the
   drawn surface, its bound — is one function where both compile, and a
   bound is derived from that function's own reads, never from a second
   fact about it.
3. **A state removed rather than handled.**
