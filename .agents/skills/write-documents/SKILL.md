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
- **Diagrams** are hand-aligned ASCII under about 78 columns, built with a short
  script; a diagram's branches name what comes out.
- **Each document says what is its own.** A document over another layer
  (pixelet over xpute) says what it inherits, links it, and moves on. It defers
  only to a document its reader opens: a README sends a person to another README
  or to ARCHITECTURE.md, never to AGENTS.md. How one module works goes in its doc comment or the README beside its code,
  and ARCHITECTURE.md points there. A file assembled at build time reads
  correctly on its own.
- **Name what outlives the document.** A README describes the system;
  hostnames, device models and version floors go in the masthead.
- **A fact the code holds is generated or derived.** A value from `spec/` is
  generated from it; a size, a count or an offset is stated by what it is
  derived from; code is cited as a path, then a name.

## Checks

- What the old version named (paths, identifiers, constants) is still there
  or was cut on purpose, and nothing points at what was cut.
- No minute under `doc/` is linked.
- Prose in `README.md`, `ARCHITECTURE.md`, xpute's two and `infra/README.md`
  wraps at 80 columns, in the paragraphs touched.
