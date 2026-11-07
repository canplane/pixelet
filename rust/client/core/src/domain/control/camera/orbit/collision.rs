// pxl-core/domain/control/camera/orbit/collision.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.
//!
//! The rules, in the order they are asked:
//!
//! - **The nearest**: within `NEAREST_DIST`, the eye goes anywhere over the
//!   pivot's level, through what is there, unasked: its rays do not meet the
//!   cell it stands in (pxl-engine `cast::walk`). Under the level it stops as
//!   anywhere, and no pitch is raised, so a zoom in tilted past the level can
//!   leave it in the ground: from there what goes no deeper is let through,
//!   or no turn, tilt or zoom would ever move it again (`way_hit`).
//! - **Stop**: the eye's way that hits something solid goes as far as it got
//!   (`way_hit`). What lies between the eye and the pivot is not asked.
//! - **Climb**, in `climb`: the way is that of the eye as drawn, its pitch
//!   raised over the floor under it (`climb_pose`), where the column over the
//!   eye is open (`rise_clear`). The pitch asked stays.
//! - **Dive**, in `climb`: a zoom in whose climbed way hits something goes
//!   through it to the pivot's side (`dive`). Nothing else goes through.
//!
//!   | setting | zoom in | anything else |
//!   | stop | Stop | Stop |
//!   | climb | Climb | Climb |
//!   | what climb hits | Dive | Stop |

use core::cell::Cell;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};

use xpute_core::global::Global;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::camera::orbit::{orbit_eye_off, pivot_world};
use crate::domain::control::camera::state::Intent;
use crate::domain::control::param::{CLIMB_PASSES, COLLISION_STEP_SAMPLES};
use crate::domain::control::param::{EYE_CLEARANCE_U, MIN_DIST, NEAREST_DIST};
use crate::domain::query::changes::Memo;
use crate::domain::query::{finest, Hit, HitKind, Ray, World};
use crate::domain::view::frame::coord::world_to_canonical;
use crate::domain::view::param::NEAR;
use crate::domain::view::setting::store::{settings_store, OrbitObstacleMode};
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

/// The pivot is part of the pose: the eye is `pivot + offset`, and a gate
/// that read only the offset could be walked through by panning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPose {
    pub pivot: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

fn pose(view: Intent) -> Option<ViewPose> {
    Some(ViewPose {
        pivot: pivot_world()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// The top of the ground under `at`'s eye, the one floor the gate reads of
/// a pose: none where no tile is resident.
fn ground_under(at: &ViewPose, access: &Ground<'_>) -> Option<f64> {
    ground_at(&eye(at).1, at.pivot, access)
}

/// The world a step's or a climb's poses read, and whether the sky is open over
/// the pivot they share, asked once.
struct Ground<'w> {
    world: &'w World<'w>,
    open: Cell<Option<(V3, bool)>>,
}

impl<'w> Ground<'w> {
    fn of(world: &'w World<'w>) -> Ground<'w> {
        Ground {
            world,
            open: Cell::new(None),
        }
    }

    fn top(&self, at: &Q3) -> Option<f64> {
        self.world.column_top(at).val().flatten()
    }

    fn open(&self, pivot: V3) -> bool {
        match self.open.get() {
            Some((at, open)) if at == pivot => open,
            _ => {
                let top = self.top(&world_to_canonical(&pivot));
                let open = top.is_none_or(|top| top <= pivot[2]);
                self.open.set(Some((pivot, open)));
                open
            }
        }
    }
}

/// The top of the ground at `point`: with the sky open over the pivot, its
/// column's top, so a hollow in a hill is no place for the eye; with the pivot
/// roofed, asked from the point's own cell, so a cave's floor counts.
fn ground_at(point: &Q3, pivot: V3, access: &Ground<'_>) -> Option<f64> {
    if access.open(pivot) {
        access.top(point)
    } else {
        access.world.floor_under(point, LEAF_ZOOM).val().flatten()
    }
}

/// Rise to the floor under the eye, a cell over it (`EYE_CLEARANCE_U`), as
/// `ground_under` reads it from the eye's own cell: ground and what is
/// placed alike, at any distance. None where nothing is known under it.
fn climb_rise(at: &ViewPose, access: &Ground<'_>) -> Option<f64> {
    Some(ground_under(at, access)? + EYE_CLEARANCE_U - at.pivot[2])
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Q3) {
    let off = orbit_eye_off(at.dist, at.polar, at.azimuth);
    let world: V3 = v3::add(&at.pivot, &off);
    (world, world_to_canonical(&world))
}

/// How far toward `to` the orbit gets from `from` (the module's rules).
fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    let mode = settings_store().orbit_obstacle;
    if mode == OrbitObstacleMode::through {
        return *to;
    }
    if to.dist <= NEAREST_DIST && to.polar <= core::f64::consts::FRAC_PI_2 {
        return *to;
    }
    let world = World::now();
    let access = Ground::of(&world);
    match mode {
        OrbitObstacleMode::climb => climb_step(to, from, &access),
        _ => match way_hit(from, to, |at| *at, &access) {
            Some(t) => between(from, to, t),
            None => *to,
        },
    }
}

