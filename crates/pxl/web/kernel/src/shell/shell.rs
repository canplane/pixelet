// pxl-kernel/shell/shell.rs

//! What the page's UI reads of the kernel and what it asks of it. The UI is an
//! interrupt source and a display: it renders one snapshot of the state it
//! shows — a JSON document the kernel writes after each turn, which the shell
//! compares whole and hands to the UI only when it changed — and every control
//! it has is an action by name, applied here to the stores.

use core::fmt::{self, Write};

use pxl_spec::space::adapter::epsg4326;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::codec::json::{read_json, JsonValue, Quoted};

use crate::control::camera::store::{camera_store, CameraStatus};
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::device_orientation::{orientation_close, orientation_open, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::store::{device_location_store, device_orientation_store, DevicePoseStatus};
use crate::global::Global;
use crate::layer::character::preview::{preview_close, preview_open};
use crate::layer::character::store::companion_load_store;
use crate::layer::character::variant::CharacterGroup;
use crate::layer::object::store::OBJECT_STORAGE_VERSION;
use crate::layer::terrain::elevation::{get_ele, ground_or_raw};
use crate::sched::ledger::log::{ledger_set_logging, ledger_window_start, ledger_window_stop};
use crate::sched::turn::world_platform;
use crate::shell::map_store::map_store;
use crate::shell::setting::schema::{SettingRef, SettingValue, SETTING_KEYS};
use crate::shell::setting::store::settings_store;
use crate::shell::ui::compass::{compass_reset, show_compass};
use crate::shell::ui::scale_bar::scale_bar;
use crate::world::change::request_world_tick;
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

/// A number as JSON writes it: `null` for one that is not finite.
struct Num(f64);

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_finite() {
            write!(f, "{}", self.0)
        } else {
            f.write_str("null")
        }
    }
}

/// A point as JSON writes it: `[lon, lat]`.
struct LonLat(f64, f64);

impl fmt::Display for LonLat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", Num(self.0), Num(self.1))
    }
}

/// A value, or `null` for none.
struct Or<T>(Option<T>);

impl<T: fmt::Display> fmt::Display for Or<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => v.fmt(f),
            None => f.write_str("null"),
        }
    }
}

fn pose_status_str(status: DevicePoseStatus) -> &'static str {
    match status {
        DevicePoseStatus::Idle => "idle",
        DevicePoseStatus::Loading => "loading",
        DevicePoseStatus::Ready => "ready",
        DevicePoseStatus::Error => "error",
    }
}

/// The state the UI shows, as one JSON document written into `out` — the
/// lane the shell hands over, so a turn builds no string of it.
pub fn ui_snapshot_json<W: Write>(out: &mut W) -> fmt::Result {
    let map = map_store();
    let nav = map_navigation();
    let location = device_location_store();
    let orientation = device_orientation_store();
    let camera = camera_store();
    let settings = settings_store();
    let bar = scale_bar(view().height.max(1.0));

    // The map centre as the shell reads it, in degrees: what its search
    // measures distance from.
    let center = map.map_center.map(|c| {
        let mut g = [0.0; 3];
        epsg4326::unproject(&mut g, c.i, c.f);
        g
    });
    // The L3 page that centre falls in, which is what a reverse-geocode label
    // is keyed by. Sent because the shell would otherwise recompute it: it
    // would have to project the degrees back and index them again, in a second
    // implementation of the space, to reach the number this line already has.
    // A decimal string rather than a number — a page index is 64-bit, and
    // JSON's number is not.
    let page = map.map_center.map(|c| L3_IDX(c.i));
    write!(
        out,
        "{{\"map\":{{\"status\":{},\"is_off_center\":{},\"center\":{},\"page\":{}}},",
        Quoted(map.map_status.as_str()),
        nav.is_off_center,
        Or(center.map(|g| LonLat(g[0], g[1]))),
        Or(page.map(|p| Quoted(p.to_string())))
    )?;
    write!(
        out,
        "\"location\":{{\"status\":{},\"has_location\":{},\"error\":{}}},",
        Quoted(pose_status_str(location.status)),
        location.location.is_some(),
        Or(location.error.map(|m| m as u32))
    )?;
    let companion = companion_load_store();
    write!(out, "\"companion\":{{\"person_ready\":{},\"pet_ready\":{}}},", companion.person_ready, companion.pet_ready)?;
    write!(
        out,
        "\"orientation\":{{\"status\":{},\"heading\":{}}},",
        Quoted(pose_status_str(orientation.status)),
        Or(orientation.orientation.and_then(|o| o.heading).map(Num))
    )?;
    write!(out, "\"view\":{{\"azimuth\":{}}},\"is_follow_mode\":{},", Num(camera.view_state.azimuth), camera.is_follow_mode)?;
    out.write_str("\"settings\":{")?;
    let mut first = true;
    for key in SETTING_KEYS() {
        let Some(value) = settings.value(key) else { continue };
        if !first {
            out.write_char(',')?;
        }
        first = false;
        match value {
            SettingRef::Toggle(b) => write!(out, "{}:{b}", Quoted(key))?,
            SettingRef::Text(t) => write!(out, "{}:{}", Quoted(key), Quoted(t))?,
            SettingRef::Number(n) => write!(out, "{}:{n}", Quoted(key))?,
        }
    }
    out.write_str("},")?;
    write!(
        out,
        "\"scale_bar\":{{\"total_meters\":{},\"bar_width_px\":{},\"is_default_zoom\":{}}},",
        Num(bar.total_meters),
        Num(bar.bar_width_px),
        bar.is_default_zoom
    )?;
    write!(out, "\"compass\":{{\"show\":{}}},", show_compass())?;
    write!(out, "\"camera_busy\":{},", camera.camera_status != CameraStatus::Idle)?;
    write!(out, "\"object_storage_version\":{OBJECT_STORAGE_VERSION}}}")
}

