// pxl-core/domain/control/navigation.rs

//! Who has the camera's pivot, where the camera goes for them, and what
//! the camera is doing. One has the pivot at a time (`Holder`): the user's
//! location once, as the app starts and as locate asks; a place the user
//! asked to go to, and then the camera on its way there; the user's
//! location tracked; or no one. Each wants the camera at a place, and a
//! place is made a point at the leaves before the camera goes there
//! (place.rs), so it waits here until the world holds the leaves over it.
//! Who asks last takes the pivot (`take`); there is one holder, so whoever
//! had it has it no more.
//!
//! | the holder           | wants               | the camera    | then       |
//! | -------------------- | ------------------- | ------------- | ---------- |
//! | the location once    | the user's location | warps there   | no one     |
//! | a place              | the place           | is sent there | on its way |
//! | on its way           | to arrive           | goes on       | no one     |
//! | the location tracked | the user's location | tracks it     | keeps it   |
//! | no one               | nothing             |               |            |
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
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;

use crate::abi::overlay::CameraState;
use crate::abi::types::LocationMode;
use crate::domain::control::camera::controller;
use crate::domain::control::navigation::location::{Grace, user_location};
use crate::domain::control::navigation::place::{Floor, point};
use crate::domain::control::param::{DEFAULT_AZIMUTH, LOCATE_ENTER_U, LOCATE_LEAVE_U};
use crate::domain::query::unknown;
use crate::domain::setting::settings;
use crate::domain::view::param::FALLBACK_G4326;
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate;
use crate::sys::sensor;

/// Who has the camera's pivot.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Holder {
    /// The user's location, once: the camera is to warp there. The app
    /// starts with it, and locate asks for it.
    LocationOnce,
    /// A place the user asked to go to, and the floor it takes there, until
    /// the camera is sent.
    Place(Q3, Floor),
    /// The camera, on its way to the place it was sent to.
    Going,
    /// The user's location, tracked: how long a location not ready is waited
    /// for.
    LocationTracked { grace: Grace },
    /// No one: the camera is where it was left, or where the hand pans it.
    NoOne,
}

static HOLDER: Global<Holder> = Global::new();

fn holder() -> &'static mut Holder {
    HOLDER.get(|| Holder::LocationOnce)
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

/// Asks for a location mode, as the location button does. Locate has the
/// location take the pivot once where there is one, so the camera warps to
/// the user's location as it did at the start. Tracking takes the pivot. As
/// tracking ends, the view turns back to north.
pub(crate) fn set_location_mode(mode: LocationMode) {
    match (mode, *holder()) {
        (LocationMode::Locate, _) => {
            if user_location().is_some() {
                take(Holder::LocationOnce);
            }
        }
        (LocationMode::Track, Holder::LocationTracked { .. }) => {}
        (LocationMode::Track, _) => take(Holder::LocationTracked { grace: None }),
        (LocationMode::Off, Holder::LocationTracked { .. }) => {
            controller::azimuth_to(DEFAULT_AZIMUTH);
            take(Holder::NoOne);
        }
        (LocationMode::Off, _) => {}
    }
}

static LOCATED: Global<bool> = Global::new();

/// Whether the pivot stands at the user's location, as of this frame's
/// `located_step`.
fn located() -> bool {
    *LOCATED.get(|| false)
}

/// Decides once a frame whether the pivot stands at the user's location:
/// it leaves past `LOCATE_LEAVE_U` across the ground and is back within
/// `LOCATE_ENTER_U`. The fix's Z is an altitude and the pivot's a floor, so the
/// height is not counted.
fn located_step() {
    let located = LOCATED.get(|| false);
    *located = match (controller::pivot(), user_location()) {
        (Some(pivot), Some(at)) => {
            // X is measured the short way across the antimeridian.
            let [dx, dy, _] = world::between(&pivot, &at).map(q3::to_real);
            let within = match *located {
                true => LOCATE_LEAVE_U,
                false => LOCATE_ENTER_U,
            };
            libm::hypot(dx, dy) <= within
        }
        _ => false,
    };
}

/// The location mode: the user's location has the pivot, or the pivot
/// stands at it, or neither.
pub(crate) fn location_mode() -> LocationMode {
    match *holder() {
        Holder::LocationTracked { .. } => LocationMode::Track,
        _ if located() => LocationMode::Locate,
        _ => LocationMode::Off,
    }
}

/// Whether the user's location is tracked.
pub(crate) fn is_tracking() -> bool {
    matches!(*holder(), Holder::LocationTracked { .. })
}

/// Ends tracking, as a hand rotates the view: tracking turns the view with
/// the heading.
pub(crate) fn stop_tracking() {
    if is_tracking() {
        take(Holder::NoOne);
    }
}

/// The place the holder wants the camera at, and the floor it takes there.
fn asked() -> Option<(Q3, Floor)> {
    match *holder() {
        Holder::LocationOnce | Holder::LocationTracked { .. } => {
            user_location().map(|at| (at, Floor::Ground))
        }
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
/// (`location::lost`), and turns the view with the device's heading where
/// there is one. The camera's mode is the setting's while the camera is
/// idle.
pub(crate) fn navigation_step(gesturing: bool) {
    place::forget_touched();
    if *holder() == Holder::Going && !controller::moving() {
        *holder() = Holder::NoOne;
    }
    if let Holder::LocationTracked { grace } = holder() {
        match (location::lost(grace), sensor::heading()) {
            (true, _) => take(Holder::NoOne),
            (false, Some(deg)) => controller::azimuth_to(deg.to_radians()),
            (false, None) => {}
        }
    }
    let exact = |(at, floor): (Q3, Floor)| point(&at, floor).exact();
    let asked = asked();
    match (*holder(), asked.and_then(exact)) {
        // The place is not answered yet, or no one asks for one.
        (_, None) => {}
        // Nothing stands there: the holder lets go.
        (_, Some(None)) => take(Holder::NoOne),
        (Holder::LocationOnce, Some(Some(to))) => {
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
        (Holder::LocationTracked { .. }, Some(Some(to))) => controller::go(to),
        // These ask for no place.
        (Holder::Going | Holder::NoOne, Some(Some(_))) => {}
    }
    if let Some(Some(to)) = fallback(asked).and_then(exact) {
        controller::warp_to(to);
    }
    if state(gesturing) == CameraState::Idle {
        controller::set_mode(settings().camera_mode);
    }
    located_step();
}

#[cfg(test)]
#[path = "navigation.test.rs"]
mod test;
