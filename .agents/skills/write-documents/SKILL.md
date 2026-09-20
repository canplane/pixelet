---
name: write-documents
description: How the repository's prose documents are written and reviewed — README.md, ARCHITECTURE.*.md, issue/ and the skills. Use before writing or reworking one, and when a document is hard to read but it is not obvious why.
---

# Writing the documents

The documents are read by someone who has not seen the code. They are not
reference cards and not essays: each one explains a mechanism in an order
that can be followed from the top. This is what to hold them to.

`deno fmt` runs with `proseWrap: preserve`, which means it keeps whatever line
breaks you wrote and imposes none. The convention is therefore per file, and
the rule is to match the file's neighbours: `README.md` and the two
`ARCHITECTURE` documents run a paragraph to one line, while `AGENTS.md`, the
skills and `issue/` wrap at about eighty columns. Reformatting a document to
the other convention makes a diff of the whole file for no reason.

## Readability is the goal, and length is only a symptom

Do not steer by word count, sentence count or an average. Those numbers move
when you cut a sentence that was already fine and stay put when you leave the
one that makes people read it twice.

A long sentence is worth looking at, but look at _why_ it is long. The ones
that cost a reader something are the ones where an insertion splits a pair
(`declares its policy — sixteen words — and what its falling edge answers
with`), where a parenthetical nests inside another, or where a pronoun points
at two possible things. A forty-word list of fields reads fine as a list.

## Earn a name before you use it

Every document here coins nouns — a ring, a strobe, a handle, a band, a run.
The failure is dropping one in as the subject of the sentence that first
mentions it, leaving the reader to work backwards to what it is for:

> The host's clock for one guest is the **strobe** (`sched/strobe.ts`): a turn
> on an animation frame, and only when something asked for one.

Write the need first, and let the name land on the sentence that answers it:

> On the host side, something has to decide when to ring at all. That is the
> **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only
> when something asked for a turn.

Never introduce a term inside a parenthetical or an em-dash aside. An aside is
the mark of something a reader may skip, and a definition is not skippable.

## Order by dependency

Nothing may be used before it exists. This governs the order of sections, not
just of sentences: a catalogue of module names belongs at the end, once each
name means something, and the vocabulary of a model belongs before the machine
that operates on it.

The check is mechanical. For each coined term, find its first use and its
definition; the first use must not come earlier. Section headings and the
document's opening sentence are exempt — a heading naming its own subject, or
a first line calling the thing what the world calls it, is not a forward
reference.

## One word, one meaning

Words go double without anyone noticing. In one document `lane` was both a
field packed into a word and a structure-of-arrays column; `frame` was a drawn
frame, an animation frame and a frame of reference; `page` was a level of the
hierarchy and a browser tab; `bake` was the offline terrain bake and the second
stage of a chunk build.

Where the code itself uses one word for both, do not rename it in the
document. Qualify it where it is defined — `column lanes`, `a frame of
reference` — and let the qualifier do the work silently from then on.

## A diagram shows what prose cannot

Before drawing, ask what the picture carries that a sentence would not. Two
things that are not diagrams:

- **A glossary with borders.** Boxes of label-and-description, one per term.
  Delete the box-drawing characters and nothing is lost, which means nothing
  was there.
- **A type signature in a code fence.** `PVF3 = { i: IVec, f: FVec }` names
  three things and shows none of them.

Draw a shape: a bit layout across two words, a flow from a provider's rows to
pixels, which end of a ring each side writes, what happens between two edges of
one call. A good diagram takes work off the prose — one of these replaced a
sixty-nine-word sentence with a twenty-five-word one.

A diagram must not say the sentence that introduces it. If the prose claims
that what is drawn is what is hit, the picture's job is to show one set of
data going two ways, and its branches should name what comes out of each —
not repeat the claim in the same words. Watch for an asymmetry there: a fork
where one branch names an output and the other states a moral is a fork with
a sentence stuck to it.

Diagrams are hand-aligned ASCII. Build them with a short script rather than by
eye; a column that is one character out is invisible in review and obvious on
the page. Keep them under about 78 columns.

## Each document says what is its own

A document that sits over another one (pixelet over xpute) must not re-explain
the layer below. Say what is inherited, link it, and move on. Three of five
headline properties in one README belonged to the layer underneath, which left
no room for what the project itself does.

The same rule inside a document: if a section has already said the turn is
somebody else's, it should not then draw that turn.

A file that is assembled at build time still has to read as a file. One
README here has a provenance section spliced in on export, and the licence
paragraph under it said "the copy carries the licence" — a phrase whose
antecedent only exists after the splice. In the file itself it pointed at
nothing.

## What a restructure breaks

Moving sections invalidates everything that describes the document, and those
are the last things anyone rereads. After a restructure, go and check:

- **Counts and summaries.** "It is in two parts" after splitting into three;
  "two documents carry the infix" after making it four; a table row describing
  a task that has since grown.
- **The sibling documents.** A word fixed in one is usually wrong in the
  others. `page` meant both an L3 page and a browser tab; it was corrected in
  the architecture and left standing in the README for another two rounds.
- **Forward references you created.** Putting a section earlier can push a
  term in front of its own definition, which is the fault the move was meant
  to fix.

## Cut what restates

After the first draft, look for sentences that carry no fact:

- a sentence explaining the sentence before it
- an inference laid on top of a fact the reader can draw themselves
- a clause about the benefit of what was just described
- scaffolding — "which is the next section", "note that"
- a claim the diagram above it already makes

Motivation earns its words. Commentary does not. One rewrite added 58% and
came down to 48% purely by removing this, with no fact lost.

## Rewrite rather than patch

When the order is wrong, moving sentences inside it keeps it wrong. Take the
sections apart, put them in dependency order, and write the joins. Patching is
for a document whose order is already right.

## Before committing

- Every code path, identifier and constant in the old version is still
  present. Diff the set of backticked tokens; a rewrite loses `-errno` or
  `16 bits` without anyone noticing.
- No coined term is used before it is defined.
- `deno fmt --check` on the file.
- Read the whole document from the top, on the page rather than in a diff.

The last one is not a formality, and it is not interchangeable with the
others. Every check above finds a class of fault it was written for and no
other. A repetition eight lines apart slips a similarity test when the
sentences around it differ; a diagram one column out looks fine in a diff; a
pronoun with nothing to point at parses cleanly; a count that has gone stale
is still a number. Worst of all, a document that defines nothing passes the
defined-before-use check perfectly, because there is nothing for a use to be
early of. Reading is what finds those, and there is no substitute for reading
all of it.
