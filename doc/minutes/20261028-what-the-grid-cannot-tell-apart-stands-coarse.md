# What the grid cannot tell apart stands coarse

**2026-10-28** — recorded with the user. Written against main at `8ee81810d`
(2026-10-28 00:00 KST). The checkout also held pixelet-d6's uncommitted reshape
of the observer's passes at the time. The record states a direction to build
from the code as it stands. Nothing in it is built yet.

> The philosophy settled in September and restated on the morning of 10-27 is
> the last key to finishing the drawing. Perspective closes the bound: what is
> far must be large. What is smaller than the grid is don't-care, drawn any way
> or passed. But a cell the grid can see is never don't-care. It stands, coarse
> if it must, and never as a hole. Each pixel is answered by a ray of its own,
> and the zoom it meets the world at follows how densely the rays land there.
> Every answer holds alike across direction, source, observer, axis and device.

## Thesis: the philosophy, in the user's words

Times are KST, 10-27, and the quotes are verbatim. The September minutes they
point to are `archive/20260928-what-is-drawn-is-what-the-screen-covers.md` ("A
cell's inside is don't-care", "A box is all the view asks"),
`archive/20260923-a-coarse-cell-is-isotropic.md` and
`archive/20260926-the-view-is-cubes-about-the-eye.md` (the eye's cubes, the
pyramid, called RTE).

- **Far is large; under the grid, anything goes.**
  - 01:31 "멀면 그만큼 크셔야지. 격자보다 작은데 내가 좆대로 그리든 말든 뭔
    상관이야. … LOD로 가서 어차피 괜찮은게 아닌거야? … 우아하고, 수학적이고,
    결정적이고, 상한이 닫히게 해결하길 바라."
  - In English: far must be correspondingly large; what is smaller than the
    grid may be drawn any way at all; solve it elegantly, mathematically,
    deterministically, with the bound closed.
- **Perspective is the bound.**
  - 01:32 "원근법이 상한을 자동으로 만들고 구조적 상한을 보장한다."
  - In English: perspective makes the upper bound by itself and guarantees it
    structurally.
- **The query is three-dimensional, in the pyramid's units.**
  - 01:48 "쿼리는 3차원, 피라미드의 줌/격자 단위로 양자화되어서 답하는게 맞을거야.
    … 그게 대칭성이고 환원이고 RTE니까."
  - 01:41 "우리는 one axis … 라는 관점이 있잖아."
  - In English: the query asks in 3D and answers quantized to the pyramid's zoom
    and grid, which is symmetry, reduction and RTE. That is the one-axis view.
- **The LOD look is the floating box.**
  - 01:55 "LOD 공중부양 상자"
  - 02:33 "기존 LOD 모습처럼 가면 되는거야. 1/8 양자화, 공중부양 등"
  - In English: the coarse look stays as it was, a box quantized to eighths,
    floating in its cell.
- **No direction is privileged.**
  - 10:17 "원근으로 보고 빈 곳은 점프하는데 지평선이 왜 자꾸 우대받냐고. 내가
    공중을 보든지 아래를 보든지 똑같아야 할거 아니야."
  - In English: looking up, down or at the horizon must cost and behave alike.
- **Don't-care means pruned, not drawn exactly.**
  - 10:19 "애초에 don't care로 보고 그냥 가지치기를 쳐서 그리지 말든가 하란
    말이다. 격자보다 작으면. 내가 언제 정확하게 그리라고 했어?"
  - 10:28 "그 철학은 9월에 이미 다 확정된거고"
  - In English: treat what is under the grid as don't-care and prune it; it was
    never to be drawn exactly. The philosophy was settled in September.
- **Observers share units, not implementations.**
  - 12:18 "해상도/가지치기 단위는 같이 맞추는게 맞지. 하지만, 인터페이스만 같고
    구현은 자기 효과에 맞게"
  - In English: the eye and the sun share resolution and pruning units and one
    interface, and each is implemented to its own effect.
- **The display decides nothing.**
  - 14:47 "LOD/레이에서 dpr의 영향을 없애자고. 직교적으로" (`0c84382cd`)
  - In English: remove the display ratio's effect on LOD and rays, orthogonally.

The September record says what the edge of don't-care is (20260928): "What is
not allowed is for an approximation to leak: to change what a neighbouring cell
observes." And: "A center ray can miss a sliver between two cells; the sliver
then stays coarse, which is quality, not a hole." Its rule is tested "on each
axis of the screen", and "the grazing ground is the same rule and needs nothing
of its own".

## What the code holds, and keeps

The code is in good shape, and it is the base. Going back to an older form is a
regression, not an answer. What stands:

- The query asks a node and answers a box in its own units, every axis alike
  (`cf769abc1`; [a node is asked in its own units](./20261027-a-node-is-asked-in-its-own-units.md)).
- The ray is a cone (`cast::Cone`). The eye's cone is an angle, the grid's cell
  (`beam::spacing`). The sun's is its texel. Control is exact.
- The prune passes what the cone cannot tell apart, so the horizon costs what
  any direction does (`cast.rs` `walk`, `meets_box`).
- Every view is cast at one τ, an angle, so the display draws and decides
  nothing (`0c84382cd`).
- The rays find and the raster hands out
  ([the ray finds, the raster hands out](./20261026-the-ray-finds-the-raster-hands-out.md)).
  The stages are beam, cell start, samples, the raster's pick, the pixel and
  the walk again (`ac7785f21`, `bf1a3e20a`).
- A cell keeping less than its cube is drawn as its kept box, eighths floating
  in the cell (`answer::kept`, `cast::kept_bounds`).

## Antithesis: rows the grid can see go empty

Seen at a grazing angle, rows of coarse cells vanish whole. The user's
screenshot was taken 10-27 22:30. Looking straight down, none vanish. The user
saw them at 14:00 too and let them pass for speed, so this is not a late
regression. It belongs to the structure.

A whole cube is not don't-care. A cell at its zoom spans at least the grid
from any direction, by the zoom rule itself. Two readings of the code account
for the loss. Both are code readings, not yet confirmed on the screen:

1. **The zoom is the distance's alone, while the samples are sparse.**
   - `cast::Detail` takes `grid::cell_at(d² · τ²)`, isotropic, so a cell's
     extent across the screen along a slant shrinks with the slant's sine.
   - Since 10-23 a pixel no longer walks: 4 × 4 samples a cell do
     (`beam::SAMPLE`). A pixel takes the nearest box a sample stopped at that
     its ray enters, and walks only where its ray enters none.
   - A row thinner on screen than a sample's spacing has no box, so its pixels
     take the box behind it.
   - [A cell casts a beam](./20261022-a-cell-casts-a-beam.md) foresaw it. Its
     Status says the cut was taken and "Sparser samples lose coverage" is
     answered by the fallback walk, not avoided. Its proof holds for a walk
     from a cell's start, not for the box a pixel takes from a sample.
2. **The prune passes as air what is as thick as a cone.**
   - Each inner side of a split's box shrinks by half a cone (`meets_box`), so
     content up to a cone thick is passed, not drawn coarse.
   - A cell near the near end of its band keeps a core `s − c` thick, where `s`
     is its side and `c` the cone, at most one cone. Samples a quarter-cone
     apart can step over it.
   - Passing a cell as air is a leak in 20260928's sense: the neighbour cell
     then observes what lies behind.

Both apply don't-care by distance, isotropically. September applied it by each
axis of the screen. That is the gap.

## What was tried and why each broke the philosophy

- **The touch rule** (`226b28d72`, `637b88db4`, `f3a2fbdd9`, `e5532c80c`).
  - It placed a coarse cell by bits only the ground set, so a terrace of placed
    cubes would look otherwise than the ground's. That breaks source symmetry.
  - The user: "사용자는 겉으로 지형인지 오브젝트인지 구분이 안되어야 한다" (23:52;
    a user must not be able to tell the ground from what is placed).
  - Reverted whole in `8ee81810d`. The lesson is in the reshape skill,
    "Proposing it" (`de7483394`).
- **Shadows and AO raised as costs.** They do not matter here. The user: "아예
  텅 비어버리는게 중요한 문제지" (what matters is going empty).
- **A pixel walking again when its pick is far beyond its neighbours'.** It
  guesses at the observation instead of following the rule.
- **Reverting the prune, or τ.** Either regresses the horizon (10:17) or the
  display's independence (14:47).
- **A finer grid, or denser samples.** Either moves the threshold and keeps the
  hole.
- **The prune's cone at each ray's own width.** The idea was a sample's quarter
  cell, or a pixel, in place of the grid's cell. It was offered as keeping the
  horizon while the rows stay. It holds neither.
  - It answers the prune's half of the loss alone. The distance zoom and the
    sparse samples stay, so rows thinner on screen than a sample's spacing
    still go. They only get thinner.
  - The bound grows with it. A grazing ray crosses about `1/s` of the cone's
    cells an octave, and a cone four times narrower lets it cross up to four
    times as many: the horizon regresses.

## Synthesis: the direction

The user, 10-27 23:58: "그냥 픽셀별로 레이를 다 쏴. 빔으로 원뿔을 잘 노려.
그리고 닿는 밀도에 비례하게 LOD/줌을 세팅하는거지." In English: shoot a ray for
every pixel, aim the cone well with the beams, and set the LOD and zoom in
proportion to how densely the rays land. Read with the morning's philosophy:

- **Every pixel is answered by a ray of its own.** It is resumed from the beam's
  conservative start, as 20261022 was first built, where the proof held: 0 of
  5,627,328 rays differed from the root walk, at 10.24 visits a pixel resumed
  against 33.16 from the root.
- **The zoom follows how densely the rays land.**
  - It is the footprint of the ray's cone on what it meets: far is large, along
    each axis of the screen, as 20260928's rule is.
  - A grazing surface is met coarser. What is under the grid is then inside one
    cell, and stands as that cell's floating eighths box, the LOD look. It is
    never passed as air where it fills a cell.
- **The bound is perspective's.** A grazing ray met at its footprint's zoom
  stops at a coarse node, so its walk is closed by perspective, not by a prune
  standing in for it.
- **Symmetry holds across all five.**
  - Direction: up, down and the horizon alike.
  - Source: the ground and what is placed are one kind.
  - Observer: same units, each its own implementation.
  - Axis: one axis, the query in 3D.
  - Device: the display draws and decides nothing.

The fix lives in the walk and the zoom rule, which every source and observer
share. It is never in a source or a pass alone.

pixelet-d3's reading sits beside this. The user likes it, but it is not the
core, and where the two part the user's stands.

- The distance zoom stays the finest any ray asks, the eye's bound for what is
  fetched and held, and the footprint only makes a ray coarser.
- The cone's angle stays the grid's cell, so rays are per pixel while the
  quantum stays display-free.
- The slant can be read off the beam's four corner depths, which costs
  nothing.

## The user's verdict and turn (10-28)

- **The axis alone lets the rows go.** The user: "이게 원래의 축이 맞아. 그런데
  이렇게 하면, 결론이 가로줄을 그냥 신경쓰지 않는다는 말로 가잖아." In English:
  this is the original axis, but on its own it concludes that the rows do not
  matter. A row thinner than the grid on the screen is don't-care under it,
  and the 14:00 rows were let pass for speed on that ground.
- **The turn.** Keep the axis, go to the pixel, and turn the LOD toward the
  rays' density. Many rays then land on the same place.
- **Concern: the prune.** Can a walk still prune when many rays land on one
  node? Read so far, and not yet decided: with the zoom at the footprint, a
  grazing ray stops at a coarse node, and the prune becomes cutting the
  descent there and drawing that node's box. It no longer means passing its
  content as air. The cost is the look. A node is a cube, so one coarse
  enough along the screen's depth is as coarse across it (pixelet-d6, 10-27
  10:16).
