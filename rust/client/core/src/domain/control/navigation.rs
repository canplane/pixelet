// pxl-core/domain/control/navigation.rs

//! Who has the camera's pivot, where the camera goes for them, and what
//! the camera is doing. One has the pivot at a time (`Holder`): the start,
//! until someone takes it; a place the user asked to go to, and then the
//! camera on its way there; the user's location, while it is tracked; or no
//! one. Each wants the camera at a place, and a place is made a point at
//! the leaves before the camera goes there (place.rs), so it waits here
//! until the world holds the leaves over it. Who asks last takes the pivot
//! (`take`); there is one holder, so whoever had it has it no more.
//!
//! | the holder          | wants               | the camera    | then      |
//! | ------------------- | ------------------- | ------------- | --------- |
//! | the start           | the user's location | warps there   | no one    |
//! | a place             | the place           | is sent there | on its way|
//! | on its way          | to arrive           | goes on       | no one    |
//! | the user's location | the user's location | goes after it | keeps it  |
//! | no one              | nothing             |               |           |
//!
//! Where nothing stands at the place, its holder lets go, whoever it is. A
//! camera never put is put at the fallback while its holder wants it
//! nowhere.
//!
//! The camera does one thing at a time (`state`): on its way to a place it
//! was sent to, an animation, it takes no gesture; and while a hand holds
//! the view or a key is held, a gesture, it is sent nowhere, the place
//! waiting. Else it is idle, and its mode can be changed.

pub(crate) mod location;
pub(crate) mod place;

use pxl_global::Global;
use pxl_spec::space::coord::adapter::epsg4326::{self, G4326};
use pxl_spec::space::coord::q3::Q3;

use crate::abi::overlay::CameraState;
use crate::abi::types::TrackingMode;
use crate::domain::control::camera::controller;
use crate::domain::control::navigation::location::{Grace, user_location};
use crate::domain::control::navigation::place::{Floor, point};
use crate::domain::control::param::DEFAULT_AZIMUTH;
use crate::domain::query::unknown;
use crate::domain::setting::settings;
use crate::domain::view::param::FALLBACK_G4326;
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate;
use crate::sys::sensor;

/// Who has the camera's pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Holder {
    /// The start: no one has taken it yet.
    Start,
    /// A place the user asked to go to, and the floor it takes there, until
    /// the camera is sent.
    Place(Q3, Floor),
    /// The camera, on its way to the place it was sent to.
    Going,
    /// The user's location, tracked: whether the view turns with the
    /// device's heading, and how long a location not ready is waited for.
    Location { heading: bool, grace: Grace },
    /// No one: the camera is where it was left, or where the hand pans it.
    NoOne,
}

static HOLDER: Global<Holder> = Global::new();

fn holder() -> &'static mut Holder {
    HOLDER.get(|| Holder::Start)
}

/// `by` takes the pivot from another: whoever had it has it no more, and
/// the camera stays where it is until its new holder moves it.
fn take(by: Holder) {
    if *holder() != by {
        *holder() = by;
        controller::stay();
        invalidate(InvalidationReason::View);
    }
}

/// What the camera is doing, given whether a hand holds the view or a key
/// is held.
pub(crate) fn state(gesturing: bool) -> CameraState {
    match (*holder(), gesturing) {
        (Holder::Going, _) => CameraState::Animation,
        (_, true) => CameraState::Gesture,
        (_, false) => CameraState::Idle,
    }
}

/// Whether a gesture is taken now: there is a camera, and it is on no
/// animation.
pub(crate) fn takes_gesture() -> bool {
    controller::pivot().is_some() && *holder() != Holder::Going
}

/// The place on the surface at `at`, its height not known.
fn surface(at: &G4326) -> Q3 {
    unknown(&epsg4326::project(at))
}

/// Goes to the ground at `to`: on the surface where the height of `to` is
/// not known, as a marker's is not, and else on the floor nearest it.
pub(crate) fn go_to_ground(to: Q3) {
    take(Holder::Place(to, Floor::Ground));
}

/// Goes to the surface at `lon`, `lat` degrees, as a search asks.
pub(crate) fn go_to_deg(lon: f64, lat: f64) {
    go_to_ground(surface(&[lon, lat, 0.0]));
}

/// Goes to the floor under the cell center `to`, as a tap on the ground
/// asks.
pub(crate) fn go_to_floor_under(to: Q3) {
    take(Holder::Place(to, Floor::Under));
}

/// The hand takes the pivot, as it pans.
pub(crate) fn take_by_hand() {
    take(Holder::NoOne);
}

