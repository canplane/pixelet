---
name: reshape
description: How a change to the code's structure or speed is found, proposed and proven — the whole pipeline mapped, a concept held in several forms made one across layers and kinds, no option that breaks a seam, equal output over real data the proof where a tool shows it, one timing at the end. Use before proposing or making an optimization or a restructuring, and before naming options for any fix.
---

# Reshaping

Clean structure comes first. Speed is earned by reshaping the whole, never by
picking off one hot spot: a greedy fix, an ad hoc addition, makes the code
worse with each one; the user asks that optimizing be structural and whole.

## Finding it

- Map the pipeline before changing it: each pass, the concepts it holds, the
  seams it goes through.
- Look for a concept held in more than one form, or one computation done
  again, across layers and kinds, and make it one; the work falls away with
  it. The fill's runs were a second form of the kept spans: three dispatches
  and a bake budget went with them (720052574).
- Read the code for it. A timing finds no structure, and a minute is
  reference, not what binds today (`doc/README.md`).
- A change that adds machinery, a special case, a toggle, a buffer or a
  kernel, without removing a concept elsewhere, is suspect.

## Proposing it

Every option stays inside the layering (ARCHITECTURE.md; `cargo check-rules`
holds its order). One that breaks a seam, a way around the world's one query or a
layer reading another's storage, is no option: it is never listed, ranked or
offered as the alternative. Where only a breach would be fast, say so, and
offer what the structure allows.

Every option is symmetric across what the world holds: the ground and what
is placed are one kind from two sources (ARCHITECTURE.md, "What fills a
cell"), and nothing drawn may tell one from the other. An option that acts on
one source alone, a shape or a fact only the ground produces, breaks that
symmetry and is no option either. The touch rule placed coarse cells by bits
only the ground set, so a terrace of placed cubes would look otherwise than
the ground's; it was built across three sessions and reverted whole.

An option is weighed by whether it is right, never by what changing to it
costs: how many places it touches, how far the present form has spread, how
much is built on it. The cheapest change is none, so a cost-weighed proposal
always ends at leaving it. The present form has no standing of its own.
And the answer is one: the right form, stated as the decision, or the reason
it cannot be decided yet. A middle form that keeps part of the wrong one is
not offered as a virtue.

## Proving it

- By equal output over real data, not by a timing, where a tool shows it: for
  the ground's fill, `fill_digest` ([verify-change](../verify-change/SKILL.md)).
  Where none does, by a test that fails when the change is wrong and by its
  generated files being what their sources make, and the proof says so.
- A timing comes once, at the end: the commit before and the change each
  served from its own export on its own port, side by side, in a GPU window
  the sessions agreed. Never a window to compare variants or chase a span.