/// The azimuth goes the short way round, as the step damps it.
fn between(from: &ViewPose, to: &ViewPose, t: f64) -> ViewPose {
    let turn = wrap(
        to.azimuth - from.azimuth,
        -core::f64::consts::PI,
        core::f64::consts::PI,
    );
    ViewPose {
        pivot: std::array::from_fn(|k| from.pivot[k] + (to.pivot[k] - from.pivot[k]) * t),
        dist: from.dist + (to.dist - from.dist) * t,
        polar: from.polar + (to.polar - from.polar) * t,
        azimuth: from.azimuth + turn * t,
    }
}

/// Whether `at`'s eye is under the top of the ground under it (`ground_under`).
fn in_solid(at: &ViewPose, access: &Ground<'_>) -> bool {
    ground_under(at, access).is_some_and(|top| top > eye(at).0[2])
}

/// A zoom in whose way hit something (`Step::Dive`), at the pitch asked: the
/// eye goes through what is on the line to the pivot's side, as far from the
/// pivot as the zoom asked or that leaves, and eases there through it, no
/// nearer than the orbit comes (`MIN_DIST`). None where nothing on the line
/// stands in the way.
fn dive(to: &ViewPose, from: &ViewPose, access: &Ground<'_>) -> Option<ViewPose> {
    let roof = toward_eye(to, from.dist, access)?;
    // Under it, the near plane's own distance, so the near plane never cuts it.
    let dist = to.dist.min(roof.t - NEAR).max(MIN_DIST);
    Some(ViewPose { dist, ..*to })
}

/// What a control's step does in `climb` (the module's rules).
enum Step {
    /// As asked, the drawn pitch rising over what is in the way (`climb_polar`).
    Climb,
    /// As far as it got: this far along the way.
    Stop(f64),
    /// Through what is in the way (`dive`).
    Dive,
}

impl Step {
    fn of(to: &ViewPose, from: &ViewPose, access: &Ground<'_>) -> Step {
        let zoom_in = to.dist < from.dist && to.polar == from.polar && to.azimuth == from.azimuth;
        let way = way_hit(from, to, |at| climb_pose(*at, access), access);
        match way {
            None => Step::Climb,
            Some(_) if zoom_in => Step::Dive,
            Some(t) => Step::Stop(t),
        }
    }
}

/// In `climb`, how far toward `to` the orbit gets from `from` (`Step`).
fn climb_step(to: &ViewPose, from: &ViewPose, access: &Ground<'_>) -> ViewPose {
    match Step::of(to, from, access) {
        Step::Climb => *to,
        Step::Stop(t) => between(from, to, t),
        Step::Dive => dive(to, from, access).unwrap_or(*to),
    }
}

/// How far along the way from `from` to `to`, from 0 to 1, the last sample
/// short of what the eye hits; none where it hits nothing. A straight line from
/// the eye as drawn to where it ends is the way, where that is clear. Else the
/// way is the straight line in control space, as the damping follows it, a
/// sample two cells the eye moves, each drawn by `draw`, and the eye goes up its
/// own column and over from each to the next (`rise_clear`). An eye that starts
/// in something solid has hit it where it stands, but for one at the nearest
/// under the pivot's level, which goes where it is no deeper (the module's
/// rules).
fn way_hit(
    from: &ViewPose,
    to: &ViewPose,
    draw: impl Fn(&ViewPose) -> ViewPose,
    access: &Ground<'_>,
) -> Option<f64> {
    let mut last = draw(from);
    if in_solid(&last, access) {
        let under_nearest = from.dist <= NEAREST_DIST && from.polar > core::f64::consts::FRAC_PI_2;
        let no_deeper = eye(&draw(to)).0[2] >= eye(&last).0[2];
        return (!(under_nearest && no_deeper)).then_some(0.0);
    }
    if line_clear(eye(&last).0, eye(&draw(to)).0, access) {
        return None;
    }
    let (a, b) = (eye(from).0, eye(to).0);
    let moved = libm::sqrt((0..3).map(|k| (b[k] - a[k]) * (b[k] - a[k])).sum());
    let samples = (libm::ceil(moved / 2.0) as u32).clamp(1, COLLISION_STEP_SAMPLES);
    for i in 1..=samples {
        let next = draw(&between(from, to, f64::from(i) / f64::from(samples)));
        if !rise_clear(&last, &next, access) {
            return Some(f64::from(i - 1) / f64::from(samples));
        }
        last = next;
    }
    None
}

