---
name: reshape
description: How a change to the code's structure or speed is found, proposed and proven — the whole pipeline mapped, a concept held in several forms made one across layers and kinds, no option that breaks a seam, equal output over real data the proof, one timing at the end. Use before proposing or making an optimization or a restructuring, and before naming options for any fix.
---

# Reshaping

Clean structure comes first. Speed is earned by reshaping the whole, never by
picking off one hot spot: a greedy fix, an ad hoc addition, makes the code
worse with each one (the user: "구조적으로 통째로 최적화를 해야지").

## Finding it

- Map the pipeline before changing it: each pass, the concepts it holds, the
  seams it goes through.
- Look for a concept held in more than one form, or one computation done
  again, across layers and kinds, and make it one; the work falls away with
  it. The fill's runs were a second form of the kept spans: three dispatches
  and a bake budget went with them (720052574).
- Read the code for it. A timing finds no structure.
- A change that adds machinery, a special case, a toggle, a buffer or a
  kernel, without removing a concept elsewhere, is suspect.

## Proposing it

Every option stays inside the layering (AGENTS.md; `cargo check-rules` holds
its order). One that breaks a seam, a way around the world's one query or a
layer reading another's storage, is no option: it is never listed, ranked or
offered as the alternative. Where only a breach would be fast, say so, and
offer what the structure allows.

## Proving it

- By equal output over real data, not by a timing: for the ground's fill,
  `fill_digest` ([verify-change](../verify-change/SKILL.md)).
- A timing comes once, at the end, against the backlog's baseline: the commit
  before and the change each served from its own export on its own port,
  side by side, in a GPU window the sessions agreed. Never a window to
  compare variants or chase a span.
