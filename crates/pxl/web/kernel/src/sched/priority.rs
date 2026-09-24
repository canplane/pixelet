// pxl-kernel/sched/priority.rs

//! What goes first: one ordering, and the three things every scheduler in the
//! tick has to agree on to be that one ordering.
//!
//! Getting a page on the screen is three passes, in series, in three
//! different files: a page is fetched, a chunk is mounted over it, and the
//! chunk is built and drawn. Nothing arrives sooner than the slowest of the
//! three, so an order is only as good as the worst of them — and each had
//! invented its own answer, and all three were different.
//!
//! - **Where from.** The ground the view is heading to look at, and not the
//!   camera. A view along the ground has its eye at the bottom edge of what
//!   it shows, so camera distance fills the screen from the bottom up and
//!   leaves the middle, which is the whole of what a tilted view is for,
//!   until last. `lead_point` gives the eye plus the target's offset and
//!   `view_centre` takes the offset back out; at rest it is the frame's
//!   origin. The page schedulers reach the same point by another road —
//!   `range_anchor`, which is `map_center` on the ground — and `map_center`
//!   is the target `lead_point` is built from, so the two are one point at
//!   two resolutions.
//! - **How far.** The distance from that ground, and the two schedulers
//!   measure it in the units their own work comes in. A page scheduler asks
//!   which ring a page is in, and a ring is the same shape at every level,
//!   so the centre page of every level is ring 0 and they go together. A
//!   chunk scheduler asks the plain distance, because a chunk is a piece of
//!   the screen and the middle of the screen is what anyone is looking at.
//!
//!   It was `in_own_sides` — the distance over the thing's own side, which
//!   is "the biggest on the screen first". The ladder puts a chunk where its
//!   own cells are the right size, so that number is about the same for
//!   every level: a coarse chunk at the horizon ranked with a fine one under
//!   the camera and the screen filled everywhere at once. Everywhere at once
//!   means the middle waits for all of it, and at the two hundred chunks a
//!   second the builder manages when it is behind, all of it is eleven
//!   seconds where the innermost ring is under three.
//! - **How the terms combine.** Lexicographically: a hard class — is this
//!   ground drawn at all — above the soft distance, never a weighted sum. A
//!   sum has some range of inputs where the soft term crosses the hard one,
//!   and a term that is not meant to order at all (a level, a page
//!   coordinate) is the same fault written differently.
//!
//! The three, as they stand:
//!
//! | pass | file | terms |
//! | --- | --- | --- |
//! | fetch | world/pyramid.rs              | class, ring, level, distance |
//! | mount | world/chunk/table.rs `mount` | distance from the centre |
//! | build | world/chunk/table.rs `priority` | holder, on screen, hole, a generation behind, distance from the centre |
//!
//! Where a pass has a term the others do not, it sits under the shared ones
//! and says why. The fetch has two: its ring is the page-space quantization
//! of the distance, since a demand's priority is one f64 the queue sorts by
//! and a continuous term cannot have anything under it; and its level is
//! there because up to WINDOW_LEVELS pages are resident over one patch of
//! ground where exactly one chunk is drawn over it, so the page order has a
//! dimension the chunk order does not.
//!
//! The lag is the lookahead, and it needs no tuning: the shown frame chases the
//! target with a critically damped step, so the gap between them is the travel
//! still to come — about a tenth of a second of motion on a steady drag
//! (POS_DAMP = 10), and the whole distance on a jump. At rest the two coincide.
//!
//! Only the order is decided here. Which LOD tier a chunk shows is still its
//! distance from the camera as drawn, because that is what is on screen.

use xpute_core::math::mat4::Mat4;
use xpute_core::math::vec3::Vec3;

/// A point in the view frame, as the chunk passes hold one.
pub type Point = [f64; 3];

/// The ground the view is heading to look at, in the view frame's own
/// coordinates: what every order here measures from.
///
/// `lead_point` returns the eye plus the target's offset, so the offset alone
/// is that ground. At rest the two coincide and it is the frame's origin, the
/// middle of the screen.
pub fn view_centre(camera: Point, lead: Point) -> Point {
    [lead[0] - camera[0], lead[1] - camera[1], lead[2] - camera[2]]
}

/// How far something is in its own sides, squared: `d²` over its side
/// squared, which is how large it looks rather than how near it is. What a
/// page's ring is in page units (world/pyramid/window.rs's `centre_dist`); the
/// chunks order by the plain distance instead, and why is above.
pub fn in_own_sides(distance2: f64, span: f64) -> f64 {
    distance2 / (span * span)
}

/// The camera position the view is heading to, in the view frame.
///
/// `target_local` is the target in the root frame's own coordinates and `root`
/// is that frame's matrix, so the result lands in the same space as every
/// chunk centre — rotation included. The camera looks at the view frame's
/// origin, which is where the target will sit once the frame has caught up, so
/// the target's current position *is* the offset the camera still has to
/// travel.
pub fn lead_point(eye: &Vec3, target_local_x: f64, target_local_y: f64, target_local_z: f64, root: &Mat4) -> Vec3 {
    let e = root;
    Vec3 {
        x: eye.x + e[0] * target_local_x + e[4] * target_local_y + e[8] * target_local_z + e[12],
        y: eye.y + e[1] * target_local_x + e[5] * target_local_y + e[9] * target_local_z + e[13],
        z: eye.z + e[2] * target_local_x + e[6] * target_local_y + e[10] * target_local_z + e[14],
    }
}

#[cfg(test)]
#[path = "priority.test.rs"]
mod test;