- **Concern: the observed pyramid's room.** The observation cache is keyed to
  the pyramid's nodes in a table of fixed size (`observed`). Per-pixel walks
  write it far more, and a zoom that follows the slant puts more zooms of one
  place in it. As it stands, it will run out of room.

## The experiment

- **When the rows began.** The felt speed-up
  ([what made it smooth](../experiment/20261026-what-made-it-smooth.md), main
  at 10-26 22:42, now `9c8562e4f`) came before the rows, and the rows were
  there at 10-27 14:00. Between them lie the box in the node's units and the
  prune (`cf769abc1`, 11:54), and earlier that night the beams passing a slab
  they clear. Exports of these, side by side, seen on the screen and not
  timed, place the start.
- **Pixel and density.** Build the turn above as an experiment on the code as
  it stands, against the state archived on 10-28. Before the rows, the
  drawing ran near the pixel and fast enough.

## Open

- **Where the slant is read.** The beam's corner depths, or the box face met.
- **Control.** Control meets what is drawn at the zoom the eye draws there
  (ARCHITECTURE.md, "The query, the only seam"), and must follow the same zoom
  rule.
- **Cost.** The pixel walks `bf1a3e20a` removed come back. They are timed once,
  at the end, against the backlog's baseline.
- **Confirming the two readings.** Seen side by side on the screen, not timed.
