# A second rule beside perspective

**2026-11-17.** Written by the agent (Claude) that did the work, at the user's
word that what is worth keeping of the day be kept. The case is recorded in
[the location zone, and the observation quantized](../minutes/20261117-the-location-zone-by-the-codes-own-axioms.md)
and its figures in
[the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md)
("Then: a pan past the service area" and what follows it).

## The rule the user holds

Everything that reads the world reads it by perspective, the same in every
direction: the farther from the eye, the coarser its cells, wherever a ray
starts. The eye is the camera's and there is no other; it sets the size a place
is asked at, and the answer is the world's
([the eye sets a size, and the world answers](../minutes/20261118-the-eye-sets-a-size-and-the-world-answers.md)).
It holds for the camera, the light, the shadows and what collides alike, and
what a ray does not find within its cap of steps is not there, as in a fog. The user has
said so of the camera and the light for more than a month. It is in `AGENTS.md`
since this day.

## What the code held instead

A second rule grew beside it, for what collides and for the camera's way: read
the world at the finest zoom held, at every distance.

| when | commit | what it did |
| --- | --- | --- |
| 2026-10-24 | `e375fb94a1` | the walker's cell reader casts its rays at one zoom |
| 2026-11-02 | `503e8e40cd`, `50a90dd012` | a detail that reads as finely as is held (`finest`), and the orbit's rays read by it |
| 2026-11-05 | the shaders by hand | the engine's walk on the CPU ends at the cap of steps, as its WGSL twins do |
| 2026-11-08 | `3df6881f37` | `ARCHITECTURE.md` says a body stands on the world at its finest held zoom, not the eye's |
| 2026-11-09 | `5b054bb5fd` | the walk loses the eye: one rule near and far, a leaf cell at a time |

No one of these was wrong where it stood. The fixed zoom is cheaper near, a node
asked for one zoom's bound where a ray by distance asks every zoom's within it:
a near walk of 27 leaf cells takes 302 µs so and about 335 by distance. It was
fitted to the near case. Its cost grows with the distance without end, and the
cap of 2026-11-05, right in itself, met it: a ray at the finest zoom through
more than about 2040 cells of solid ran out of steps and was read as having met
nothing. The sentence of 2026-11-08 then made the second rule read as a rule,
and every agent after it took it for one.

## What it cost that day

- A pan from the orbit's largest distance past the service area walked tens of
  thousands of leaf cells a frame: 100 to 560 ms.
- The floor under a point deep in the ground answered the world's top, 32768.
- The camera's long rays ended at the cap and read as clear.
- A climb where something is placed at the finest zoom took 57 ms from 2000
  cells down.

By distance, the climb takes 0.14 to 0.35 ms from 100 to 30,000 cells down, and
the answers are right.

## How the agent went round it

The agent met the fault eight times and laid a piece beside it each time before
it took the rule:

1. a walk that crosses an empty shard in one step (`18598cc6e8`), for one of two
   cases;
2. the rule of 2026-10-11 put back as it was;
3. a walk over the cells of the zoom drawn;
4. the ground's leaf zoom as the edge, then an angle of the walker's own size,
   then the clip level's reach;
5. the finest detail kept and the ray's length held to a count of cells derived
   from the cap, with a loop over reaches and an early end;
6. the rays by distance, and then taken back by the agent on its own judgment,
   because two tests failed and a near walk was a tenth slower;
7. one function for every ray of control (`World::near`), which read what it met
   far off again from nearer inside one query, a zoom finer a pass;
8. a reader that asked by its distance from where it stood, and after it an eye
   for whatever had none: a place the camera is sent to read as an eye standing
   there, a pan leading an eye of its own.

Each piece closed what was in front of it and left a second rule standing. The
last two were second rules themselves, each a size that something other than the
eye set. The sixth is the one to keep in mind: the general rule failed two tests, and the
agent read that as the rule's fault. Both failures were the old code's. A ray by
distance reported its coarsest zoom's cells as read, 2048 units a side for one
point, whatever it had read; and the tests held answers that only the fixed zoom
gives. The user refused the retreat. With the report made right where it is
written, the rule held and the pieces of the fifth were not needed.

The agent also called the tenth a regression. It is the price of the general
rule where a special one had been fitted, and the special one had bought the far
case's wrong answers with it.

## What to keep clear of

- **A reader of the world with a zoom, a length or a constant of its own.** It
  is a second rule. Look for `finest`, a zoom handed down as a number, a reach
  that is not the ray's own.
- **A sentence in a contract that says where the rule does not reach.** "Not the
  eye's" was such a sentence. Ask whose decision it records before reading it as
  one.
- **A fix whose size grows.** A count, a loop over the count and an early end
  for the loop are three pieces for one fault. The rule that already holds
  elsewhere would have been one.
- **A general rule that fails where a special one passed.** Read what the
  special one was leaning on before reading it as the general one's fault: a
  report made for the special case, a test that holds the special answer.
- **A cost called a regression against a form that was fitted.** Say what the
  fitted form paid elsewhere, and measure both ends.

## How to check

- For any read, ask what decides the size it asks at: the eye alone, by its
  distance, or something else. The eye's own rays ask each node at its own
  distance from the eye; a body asks its cell at the one size the eye sets for
  the body. With no eye a read answers a lack.
- What is met far from the eye is a coarser cell, and that is the answer.
  Nothing reads it again from nearer.
- A walk's steps should not grow with how far it goes. Time a query from near
  and from very far: by distance the two are of one order.

## The last of it

The orbit's rays (`orbit.rs` `line_clear`, `toward_eye`) and the pointer's
(`pick.rs`) read by `finest` for a few hours more. They are the one walk the
walker's are, and `finest` is gone from the code.

## The day, as it went

A diary, since the talk it came from will not be kept.

The day began with a lag: a drag over the sea, far from any served ground, froze
the frame. I measured it, found the walk, and made the walk hop over empty
shards. It worked and I committed it. The user asked whether the bound had once
been closed and someone had opened it. It had: in October a pan far out did not
walk at all. I put October's code back and felt I had restored something.

The reviewer found October's code had the same hole, smaller. I chose which of
two camera states the test should read. The user asked whether I knew what a
reduction is. I did not answer that well. I had been choosing between forms
where I should have been asking what the one rule was.

Then the user said a mesh in the pyramid has the pyramid's dynamics, as a placed
cube has the ground's, and it was one sentence and it closed four of my
attempts: the walker is a cell, it stands where a decor of that cell is drawn,
and the character and its walk and the chat's line follow from that.

The cap of steps came last. I kept the finest zoom and counted cells. I derived
the count so that it would not look like magic, and I was pleased with the
derivation. The user asked what could possibly not close under one isotropic
rule. I tried the rule, saw two tests fail and a number go up, and went back to
my count. That was the worst turn of the day: I had the right form in my hands
and put it down because the old code disagreed with it. The user did not let it
stand. Read adversarially, the old code was reporting a square two kilometres a
side for a ray that had read one cell.

What I notice, writing this: every piece I added was defensible alone, and I
defended each. None of them was the rule. The user was not asking for more care;
they were asking for fewer things. The form that came out is shorter than the
one the day began with, and it answers from thirty thousand cells down in a
third of a millisecond.

The names were a second thread. I had been calling things by words nobody had
chosen, in Korean and in English, and the user could not tell what I meant.
The world's box was `BOUNDS` and is `BOUND`. A walker is a walker and a column is
the code's word, not mine to translate.
