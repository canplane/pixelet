# Ten ways to keep the camera out of the ground, and why nine were wrong

**2026-09-09** — the orbit camera could travel under the terrain, where the
shell has no inward-facing geometry and the scene reads inside out. Ten attempts
went into fixing that. Only the last survives, and the nine before it are worth
writing down because each looked correct until it ran.

## The constraint that killed most of them

The camera is an orbit rig: `dist`, `polar`, `azimuth`, with `lookAt` pinned on
the target. That last part is what makes the obvious fixes wrong — **moving the
eye is moving the angle.** There is no way to nudge the camera out of the ground
that does not also change where it is pointing, and a camera that picks its own
angle is worse than one that occasionally shows a hillside. The user's own
phrasing settled it: the camera must not adjust angles on its own, and rendering
must not fake what the controls say.

## What was tried

**Lift the eye out of the ground.** Sample terrain under the eye, raise it to
clear. Works, and the whole view tilts while `polar` says otherwise — the scale
bar and the picture disagree.

**Lift until the sightline clears.** Raise until the segment from eye to target
is above ground. On terraced terrain the sightline runs within a couple of cells
of the surface for its first stretch, and something is nearly always a step
higher there. This refused almost every gesture.

**Spring arm.** Sweep from target toward the eye, shorten `dist` to the first
hit, damp back out when clear. This is the conventional answer in third-person
games and it was rejected on feel: the camera oscillates, and worse, shortening
`dist` means the zoom is no longer what the user set. Pushing back out also
invites a feedback loop — the camera moves, the ground under it changes, the
solve changes.

**Refuse the step outright.** Correct, and it made one rule behave as two. A
drag is a stream of tiny steps, so refusing each one that enters a hill reads
naturally as "it stopped at the hill". The scale bar is a single jump to a named
zoom, and refusing that leaves a button that does nothing, with no feedback and
a legitimate zoom that can never be reached.

## What survived

Terrain gets exactly one question — _would this view state put the eye inside
it_ — and `camera/terrain_gate.ts` answers that and only that.

**It is a pure function of `dist`, `polar`, `azimuth`,** not of the live scene
graph, so any input path can ask before anything is adopted.

**It sits in `set_view_state`.** The first working version sat at the damping
layer instead, and the scale bar moved while the view stood still: a zoom that
could not take effect still wrote `dist`, and the scale bar reads `dist`. Gating
the control rather than the render is the difference between refusing a move and
pretending to make one.

**It asks about the eye, not the sightline.** See above — the sightline test is
the one that refused everything.

**It samples the whole path.** Where a step lands is not enough, because damping
visits everything between two states and a gesture can cross a ridge in a single
event. The path is the straight line in control space, which is exactly what the
damping follows, so eight points along it are tested with azimuth taking the
short way round.

**It clamps rather than refuses.** The gate returns the furthest clear point
along the way and the store adopts it: go as far as you asked, or as far as you
can. It stops at the last clear sample rather than at the crossing point, since
landing exactly on the clearance leaves the next frame's rounding to decide
which side the eye is on.

**A destination whose tile has not loaded is not blocked.** Refusing on data
that has not arrived is refusing on a guess.

**Being inside terrain releases everything.** The character walks and the world
scrolls under a camera that was not moving, which no refusal can prevent. Once
the eye is inside, every control is free again, or there is no way out.

## Why it is a setting, and what `climb` is

Whether terrain should constrain the camera at all is a taste question, so
`Settings > Camera` asks it as one choice rather than three switches:

    through — nothing. The eye goes wherever the controls put it.
    stop    — a control goes as far as it asked or as far as it can.
    climb   — dist is always granted; the view pitches up instead.

`climb` is the one thing here that does move an angle, and it is opt-in for that
reason. It grants every control and answers terrain at render time by raising
the _drawn_ pitch toward straight down until the eye clears; `view_state.polar`
is never written, so the pitch relaxes on its own once the ground does and
`dist` stays exactly what the scale bar reads. The trade is explicit: every zoom
stays reachable — which `stop` cannot promise, since a hill close to the
character caps how far in you can go — at the cost of an angle nobody asked for.

The solve is closed-form. The eye sits `dist * cos(polar)` above the target, so
clearing a required rise `h` needs `acos(h / dist)`. A bounded scan was tried
first and juddered on ground it was only passing over: sixteen fixed angles
quantize the answer a second time, after the cell the height was read from. Two
passes settle the implicit part (steepening moves the eye, which changes the
ground under it), since the eye travels less as the pitch approaches vertical.

Feedback is impossible by construction: the probe reads `dist`, `polar` and
`azimuth`, and the climb moves none of them.

## What this does not solve

Rotation into a hillside is handled the same way as everything else — the
azimuth destination is gated — but "there is a hundred-story building to my
left" has no good answer beyond stopping. That was accepted rather than solved:
the edge cases of continuing to rotate past an obstruction are not worth
carrying.
