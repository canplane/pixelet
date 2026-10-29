---
name: write-documents
description: How the documents people read are written and reviewed — README.md, ARCHITECTURE.md, xpute's two and the READMEs beside the code. Use before writing or reworking one, before committing a change to one, and when one is hard to read but it is not obvious why. Not for doc/, AGENTS.md or the skills.
---

# Writing the documents

These documents are read by people who have not seen the code. Each one
explains a mechanism in an order you can follow from the top.

[reference.md](./reference.md) holds the rules. Read it before writing or
restructuring a document, or when one reads badly. For an edit that keeps the
order, the checks below are enough.

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
- **No minute is linked.** A document cites the code, another document or an
  outside source, never a record under `doc/`.
- **Prose wraps at 80 columns** in `README.md`, `ARCHITECTURE.md`, xpute's two
  and `infra/README.md`, by hand: break after the last word that fits in 80
  columns. Rewrap only the paragraphs you touched.
- **Read the whole document from the top**, rendered, not as a diff.
