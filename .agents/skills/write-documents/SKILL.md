---
name: write-documents
description: How the documents people read are written and reviewed — README.md, ARCHITECTURE.md, CONTRIBUTING.md, xpute's own, infra's and the READMEs beside the code. Use when writing or editing one. Not for doc/, AGENTS.md or the skills.
---

# Writing the documents

For what people read. What an agent reads follows AGENTS.md's Instructions.

## Each document has one reader

A document answers one question for one reader, in one kind, and holds nothing
else. The kinds are an explanation (how it works and why it holds), a how-to
(what to type) and a reference (what each name means). A document that mixes
them serves no reader.

| Document | Reader | Kind | Question |
| --- | --- | --- | --- |
| `README.md` | someone who has just arrived | explanation | what is this, and how can it be? |
| `ARCHITECTURE.md` | someone who wants to understand or change the code | explanation | how does it work, why does it hold, and where is each part? |
| `CONTRIBUTING.md` | someone working on it | how-to | what do I type to set it up, run it and check it? |
| a README beside the code | someone working in that module | explanation | how does this module work? |
| `infra/README.md` | an operator | how-to | how is production set up and run by hand? |
| `xpute/USAGE.md` | someone building a program on xpute | how-to | what do I write and type to run my code on it? |

xpute has the same cut under its own theme. The reference is `spec/` and the
generated code's comments.

## Written as a paper is

A document is read as a paper is: its first paragraph gives the whole, each
section carries one argument a reader follows to its end, and what only proves
a point goes to an appendix. Here the appendix is the module's README, as near
its code as it can be, and a document above it names it and says no more. A
root document holds what lasts: the problem, the parts, how they meet, and
what each part does not know, which the code shows only by its absence. Its
length is what that argument needs; a detail laid in it that no argument
needs is a detail nobody reads.

It states its model as a textbook of computer architecture states a processor's:
the contract the system keeps, at its own level, simple enough to carry the
idea. Simplifying and leaving out is how it works; an exception, an edge case
or a low bit is said where it lives, in the code and the module beside it, for
the reader who goes down to look. What it must not do is say the system keeps
a promise it does not.

## Revising one

A document is revised by the lens it is written with, wide, not by its
sentences one at a time.

- Each section's idea is said in one line first. A section whose idea cannot
  be said, or that reaches it late under its support, is the finding.
- What is caught is a view: a structure, a boundary, an owner or a flow told
  as the code no longer has it; an idea the system stands on that the document
  does not state; two sections telling one idea.
- The model is what is checked against the code: the promise, the parts, how
  they meet and what each does not know. The count of sentences verified is no
  measure.
- A detail is not sharpened. One that is off, or that takes a clause to make
  exact, goes down to the module beside its code, or out. A sentence is added
  only where a reader would hold the model wrong without it, never to make a
  detail exact, and a revision that leaves a document longer is read again for
  what it added.
- Each level says its own and leaves the rest to the one below: a README what
  the thing is and why, `ARCHITECTURE.md` the model and why it is so, a
  module's README its part's duty and contract, the code its conditions, the
  spec what is promised to the bit.
- A statement goes when the code contradicts it or the argument does not need
  it, never because it was not looked up.

## Rules

- **A rule the system stands on is stated as a contract.** One sentence, in
  the document's exact terms, says what holds for every case, what alone may
  differ and so what breaks it; the section under it is its argument. The
  code is read against that sentence when it changes, so a sentence any code
  would pass, or a list of what the code does, guards nothing.
- **One theme, carried through.** A document is written from its theme, not
  patched from its last version. pixelet's is that the world exists only as far
  as it is seen; each part of `ARCHITECTURE.md` says which promise of the theme
  it keeps and why it can. A sentence that serves no part of the theme goes.
- **Whole sentences that people want to read.** Write what matters, the core and
  its reason, not an inventory of every member. A list is for things that are
  really parallel, never for an argument.
- **Facts are checked against the code**, older texts holding stale names and
  wrong claims. Name only the files, types and functions a reader needs to find
  the thing, by the name a reader searches for rather than a link that moves.
- **Earn a name before you use it.** Say the need first, then name what meets
  it. A name this project made needs this, and so does a field's term the reader
  may not bring (a `clipmap`).
- **Order by dependency.** The whole comes first, a model's vocabulary before
  the machine that uses it, and a map of the code last.
- **An `ARCHITECTURE.md` ends, before its map, with what it took from earlier
  work:** a table, each row its precedent and what is taken, the part named as
  the document names it, and no row for a name alone. This is not history.
- **Diagrams** are hand-aligned ASCII under about 78 columns; a diagram's
  branches name what comes out.
- **Each part is said once.** A paragraph that several documents would repeat
  lives in one, and the others link it.
- **A heading the code cites stays.** Comments name sections of
  `ARCHITECTURE.md` by their titles; keep those titles, or change the comments
  with them.
- **What no document holds:** history, a changelog, a glossary (a name is
  defined where it is first used), and any link to a minute, an experiment, the
  backlog or anything else under `doc/`, to AGENTS.md or to a skill. A command a
  person runs is written out, not sent to a skill.
- **Prose wraps at 80 columns.**