/// A string field of an action, its escapes decoded; none where it is
/// missing or not a string.
fn text_of(action: &JsonValue, key: &str) -> Option<String> {
    Some(action.get(key)?.as_str()?.to_string())
}

fn f64_of(action: &JsonValue, key: &str) -> Option<f64> {
    action.get(key)?.as_f64()
}

fn bool_of(action: &JsonValue, key: &str) -> Option<bool> {
    action.get(key)?.as_bool()
}
/// What the tracking button did, as the button showed it (INPUT_TRACKING).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tracking {
    /// Back to the device.
    Recenter,
    /// Follow mode on, where the compass is listened to.
    Follow,
    /// Follow mode off, north up again.
    Unfollow,
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

/// Applies one UI action, a JSON object with a `type`. False for one that does
/// not read.
pub fn ui_action(text: &str) -> bool {
    let Some(action) = read_json(text) else { return false };
    let Some(kind) = text_of(&action, "type") else { return false };
    match kind.as_str() {
        "set_setting" => {
            let (Some(key), Some(value)) = (text_of(&action, "key"), action.get("value")) else { return false };
            let value = match (value.as_bool(), value.as_str(), value.as_f64()) {
                (Some(b), _, _) => SettingValue::Toggle(b),
                (_, Some(s), _) => SettingValue::Text(s.to_string()),
                (_, _, Some(n)) => SettingValue::Number(n as u32),
                _ => return false,
            };
            settings_store().set_setting(&key, value);
        }
        "toggle_setting" => {
            let Some(key) = text_of(&action, "key") else { return false };
            settings_store().toggle_setting(&key);
        }
        "append_chat_line" => {
            let Some(text) = text_of(&action, "text") else { return false };
            crate::layer::character::chat_bubble::append_chat_line(&text);
        }
        "reset_dist" => camera_store().reset_dist(),
        // Where the camera stands, set outright rather than driven: what the
        // kernel's own test puts a view at (apps/web/src/kernel/kernel.test.ts),
        // and what an A/B reads two builds at the same view with.
        "set_view" => {
            let (dist, polar) = (f64_of(&action, "dist"), f64_of(&action, "polar"));
            camera_store().set_view_state(crate::control::camera::store::ViewPatch { dist, azimuth: None, polar });
        }
        "compass_reset" => compass_reset(),
        "go_to" => {
            // A search result: its point, grounded the way every map_center
            // setter that respects ground_snap is.
            let (Some(lon), Some(lat)) = (f64_of(&action, "lon"), f64_of(&action, "lat")) else { return false };
            let mut target = Pvf3::default();
            epsg4326::project(&mut target, &[lon, lat, 0.0]);
            // The user drove the camera: GPS stops overriding it.
            camera_store().is_follow_mode = false;
            let grounded = ground_or_raw(&target);
            map_store().map_center = Some(grounded);
            // A jump usually lands on a page nothing has loaded yet, where the
            // ground falls back to the raw coordinate: grounded again once the
            // page's tile arrives, rather than left under the terrain.
            shell().go_to = (grounded == target).then_some(target);
        }
        "enable_sensors" => {
            // Where iOS gates the compass, the shell prompted in the click and
            // this starts nothing for it; its answer arrives on its own.
            let platform = world_platform();
            request_orientation(platform.orientation);
            start_watching(platform.geolocation);
        }
        "sensors_open" => {
            let platform = world_platform();
            geolocation_open(platform.geolocation);
            orientation_open(platform.orientation);
        }
        "sensors_close" => {
            geolocation_close();
            orientation_close();
        }
        "preview_open" => {
            let (Some(id), Some(group), Some(number)) = (
                f64_of(&action, "id"),
                f64_of(&action, "group").and_then(|g| u8::try_from(g as u32).ok()).and_then(CharacterGroup::of),
                f64_of(&action, "number").and_then(|n| u8::try_from(n as u32).ok()),
            ) else {
                return false;
            };
            if !preview_open(id as u32, group, number) {
                return false;
            }
        }
        "preview_close" => {
            let Some(id) = f64_of(&action, "id") else { return false };
            preview_close(id as u32);
        }
        "ledger_logging" => {
            let Some(on) = bool_of(&action, "on") else { return false };
            ledger_set_logging(on);
        }
        "ledger_window_start" => ledger_window_start(),
        "ledger_window_stop" => ledger_window_stop(),
        _ => return false,
    }
    request_world_tick();
    true
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
