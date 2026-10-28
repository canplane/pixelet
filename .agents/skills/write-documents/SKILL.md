---
name: write-documents
description: How the documents people read are written and reviewed — README.md, ARCHITECTURE.md, xpute's two and the READMEs beside the code. Use before writing or reworking one, before committing a change to one, and when one is hard to read but it is not obvious why. Not for doc/, AGENTS.md or the skills.
---

# Writing the documents

These documents are read by people who have not seen the code. Each one
explains a mechanism in an order you can follow from the top; it is not a
reference card or an essay.

This skill does not cover what agents write for each other: `doc/` (the
backlog, minutes and experiments), `AGENTS.md` and the skills. Their readers
already know the tree.

[reference.md](./reference.md) says what a document should be and which fault
each rule came from. Read it before writing or restructuring a document, or when
one reads badly for no clear reason. For an edit that keeps the order, the
checks below are enough.

## Before committing

- **Nothing named is lost.** Every path, identifier and constant in the old
  version is still there, or was removed on purpose. Diff the backticked
  tokens:

  ```sh
  f=README.md; diff <(git show HEAD:$f | grep -o '`[^`]*`' | sort -u) \
    <(grep -o '`[^`]*`' $f | sort -u)
  ```

- **No coined term is used before it is defined.** A heading that names its own
  subject, and an opening line that uses the thing's common name, are exempt.
- **Nothing points at what the document no longer holds.** After moving or
  cutting a section, recheck counts and summaries ("in two parts"), table rows,
  sibling documents that use the same word, forward references, and sentences
  whose antecedent was cut.
- **Prose wraps at 80 columns.** No formatter runs on `README.md`,
  `ARCHITECTURE.md`, xpute's two or `infra/README.md`, so wrap them by hand the
  way `deno fmt` wraps Markdown under `ts/`: break after the last word that fits
  in 80 columns. Tables, figures and unbreakable links may run past. Rewrap only
  the paragraphs you touched.
- **Read the whole document from the top**, rendered, not as a diff. The checks
  above each catch one kind of fault; only reading catches the rest (a
  repetition, a misaligned diagram, a dangling pronoun, a stale count).
