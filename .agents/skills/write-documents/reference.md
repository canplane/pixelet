# What a document should be

The rules behind the checks in [SKILL.md](./SKILL.md).

## Lead with the core

Someone who has not seen the code knows what this is after the first screen.
Judge every paragraph, picture and heading by whether it gets them there
sooner. Put first the claim the rest follows from.

Prefer the plainer word when it is as short and as accurate.

The masthead (tagline, date, play link, screenshot) stays on top.

## Readability is the goal, length only a symptom

When a sentence is long, find what costs the reader: an insertion that splits a
pair, nested parentheticals, a pronoun with two possible referents.

## Earn a name before you use it

Say the need first, then name what meets it:

> On the host side, something has to decide when to ring at all. That is the
> **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only
> when something asked for a turn.

Introduce a term in a main clause, never in a parenthetical or a dash aside.

Only names invented here need this (`strobe`, `doorbell`, `ring`, `handle`,
`band`, `run`, `XTP`). World terms (`arena`, `errno`, `slab`, `golden records`)
are used as they are.

## Order by dependency

Nothing is used before it is defined, sections included: a catalog of module
names goes at the end, a model's vocabulary before the machine that uses it.

## One word, one meaning

Watch for words used in two senses (`frame` as a drawn frame and a frame of
reference; `page` as a pyramid square and a browser tab). Where the code uses
one word for both, qualify it where it is defined (`a frame of reference`).

## A diagram shows what prose cannot

Use a **figure** for a shape (a layout, a flow, which end writes what), a
**table** for values a reader looks things up in, and **prose** for the rest.

Diagrams are hand-aligned ASCII under about 78 columns, built with a short
script. A diagram adds to the sentence that introduces it; its branches name
what comes out.

An analogy gets one clause.

## Each document says what is its own

A document over another layer (pixelet over xpute) says what it inherits, links
it, and moves on. It defers only to a document its reader opens: a README sends
a person to another README or to ARCHITECTURE.md, never to AGENTS.md.

ARCHITECTURE.md holds the structure: the parts, the rule each follows, and what
crosses between them. How one module works goes in its doc comment or the
README beside its code, and ARCHITECTURE.md points there.

A file assembled at build time reads correctly on its own.

## Cut what restates

Remove sentences that carry no fact: one explaining the sentence before it, an
inference the reader can draw, a clause about the benefit of what was just
described, scaffolding ("note that"), and claims a nearby diagram or table
already makes.

## Name what outlives the document

A README describes the system; hostnames, device models and version floors go
in the masthead. Ask what a sentence would say if its fact changed.

## A fact the code holds is generated or derived

A value from `spec/` is generated from it; a size, a count or an offset is
stated by what it is derived from; code is cited as a path, then a name. Before
adding a table of numbers or a list of citations, ask what will keep it true.

## Add a section opening where it is needed

A section gets an opening when its point is buried or the first thing a reader
meets has no frame.

## Rewrite rather than patch

When the order is wrong, take the sections apart, put them in dependency order,
and write the joins.
