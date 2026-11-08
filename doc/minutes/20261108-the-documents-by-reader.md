# The documents, by reader

**2026-11-08** — recorded with the user, before pixelet's and xpute's documents
were written again. Main was at `4deb3dbcce`.

## What the user decided

**The documents are written new, around one theme.** pixelet's theme is that the
world exists only as far as it is seen. The eye sets the detail, so what a frame
costs is bound to the screen and not to the world. Everything asks one question
of one function. What can be computed is not stored. What is not yet known is
answered coarser or not at all, and never made up. What was found is remembered.
A document is not the old text shortened or patched: it is written from the
theme, and the old one is read only for the facts the new one needs, each
checked against the code, since the old text holds stale names and wrong claims.
The new file is written beside the old one, and replaces it once it has been
reviewed.

**Each document has one reader and answers one question.**

- `README.md` is for someone who has just arrived and asks what this is and how
  it can be. It carries the theme in a few paragraphs, the play link and the
  screenshot, the two commands that start it, and where to read next.
- `ARCHITECTURE.md` is for someone who wants to understand or change the code.
  It starts from the problem and the theme, ties each claim of the theme to the
  part that keeps it (the world, the eye, drawing, the core under them, making
  the data), and ends with the boundaries and the invariants, often an absence,
  and a codemap in the order of dependency. A separate DESIGN.md was weighed and
  not taken: once how each module works moves beside its code, why a part holds
  and where it lies are about the same parts, and two documents would say them
  twice.
- `CONTRIBUTING.md` is for someone working on it, and holds only what a person
  does at the keyboard: the setup, the dev modes, the targets, the environment,
  the device builds, the checks and `make spec`. The rules the agents' files hold
  are not copied into it.
- How one module works goes into a README beside its code: the renderer, the
  core's layers and clipmap, the reads and their scheduler, the bake, the client.
  `rust/spec/src/space/README.md` is the model.
- `infra/README.md` stays, for an operator.

**xpute takes the same cut under its own theme.** Its README is for the author of
a program that embeds a core: the picture, the smallest example, and what xpute
leaves to the program. Its ARCHITECTURE is built around the contract between the
core and the runtime. What another language implements against is reference,
kept in `spec/` and the generated code's comments, and the XTP README stays as
the format's reference.

**What no document holds.** No history. No CHANGELOG and no glossary: a name is
defined where it is first used. No link to a minute, an experiment, the backlog
or anything else under `doc/`, and none to AGENTS.md or a skill; the agents'
setup moves from the README into AGENTS.md. A paragraph that several documents
would repeat, such as the core and the runtime as a unikernel and its monitor,
stays in one.

**How the work is done.** Every session works in the main checkout; a worktree
is used only where a separate tree must be built. The work is split by file, a
file being edited by one session at a time, and a session commits only its own
paths, at the commit size the user set.
