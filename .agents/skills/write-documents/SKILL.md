---
name: write-documents
description: How the documents people read are written and reviewed — README.md, ARCHITECTURE.md, CONTRIBUTING.md, xpute's own, infra's and the READMEs beside the code. Use when writing or editing one. Not for doc/, AGENTS.md or the skills.
---

# Writing the documents

For what people read. What an agent reads follows AGENTS.md's Instructions.

## Each document has one reader

A document answers one question for one reader, and holds nothing else.

| Document | Reader | Question |
| --- | --- | --- |
| `README.md` | someone who has just arrived | what is this, and how can it be? |
| `ARCHITECTURE.md` | someone who wants to understand or change the code | how does it work, why does it hold, and where is each part? |
| `CONTRIBUTING.md` | someone working on it | what do I type to set it up, run it and check it? |
| a README beside the code | someone working in that module | how does this module work? |
| `infra/README.md` | an operator | how is production set up and run by hand? |

xpute has the same cut under its own theme; what another language implements
against is reference, kept in `spec/` and the generated code's comments.

## Rules

- **One theme, carried through.** A document is written from its theme, not
  patched from its last version. pixelet's is that the world exists only as far
  as it is seen; each part of `ARCHITECTURE.md` says which promise of the theme
  it keeps and why it can. A sentence that serves no part of the theme goes.
- **Whole sentences that people want to read.** Write what matters, the core and
  its reason, not an inventory of every member. A list is for things that are
  really parallel, never for an argument.
- **Facts are checked against the code**, older texts holding stale names and
  wrong claims. Name only the files, types and functions a reader needs to find
  the thing.
- **Earn a name before you use it.** Say the need first, then name what meets
  it. A name this project made needs this, and so does a field's term the reader
  may not bring (a `clipmap`).
- **Order by dependency.** A model's vocabulary comes before the machine that
  uses it, and a map of the code comes last.
- **An `ARCHITECTURE.md` ends, before its map, with what it took from earlier
  work:** a table, each row its precedent and what is taken, the part named as
  the document names it, and no row for a name alone. This is not history.
- **Diagrams** are hand-aligned ASCII under about 78 columns; a diagram's
  branches name what comes out.
- **Each part is said once.** A paragraph that several documents would repeat
  lives in one, and the others link it. How one module works lives beside its
  code, and `ARCHITECTURE.md` points there.
- **A heading the code cites stays.** Comments name sections of
  `ARCHITECTURE.md` by their titles; keep those titles, or change the comments
  with them.
- **What no document holds:** history, a changelog, a glossary (a name is
  defined where it is first used), and any link to a minute, an experiment, the
  backlog or anything else under `doc/`, to AGENTS.md or to a skill. A command a
  person runs is written out, not sent to a skill.
- **Prose wraps at 80 columns.**
