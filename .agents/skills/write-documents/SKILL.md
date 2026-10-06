---
name: write-documents
description: How the repository's prose documents are written and reviewed — README.md, ARCHITECTURE.*.md, decision/ and the skills. Use before writing or reworking one, and when a document is hard to read but it is not obvious why.
---

# Writing the documents

The documents are read by someone who has not seen the code. They are not
reference cards and not essays: each one explains a mechanism in an order
that can be followed from the top. This is what to hold them to.

## Lead with the core

Write as if pitching the project: the sooner the thing lands, the better the
document, and everything else here serves that. Someone who has not seen the
code should be able to say what this is after the first screen. The test of
any paragraph, picture or heading is whether it got them there sooner.

That is what Occam's razor means here. Find the claim the rest follows from,
put it first, and let the consequences come after it. xpute's README opened on
the turn — seven invented names at once — when the core was that the interface
is three edges and no others. Turns are a consequence of that narrowness, not
the other way round. Moving the narrow picture to the front let the README
drop all seven names.

Plain words carry a reader faster than precise-sounding ones. Where a plainer
word is about as short and just as accurate, it wins: the document exists for
someone who does not know the subject yet.

The masthead is not the body. A tagline, a date, a play link, a screenshot sit
above the argument the way a title page does, and a reader passes over them.
Counting them as delay and moving them down costs what they were doing and
buys a reader nothing.

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
mentions it, leaving the reader to work backward to what it is for:

> The host's clock for one guest is the **strobe** (`sched/strobe.ts`): a turn
> on an animation frame, and only when something asked for one.

Write the need first, and let the name land on the sentence that answers it:

> On the host side, something has to decide when to ring at all. That is the
> **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only
> when something asked for a turn.

Never introduce a term inside a parenthetical or an em-dash aside. An aside is
the mark of something a reader may skip, and a definition is not skippable.

Only a name this tree invented needs earning. `strobe`, `doorbell`, `ring`,
`change`, `handle`, `band`, `run`, `XTP` and `PVF3` were made here, and nobody
can look them up. `arena`, `deque`, `errno`, `buddy`, `slab`, `golden records`,
`virtio` belong to the world, and a reader either knows one or can find it.
Explaining those is padding. Paraphrasing one away is worse: `recorded output`
says less than `golden records` and cannot be checked against anything.

Where a borrowed term is genuinely hard, name it and move on rather than
unpacking it — a document that stops to teach its dependencies never gets to
its own subject. Where a plainer word is as precise, prefer the plainer one.

## Order by dependency

Nothing may be used before it exists. This governs the order of sections, not
just of sentences: a catalog of module names belongs at the end, once each
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
frame, an animation frame and a frame of reference; `page` was a square of the
pyramid and a browser tab; `bake` was the offline terrain bake and the second
stage of a chunk build.

Where the code itself uses one word for both, do not rename it in the
document. Qualify it where it is defined — `column lanes`, `a frame of
reference` — and let the qualifier do the work silently from then on.

## A diagram shows what prose cannot

Judge a figure the way a paper does. A **figure** when the thing has a shape a
sentence cannot hold — a layout, a flow, which end writes what. A **table**
when the content is values a reader looks up rather than reads. **Prose** for
the rest. A picture that is neither a shape nor a lookup is decoration, and
decoration in a document someone has to search is a cost.

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

Drawing is also a check on the prose. A diagram cannot hand-wave: it has to
commit to numbers, and to which thing points at which. Putting the coarse-cell
rule into a picture turned up a sentence that said "quarter-size" where the
code takes a cube root and the answer is half.

An analogy is a picture too, and it must not argue. Its job is to hand the
reader a shape they already hold: one clause, attached to the sentence it
illustrates. Add a second clause defending the comparison and it stops being a
picture and becomes a claim — one that then has to survive the difference
between the two things. A line here said another editor reserves memory the
same way and then explained that it does so for a reason that does not apply,
which left a name and no picture. Cut back to the clause, it works again.

## Each document says what is its own

