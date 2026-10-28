# What a document should be

The rules behind the checks in [SKILL.md](./SKILL.md).

## Lead with the core

Someone who has not seen the code should know what this is after the first
screen. Judge every paragraph, picture and heading by whether it gets them there
sooner. Find the claim the rest follows from and put it first. xpute's README
once opened with seven invented names; leading with its core, that the interface
is three edges and no others, made all seven unnecessary.

Prefer the plainer word when it is as short and as accurate.

The masthead (tagline, date, play link, screenshot) is not the body; leave it on
top.

## Readability is the goal, length only a symptom

Don't steer by word or sentence counts. When a sentence is long, look at why: an
insertion that splits a pair, nested parentheticals, or a pronoun with two
possible referents cost the reader. A long list of fields reads fine.

## Earn a name before you use it

Don't make a term this tree coined the subject of the sentence that first
mentions it. Say the need first, then name what meets it:

> On the host side, something has to decide when to ring at all. That is the
> **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only
> when something asked for a turn.

Never introduce a term in a parenthetical or a dash aside; asides are skippable,
definitions are not.

Only names invented here need this (`strobe`, `doorbell`, `ring`, `handle`,
`band`, `run`, `XTP`). World terms (`arena`, `errno`, `slab`, `golden records`)
are used as they are: don't explain them, and don't paraphrase them away.

## Order by dependency

Nothing is used before it is defined, sections included: a catalog of module
names goes at the end, a model's vocabulary before the machine that uses it. A
heading naming its own subject, and an opening line using the thing's common
name, are exempt.

## One word, one meaning

Watch for words used in two senses (`frame` as a drawn frame and a frame of
reference; `page` as a pyramid square and a browser tab). Where the code uses
one word for both, don't rename it; qualify it where it is defined (`a frame of
reference`).

## A diagram shows what prose cannot

Use a **figure** for a shape (a layout, a flow, which end writes what), a
**table** for values a reader looks things up in, and **prose** for the rest. A
glossary in boxes or a type signature in a code fence is not a diagram.

Diagrams are hand-aligned ASCII under about 78 columns; build them with a short
script, since a column one character off is invisible in review. A diagram must
not repeat the sentence that introduces it; its branches name what comes out.
Drawing also checks the prose: a picture has to commit to numbers.

An analogy gets one clause. A second clause defending it turns it into a claim.

## Each document says what is its own

A document over another layer (pixelet over xpute) says what it inherits, links
it, and moves on. A document can only defer to one its reader will open: a
README cannot send a person to AGENTS.md.

ARCHITECTURE.md holds the structure: the parts, the rule each follows, and what
crosses between them. How one module works belongs in its doc comment or the
README beside its code, and ARCHITECTURE.md points there.

A file assembled at build time must still read correctly on its own.

## What a restructure breaks

After moving sections, check:

- **Counts and summaries** ("in two parts"), and table rows for a task that has
  grown.
- **Sibling documents** that use the same word.
- **Forward references** the move created.

## Cut what restates

Remove sentences that carry no fact: one explaining the sentence before it, an
inference the reader can draw, a clause about the benefit of what was just
described, scaffolding ("note that"), and claims a nearby diagram or table
already makes. Motivation earns its words; commentary does not.

## Name what outlives the document

A README describes the system, not today's deployment. Hostnames, device models
and version floors go stale; keep them in the masthead. Ask what the sentence
would say if the fact changed; if nothing, the fact was never the point.

## A fact the code holds is not kept by hand

Don't make a document a second copy of something the code derives (a value from
`spec/`, a size, a count, an offset). Generate values from `spec/`, state a size
by what it is derived from, and cite code as a path then a name so a search
finds it when it moves. Before adding a table of numbers or a list of citations,
ask what will keep it true.

What no check can hold (who owns what, what a failure means, why the structure
is what it is) is the document's real job, kept by a person reading it.

## Symmetry is not a reason

Don't add a section opening because its siblings have one; add it when the point
is buried or the first thing a reader meets has no frame.

## Rewrite rather than patch

If the order is wrong, moving sentences around keeps it wrong. Take the sections
apart, put them in dependency order, and write the joins.
