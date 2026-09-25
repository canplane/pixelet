// pxl-kernel/shell/shell.rs

//! What the page's UI reads of the kernel and what it asks of it. The UI is an
//! interrupt source and a display: it renders one snapshot of the state it
//! shows — a Snapshot packet (cmd.json) the kernel writes after each turn and
//! hands over only when it changed — and every control it has is a command
//! of its own, applied here to the stores.

use pxl_spec::bake::place_page::leaf_slot;
use pxl_spec::place::PlaceRank;
use pxl_spec::space::adapter::epsg4326;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::wire::xtp::PacketWriter;

pub use crate::abi::cmd::Tracking;
use crate::abi::cmd::{CompanionState, LocationState, LonLat, MapState, OrientationState, PlaceName, PlacePart, ScaleBar, Snapshot};
use crate::control::camera::store::{camera_store, CameraStatus, ViewPatch};
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::device_orientation::{orientation_close, orientation_open, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::store::{device_location_store, device_orientation_store};
use crate::global::Global;
use crate::layer::character::store::companion_load_store;
use crate::layer::geocode::fetcher::centre_group;
use crate::layer::geocode::store as geocode_store;
use crate::layer::object::store::OBJECT_STORAGE_VERSION;
use crate::layer::terrain::elevation::{get_ele, ground_or_raw};
use crate::sched::turn::world_platform;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, SettingKey};
use crate::shell::ui::compass::show_compass;
use crate::shell::ui::scale_bar::{mercator_zoom, scale_bar};
use crate::world::frame::view::view;

struct Shell {
    /// A search jump that landed on a page with no terrain yet: re-grounded
    /// the moment that page's tile arrives, while the map is still there.
    go_to: Option<Pvf3>,
}

static SHELL: Global<Shell> = Global::new();

fn shell() -> &'static mut Shell {
    SHELL.get(|| Shell { go_to: None })
}