/// Sets the tracking mode, as the tracking button asks. Tracking takes the
/// pivot as it starts; from one tracking mode to another it has it already.
/// As it ends, the view turns back to north.
pub(crate) fn track(mode: TrackingMode) {
    let turns = mode == TrackingMode::FollowWithHeading;
    match (mode, holder()) {
        (TrackingMode::None, Holder::Location { .. }) => {
            controller::azimuth_to(DEFAULT_AZIMUTH);
            take(Holder::NoOne);
        }
        (TrackingMode::None, _) => {}
        (_, Holder::Location { heading, .. }) => *heading = turns,
        _ => take(Holder::Location {
            heading: turns,
            grace: None,
        }),
    }
}

/// The tracking mode: whether the user's location has the pivot, and
/// whether the view turns with the device's heading.
pub(crate) fn tracking_mode() -> TrackingMode {
    match *holder() {
        Holder::Location { heading: true, .. } => TrackingMode::FollowWithHeading,
        Holder::Location { heading: false, .. } => TrackingMode::Follow,
        _ => TrackingMode::None,
    }
}

/// Whether the user's location is tracked.
pub(crate) fn is_tracking() -> bool {
    tracking_mode() != TrackingMode::None
}

/// Ends the view's turning with the device's heading, as a hand rotates the
/// view: the user's location is tracked on.
pub(crate) fn stop_tracking_heading() {
    if let Holder::Location { heading, .. } = holder() {
        *heading = false;
    }
}

/// The place the holder wants the camera at, and the floor it takes there.
fn asked() -> Option<(Q3, Floor)> {
    match *holder() {
        Holder::Start | Holder::Location { .. } => user_location().map(|at| (at, Floor::Ground)),
        Holder::Place(at, floor) => Some((at, floor)),
        Holder::Going | Holder::NoOne => None,
    }
}

/// The fallback, where a camera never put is put while its holder asks for
/// no place; none else.
fn fallback(asked: Option<(Q3, Floor)>) -> Option<(Q3, Floor)> {
    let unput = asked.is_none() && controller::pivot().is_none();
    unput.then(|| (surface(&FALLBACK_G4326), Floor::Ground))
}

/// The place being made a point, where the camera is to be next: the one
/// its holder asks for, or the fallback. The terrain is asked for the
/// leaves over it.
pub(crate) fn destination() -> Option<Q3> {
    let asked = asked();
    asked.or_else(|| fallback(asked)).map(|(at, _)| at)
}

/// Has the camera go where its holder wants it, once that place is a point
/// (the module's table), each frame, given whether a hand holds the view or
/// a key is held. Tracking ends where the location is lost
/// (`location::lost`), and turns the view with the device's heading. The
/// camera's mode is the setting's while the camera is idle.
pub(crate) fn navigation_step(gesturing: bool) {
    place::forget_touched();
    if *holder() == Holder::Going && !controller::moving() {
        *holder() = Holder::NoOne;
    }
    if let Holder::Location { heading, grace } = holder() {
        match (location::lost(grace), sensor::heading()) {
            (true, _) => take(Holder::NoOne),
            (false, Some(deg)) if *heading => controller::azimuth_to(deg.to_radians()),
            (false, _) => {}
        }
    }
    let exact = |(at, floor): (Q3, Floor)| point(&at, floor).exact();
    let asked = asked();
    match (*holder(), asked.and_then(exact)) {
        // The place is not answered yet, or no one asks for one.
        (_, None) => {}
        // Nothing stands there: the holder lets go.
        (_, Some(None)) => take(Holder::NoOne),
        (Holder::Start, Some(Some(to))) => {
            controller::warp_to(to);
            *holder() = Holder::NoOne;
        }
        // During a gesture the place waits, as it did for its leaves.
        (Holder::Place(..), Some(Some(_))) if gesturing => {}
        // The camera is on its way where the send started a move.
        (Holder::Place(..), Some(Some(to))) => {
            controller::go(to);
            controller::come_in();
            *holder() = match controller::moving() {
                true => Holder::Going,
                false => Holder::NoOne,
            };
        }
        (Holder::Location { .. }, Some(Some(to))) => controller::go(to),
        // These ask for no place.
        (Holder::Going | Holder::NoOne, Some(Some(_))) => {}
    }
    if let Some(Some(to)) = fallback(asked).and_then(exact) {
        controller::warp_to(to);
    }
    if state(gesturing) == CameraState::Idle {
        controller::set_mode(settings().camera_mode);
    }
}

#[cfg(test)]
#[path = "navigation.test.rs"]
mod test;
