---
name: reduce
description: How a structural audit and cleanup is run — finding where the tree strays from the main axis or keeps a fact twice, ranking what to remove, and landing it beside other agents. Use when asked to audit, tidy or reduce the tree, or before a refactor whose point is a simpler structure. The principles it applies are AGENTS.md's "The main axis" and "Architecture first".
---

# Running a reduction

The principles are AGENTS.md's: "The main axis" says what the tree should
look like and how a reduction is valued, "Architecture first" how a change
is made. This is the procedure that applies them, the same each time.

## 1. Find

Look for what a reduction removes; each finding names the files it is in.

- **Edges up the axis.** List what each top-level module names of the
  others anywhere, not only in `use` lines (`git grep -h "crate::<other>" --
  <module>`, inline paths included), and mark every edge that points up:
  a layer naming the view, io naming a layer, anything below the scene
  naming it. The profiler is exempt; it observes.
- **Reads past the query.** Outside a layer, any use of its storage — its
  store, its residency's tiles, its lanes — rather than the query's bound or
  record.
- **One fact kept twice.** Two lists, numbers or rules that must agree. The
  tell is whatever holds them equal: a test or a `const` assert comparing
  them, a comment saying one follows the other, a value copied each turn
  and compared.
- **One fact said in two ways.** The same quantity in two units, or one
  question answered by two code paths with their own rules.
- **Dead and test-only items.** Something nothing calls, or only a test.

## 2. Judge

For each finding, say before touching it:

- what disappears — the state, owner, name, path or copy;
- whether the code then points further down the axis;
- whether a fact ends with fewer owners.

A finding where nothing disappears is worth little and is left. Prefer the
deletion that makes the problem impossible to the machinery that manages it.
Rank what is left by the order in "The main axis", and say plainly where the
first fix you thought of is not the smallest state space.

## 3. Agree

Changes of shape go to the other agents and to the user before they are
written.

- Send the ranked findings to the agents working in the tree for adversarial
  review: is each real, is the fix the smallest state space, what was missed.
- Split the work by where it lies, and say which files each agent opens.
- A choice between designs goes to the user with each side's state space and
  cost; what the user has decided is carried through, not reopened.

## 4. Land

- Keep behaviour, or say in the commit what it trades. Where the change
  could cost, measure against the commit before and explain any difference
  by its mechanism.
- Where the output is deterministic, prove the change by what it produces:
  old against new, hashed per unit (a leaf, a shard), over real data, before
  landing. Equality is the cheapest proof and what makes a fold safe.
- Find a cost by narrowing on real data: each statement, then each part of
  the slow one, then the rows it touches, rather than by reading the code.
- Confirm behaviour by running the app before running the tests; a test in
  the structure's way is rewritten against the new interface or deleted.
- Read every file's diff before committing it. A path commits whole, so
  another agent's hunk in it goes in your commit: name it in the body, or
  ask them to commit first.
- Send each commit to the other agents for review, saying what to attack
  first.