/// The place the centre page is in, broad to narrow, cut at the rung this
/// zoom asks for, each part handed to `each`; none where the page is in none
/// or the packets it is read out of are not here yet.
///
/// Every language the provider had, rather than one resolved name. The
/// reader's language is the page's — i18next holds it — and switching it
/// re-reads a label already sent instead of asking for another.
fn place_parts(page: Option<IVec>, zoom: f64, each: &mut dyn FnMut(&PlacePart<'_>)) {
    let deepest = PlaceRank::at_zoom(zoom);
    let parts = page
        .zip(centre_group())
        .zip(geocode_store::dict())
        .and_then(|((page, group), dict)| {
            let group = geocode_store::group_of(group)?;
            group.view.place_of(leaf_slot(ivec::X(page), ivec::Y(page)), &dict.view)
        })
        .into_iter()
        .flatten()
        .filter(|p| p.rank <= deepest);
    for part in parts {
        let names = |each: &mut dyn FnMut(&PlaceName<'_>)| {
            for (lang, name) in part.languages() {
                each(&PlaceName { lang: &lang, name: &name });
            }
        };
        each(&PlacePart { name: &part.name, names: &names });
    }
}

/// The state the UI shows, as a Snapshot packet written into `out`; none
/// where it does not fit.
pub fn ui_snapshot(out: &mut [u8]) -> Option<u32> {
    let map = map_store();
    let location = device_location_store();
    let orientation = device_orientation_store();
    let camera = camera_store();
    let settings = settings_store();
    let companion = companion_load_store();
    let bar = scale_bar(view().height.max(1.0));
    let page = map.map_center.map(|c| L3_IDX(c.i));
    let center = map.map_center.map(|c| {
        let mut g = [0.0; 3];
        epsg4326::unproject(&mut g, c.i, c.f);
        LonLat { lon: g[0], lat: g[1] }
    });
    let values = SettingKey::ALL.map(|key| settings.get(key));
    let zoom = mercator_zoom(view().height.max(1.0));
    let snapshot = Snapshot {
        map: MapState {
            status: map.map_status,
            is_off_center: map_navigation().is_off_center,
            center,
            page,
        },
        location: LocationState {
            status: location.status,
            has_location: location.location.is_some(),
            error: location.error,
        },
        companion: CompanionState {
            person_ready: companion.person_ready,
            pet_ready: companion.pet_ready,
        },
        orientation: OrientationState {
            status: orientation.status,
            heading: orientation.orientation.and_then(|o| o.heading),
        },
        azimuth: camera.view_state.azimuth,
        is_follow_mode: camera.is_follow_mode,
        camera_busy: camera.camera_status != CameraStatus::Idle,
        show_compass: show_compass(),
        settings: &values,
        scale_bar: ScaleBar {
            total_meters: bar.total_meters,
            bar_width_px: bar.bar_width_px,
            is_default_zoom: bar.is_default_zoom,
        },
        place: &|each| place_parts(page, zoom, each),
        poi: &crate::layer::poi::placement::badges,
        object_sizes: &crate::layer::object::size::OBJECT_SIZES,
        object_storage_version: OBJECT_STORAGE_VERSION,
    };
    let mut w = PacketWriter::new(out, Snapshot::FIELDS);
    snapshot.write(&mut w);
    w.finish()
}

/// The tracking button's press. Which of the three it is was decided by the
/// shell from the state it drew the button in: the press means what the user
/// saw, and entering follow mode is also what the shell's compass prompt has
/// to be raised for.
pub fn tracking_action(tracking: Tracking) {
    let camera = camera_store();
    match tracking {
        Tracking::Recenter => {
            if let Some(device) = map_navigation().device_coord {
                // device_coord's Z is the device's raw GPS altitude, not the
                // map's baked terrain height — ground_or_raw applies the same
                // terrain sample a pan uses, so this recenter lands where
                // panning would.
                let target = if settings_store().ground_snap { ground_or_raw(&device) } else { device };
                map_store().map_center = Some(target);
            }
            camera.is_follow_mode = false;
        }
        Tracking::Follow => {
            if request_orientation(world_platform().orientation) {
                camera.is_follow_mode = true;
            }
        }
        Tracking::Unfollow => {
            camera.is_follow_mode = false;
            camera.reset_azimuth();
        }
    }
}

/// Moves the map centre to `target`, grounded the way every map_center
/// setter that respects ground_snap is.
pub fn go_to(target: Pvf3) {
    // The user drove the camera: GPS stops overriding it.
    camera_store().is_follow_mode = false;
    let grounded = ground_or_raw(&target);
    map_store().map_center = Some(grounded);
    // A jump usually lands on a page nothing has loaded yet, where the
    // ground falls back to the raw coordinate: grounded again once the
    // page's tile arrives, rather than left under the terrain.
    shell().go_to = (grounded == target).then_some(target);
}

/// A search result: the centre moved to degrees.
pub fn go_to_degrees(lon: f64, lat: f64) {
    let mut target = Pvf3::default();
    epsg4326::project(&mut target, &[lon, lat, 0.0]);
    go_to(target);
}

/// Where the camera stands, set outright rather than driven: what the
/// kernel's own test puts a view at (apps/web/src/kernel/kernel.test.ts), and
/// what an A/B reads two builds at the same view with.
pub fn set_view(dist: Option<f64>, polar: Option<f64>) {
    camera_store().set_view_state(ViewPatch { dist, azimuth: None, polar });
}

/// The sensor button. Where iOS gates the compass, the shell prompted in the
/// click and this starts nothing for it; its answer arrives on its own.
pub fn sensors_enable() {
    let platform = world_platform();
    request_orientation(platform.orientation);
    start_watching(platform.geolocation);
}

/// The map is on screen and wants the sensors.
pub fn sensors_open() {
    let platform = world_platform();
    geolocation_open(platform.geolocation);
    orientation_open(platform.orientation);
}

/// The map is gone, and the sensors are let go.
pub fn sensors_close() {
    geolocation_close();
    orientation_close();
}

/// The shell's turn: a pending search jump re-grounded once its page's terrain
/// has arrived, if the map is still on that page.
pub fn shell_step() {
    let Some(target) = shell().go_to else { return };
    if get_ele(&target).is_none() {
        return;
    }
    shell().go_to = None;
    if map_store().map_center.is_some_and(|c| L3_IDX(c.i) == L3_IDX(target.i)) {
        map_store().map_center = Some(ground_or_raw(&target));
    }
}
