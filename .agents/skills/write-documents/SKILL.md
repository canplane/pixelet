---
name: write-documents
description: How the repository's prose is written and reviewed — README.md, ARCHITECTURE.md, xpute's two, the READMEs beside the code, the live minutes and the skills. Use before writing or reworking one, before committing a change to one, and when a document is hard to read but it is not obvious why.
---

# Writing the documents

The documents are read by someone who has not seen the code. They are not
reference cards and not essays: each one explains a mechanism in an order that
can be followed from the top. `doc/minutes/archive/` and `doc/experiment/` keep
their day and are not held to this (`doc/README.md`).

What a document should be, with the faults each rule was written after, is
[reference.md](./reference.md). Read it before writing a document or
restructuring one, and when one reads badly for no reason you can name. For an
edit that leaves the order as it is, the checks below are enough. How code is
cited is the convention skill's.

## Before committing

- **Nothing named is lost.** Every path, identifier and constant of the old
  version is still there, or is gone on purpose. A rewrite loses `-errno` or
  `16 bits` without anyone noticing, so diff the backticked tokens:

  ```sh
  f=README.md; diff <(git show HEAD:$f | grep -o '`[^`]*`' | sort -u) \
    <(grep -o '`[^`]*`' $f | sort -u)
  ```

- **No coined term comes before its definition.** For each name the tree
  invented, its first use is not earlier than where it is earned. A heading
  naming its own subject, and an opening line calling the thing what the world
  calls it, are exempt.
- **Nothing points at what the document no longer holds.** After moving or
  cutting a section, reread what describes the document: counts and summaries
  ("in two parts"), a table row for a task that has grown, the sibling documents
  where the same word stands, a forward reference the move created, and any
  sentence whose antecedent was cut.
- **The wrap is the file's.** No formatter touches the documents outside the
  code trees (`Makefile`), so a file keeps the wrap it has: `README.md`,
  `ARCHITECTURE.md`, xpute's two and `infra/README.md` run a paragraph to one
  line; `AGENTS.md`, `doc/` and the skills wrap at about eighty columns. A
  Markdown file under `ts/` or `xpute/ts/` is `deno fmt`'s, and `make check`
  holds it. Rewrapping to the other convention is a diff of the whole file for
  nothing.
- **Read the whole document from the top**, on the page rather than in a diff.

The last is not a formality, and the others do not replace it. Each check above
finds the class of fault it was written for and no other. A repetition eight
lines apart slips a similarity test when the sentences around it differ; a
diagram one column out looks fine in a diff; a pronoun with nothing to point at
parses cleanly; a count that has gone stale is still a number. Worst of all, a
document that defines nothing passes the defined-before-use check perfectly,
because there is nothing for a use to be early of. Reading is what finds those,
and there is no substitute for reading all of it.
