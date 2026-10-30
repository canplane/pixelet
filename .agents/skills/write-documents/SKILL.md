---
name: write-documents
description: How the documents people read are written and reviewed — README.md, ARCHITECTURE.md, xpute's two and the READMEs beside the code. Use when writing or editing one. Not for doc/, AGENTS.md or the skills.
---

# Writing the documents

## Rules

- **Lead with the core.** Put first the claim the rest follows from. The
  masthead (tagline, date, play link, screenshot) stays on top.
- **Earn a name before you use it.** Say the need first, then name what meets
  it, in a main clause:

  > On the host side, something has to decide when to ring at all. That is the
  > **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only
  > when something asked for a turn.

  A name this project made needs this (`XTP`, `strobe`), and so does a field's
  term the reader may not bring (`ring` as io_uring uses it, `doorbell` as NVMe
  does, `handle`). A term used as the field uses it and widely known (`arena`,
  `errno`, `slab`, `golden records`) is used as it is.

  Which name a thing gets is the naming skill's
  ([naming](../naming/SKILL.md)); this one explains it. Defining a name does
  not approve it: a new name follows naming first.
- **Order by dependency.** A catalog of module names goes at the end, a model's
  vocabulary before the machine that uses it.
- **One word, one meaning.** Watch for words used in two senses (`frame` as a
  drawn frame and a frame of reference; `page` as a pyramid square and a
  browser tab). Where the code uses one word for both, qualify it where it is
  defined (`a frame of reference`).
- **Diagrams** are hand-aligned ASCII under about 78 columns, built with a short
  script; a diagram's branches name what comes out.
- **Each document says what is its own.** A document over another layer
  (pixelet over xpute) says what it inherits, links it, and moves on. It defers
  only to a document its reader opens: a README sends a person to another README
  or to ARCHITECTURE.md, never to AGENTS.md. ARCHITECTURE.md holds the
  structure: the parts, the rule each follows, and what crosses between them.
  How one module works goes in its doc comment or the README beside its code,
  and ARCHITECTURE.md points there. A file assembled at build time reads
  correctly on its own.
- **Name what outlives the document.** A README describes the system;
  hostnames, device models and version floors go in the masthead.
- **A fact the code holds is generated or derived.** A value from `spec/` is
  generated from it; a size, a count or an offset is stated by what it is
  derived from; code is cited as a path, then a name.

## Checks

- **Nothing named is lost.** Every path, identifier and constant in the old
  version is still there, or was removed on purpose. Diff the backticked
  tokens:

  ```sh
  f=README.md; diff <(git show HEAD:$f | grep -o '`[^`]*`' | sort -u) \
    <(grep -o '`[^`]*`' $f | sort -u)
  ```

- **No term of this project's is used before it is defined.** A heading that
  names its own subject, and an opening line that uses the thing's common name,
  are exempt.
- **Nothing points at what the document no longer holds.** After moving or
  cutting a section, recheck counts and summaries ("in two parts"), table rows,
  sibling documents that use the same word, forward references, and sentences
  whose antecedent was cut.
- **No minute is linked.** A document cites the code, another document or an
  outside source, never a record under `doc/`.
- **Prose wraps at 80 columns** in `README.md`, `ARCHITECTURE.md`, xpute's two
  and `infra/README.md`, by hand: break after the last word that fits in 80
  columns. Rewrap only the paragraphs you touched.