/// Whether the eye gets from `from` to `to` through nothing solid: up its own
/// column to the higher of the two, over to the column of `to`, and down it. It
/// rises only where the column over it is open.
fn rise_clear(from: &ViewPose, to: &ViewPose, access: &Ground<'_>) -> bool {
    let (a, b) = (eye(from).0, eye(to).0);
    let top = a[2].max(b[2]);
    let (over_a, over_b) = ([a[0], a[1], top], [b[0], b[1], top]);
    line_clear(a, over_a, access)
        && line_clear(over_a, over_b, access)
        && line_clear(over_b, b, access)
}

/// Whether the line from `a` to `b` meets nothing a ray cannot see through, the
/// cell at `a` aside; decor holds nothing.
fn line_clear(a: V3, b: V3, access: &Ground<'_>) -> bool {
    let d = v3::sub(&b, &a);
    let t_max = libm::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if t_max == 0.0 {
        return true;
    }
    let ray = Ray {
        origin: a,
        dir: d.map(|v| v / t_max),
        t_max,
    };
    access
        .world
        .first_hit(&ray, finest())
        .val()
        .flatten()
        .is_none_or(|hit| matches!(hit.kind, HitKind::Decor(_)))
}

/// What `at`'s pivot meets toward its eye within `t_max`: a roof, where the
/// eye is straight over it; within the eye's own distance, what hides the
/// pivot from it.
fn toward_eye(at: &ViewPose, t_max: f64, access: &Ground<'_>) -> Option<Hit> {
    let ray = Ray {
        origin: at.pivot,
        dir: orbit_eye_off(1.0, at.polar, at.azimuth),
        t_max,
    };
    access.world.first_hit(&ray, finest()).exact().flatten()
}

/// How far toward `to` the orbit gets from `from`, as the controls ask it.
pub fn allowed_view(to: Intent, from: Intent) -> Intent {
    let (Some(from_pose), Some(to_pose)) = (pose(from), pose(to)) else {
        return to;
    };
    let reached = allowed_step(&to_pose, &from_pose);
    Intent {
        dist: reached.dist,
        polar: reached.polar,
        azimuth: reached.azimuth,
    }
}

/// The shallowest pitch that clears terrain, solved as `acos(h / dist)`.
///
/// It stands while what it is made of stands: the pose, where its pivot is
/// on the world, and the ground its walks read (`Columns`).
pub fn climb_polar(pivot: V3, dist: f64, polar: f64, azimuth: f64) -> f64 {
    let at = ViewPose {
        pivot,
        dist,
        polar,
        azimuth,
    };
    let key = MemoKey {
        at,
        pivot: world_to_canonical(&at.pivot),
    };
    let last = CLIMB_PITCH.get(|| Memo::NONE);
    last.forget_touched();
    last.of(World::now(), key, |w| climb_pose(at, &Ground::of(&w)).polar)
}

/// What a climb is made of, by value.
#[derive(Clone, Copy, PartialEq)]
struct MemoKey {
    at: ViewPose,
    pivot: Q3,
}

/// The last climb's pitch, kept while what it is made of stands and no
/// change touched what it read (`Memo`).
static CLIMB_PITCH: Global<Memo<MemoKey, f64>> = Global::new();

/// `at` as the climb draws it: as it is at the nearest (`NEAREST_DIST`), else
/// steepened as the floor under the eye asks (`steepened`), the pitch kept
/// however far that got, and the pitch asked where even straight down the eye
/// at `at`'s distance is in something solid, a tunnel's roof or a cave's.
fn climb_pose(at: ViewPose, access: &Ground<'_>) -> ViewPose {
    if at.dist <= NEAREST_DIST {
        return at;
    }
    let climbed = steepened(at, access);
    match in_solid(&climbed, access) && in_solid(&ViewPose { polar: 0.0, ..at }, access) {
        true => at,
        false => climbed,
    }
}

/// `at` steepened by the rise to the floor under its eye (`climb_rise`), only
/// ever steepened, until the eye clears that floor: past level too, where
/// the floor lies under the pivot and the rise is none or below it.
fn steepened(at: ViewPose, access: &Ground<'_>) -> ViewPose {
    let mut climbed = at;
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = climb_rise(&climbed, access) else {
            break;
        };
        let solved = libm::acos((rise / climbed.dist).clamp(-1.0, 1.0));
        if solved >= climbed.polar {
            break;
        }
        climbed.polar = solved;
    }
    climbed
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