A document that sits over another one (pixelet over xpute) must not re-explain
the layer below. Say what is inherited, link it, and move on. Three of five
headline properties in one README belonged to the layer underneath, which left
no room for what the project itself does.

A document can only defer to one its reader will open. AGENTS.md is written
for agents; a README cannot send a human there and call the subject covered.
Two statements of one rule at two depths — a paragraph for someone browsing
the tree, the full rule for whoever writes code — are not duplication, because
no single reader meets both. Ask who opens the file before treating a pointer
to it as coverage.

The same rule inside a document: if a section has already said the turn is
somebody else's, it should not then draw that turn.

A file that is assembled at build time still has to read as a file. One
README here has a provenance section spliced in on export, and the license
paragraph under it said "the copy carries the license" — a phrase whose
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
- a fact the table beside it already carries

A table and a figure are content, not decoration, so read them as part of the
section before writing a line near one. A README here ended a section with
"WebGPU only" three rows under a table whose Rendering row said WebGPU.

Motivation earns its words. Commentary does not. Removing only this from one
rewrite cut a fifth of its length, with no fact lost.

## Name what outlives the document

A README describes a system, not a deployment of it. A hostname, a device
model, a version floor are facts about today's arrangement: they go stale
without anyone noticing, and a reader who has a different one is told
something false. The masthead is where today belongs — a play link, a date,
a badge — because a reader takes it as current by construction.

Two lines here named today in the body. One made an iPhone 11 sound like a
design input when the decision was WebGPU and the phone was a consequence;
the other put the domain at the top of a diagram whose subject was that every
request arrives at one Worker. Both said less than the thing they displaced.

Ask what the sentence would have to say if the fact changed. If the answer is
"nothing" — the structure holds either way — the fact was never the point.

## A fact the code holds is not kept by hand

A document that states what the code derives — a value generated from
`spec/`, a path, a name, a size, a count, an offset — must not become a
second place that fact is kept. One day here set the memory map's numbers by
hand twice, and a third change was already coming; a sweep found functions
cited under paths that stayed after the functions were renamed.

So a derived fact goes where a check already reaches it: a value from
`spec/` is generated, a path and a name cited right after it
(`world/query.rs` `first_hit`) are held by `check:rules`, and a size is
said by what it is derived from, never copied as a number into a document.
Before adding a table of numbers
or a citation, ask whether an existing check can hold it, and cite in the
form it reads. A new mechanism is built only for drift that has recurred,
and as far as it recurred.

What a check cannot hold is not generated either: who owns what, what a
failure means, where an abstraction ends, why the structure is what it is
and what decided it. Those are explanations, kept by a person who reads
them. A check or a generator only stops a document contradicting a fact the
code already has; it never defines the meaning or the contract the document
is there to state.

## Symmetry is not a reason

Every rule above pushes toward consistency, and applied without judgment that
becomes its own fault. A section does not need an opening because its siblings
have one; it needs one when its point is buried or when the first thing a
reader meets has no frame. Two parts of one document here earned an opening —
one began on a twenty-five-line graph, the other kept its thesis sixty lines
down — and the third did not, because it begins on a table, which explains
itself. Adding one there would only have repeated the map in the introduction.

Ask what a reader gains, not what the shape would look like.

## Rewrite rather than patch

When the order is wrong, moving sentences inside it keeps it wrong. Take the
sections apart, put them in dependency order, and write the joins. Patching is
for a document whose order is already right.

## Mechanics

`deno fmt` runs with `proseWrap: preserve`, which means it keeps whatever line
breaks you wrote and imposes none. The convention is therefore per file, and
the rule is to match the file's neighbors: `README.md` and the two
`ARCHITECTURE` documents run a paragraph to one line, while `AGENTS.md`, the
skills and `decision/` wrap at about eighty columns. Reformatting a document to
the other convention makes a diff of the whole file for no reason.

Diagrams are hand-aligned ASCII. Build them with a short script rather than by
eye; a column that is one character out is invisible in review and obvious on
the page. Keep them under about 78 columns.

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
