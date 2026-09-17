// pxl_core/app/shell.rs

//! What the page's UI reads of the kernel and what it asks of it. The UI is an
//! interrupt source and a display: it renders one snapshot of the state it
//! shows — a JSON document the kernel writes after each turn, which the shell
//! compares whole and hands to the UI only when it changed — and every control
//! it has is an action by name, applied here to the stores.

use core::fmt::{self, Write};

use pxl_shared::spec::cell::{placeable_cube_of, DecorId};
use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use xpute_core::kit::json::{read_json, JsonValue, Quoted};

use crate::app::setting::schema::{SettingRef, SettingValue, SETTING_KEYS};
use crate::app::setting::store::settings_store;
use crate::app::store::ui_store::{ui_store, ActiveTool};
use crate::module::camera::store::{camera_store, CameraStatus};
use crate::module::device::pose::device_orientation::{orientation_subscribe, orientation_unsubscribe, request_orientation, OrientationPlatform};
use crate::module::device::pose::geolocation::{geolocation_subscribe, geolocation_unsubscribe, start_watching};
use crate::module::device::pose::store::{device_location_store, device_orientation_store, DevicePoseStatus};
use crate::module::map::navigation::map_navigation::map_navigation;
use crate::module::map::store::map_store;
use crate::module::map::ui::control::compass::{compass_reset, show_compass};
use crate::module::map::ui::control::scale_bar::scale_bar;
use crate::module::world::change::request_world_tick;
use crate::module::world::character::character_preview::{preview_close, preview_open};
use crate::module::world::character::store::companion_load_store;
use crate::module::world::character::variant::CharacterGroup;
use crate::module::world::environment::theme::themes;
use crate::module::world::environment::theme_clock::theme_key;
use crate::module::world::frame::view::view;
use crate::module::world::object::store::OBJECT_STORAGE_VERSION;
use crate::module::world::sched::log::{ledger_set_logging, ledger_window_start, ledger_window_stop};
use crate::module::world::terrain::elevation::{get_ele, ground_or_raw};
use crate::module::world::world::{set_world_platform, world_platform, Platform};
use crate::sys::global::Global;

struct Shell {
    /// A search jump that landed on a page with no terrain yet: re-grounded
    /// the moment that page's tile arrives, while the map is still there.
    go_to: Option<PVF3>,
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

fn active_tool_of(s: &str) -> Option<ActiveTool> {
    match s {
        "pointer" => Some(ActiveTool::Pointer),
        "object" => Some(ActiveTool::Object),
        "decor" => Some(ActiveTool::Decor),
        "eraser" => Some(ActiveTool::Eraser),
        _ => None,
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

    // The map centre as the shell reads it, in degrees: what its reverse
    // geocode keys a page by and its search measures distance from.
    let center = map.map_center.map(|c| {
        let mut g = [0.0; 3];
        epsg4326::unproject(&mut g, c.i, c.f);
        g
    });
    write!(
        out,
        "{{\"map\":{{\"status\":{},\"is_off_center\":{},\"center\":{}}},",
        Quoted(map.map_status.as_str()),
        nav.is_off_center,
        Or(center.map(|g| LonLat(g[0], g[1])))
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
    // The fog is the one theme colour the shell paints too, so it crosses as
    // the CSS colour a stylesheet takes. The kernel holds it as the number it
    // lights with (environment/theme.json's `$type: "rgb"`); spelling it out
    // is this one writer's job, the way the shipped palette's is.
    write!(out, "\"camera_busy\":{},\"fog\":\"#{:06x}\",", camera.camera_status != CameraStatus::Idle, themes(theme_key()).fog)?;
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
/// The tracking button: back to the device when the map is off it, else
/// follow mode on — the compass sensor asked for first — or off again
/// (INPUT_TRACKING).
pub fn tracking_action() {
    let location = device_location_store();
    let is_enabled = location.status == DevicePoseStatus::Ready && location.location.is_some();
    let is_loading = location.status == DevicePoseStatus::Loading;
    if !is_enabled || is_loading {
        return;
    }
    let camera = camera_store();
    let nav = map_navigation();
    if nav.is_off_center && !camera.is_follow_mode {
        if let Some(device) = nav.device_coord {
            // device_coord's Z is the device's raw GPS altitude, not the map's
            // baked terrain height — ground_or_raw applies the same terrain
            // sample a pan uses, so this recenter lands where panning would.
            let target = if settings_store().ground_snap { ground_or_raw(&device) } else { device };
            map_store().map_center = Some(target);
        }
        camera.is_follow_mode = false;
        return;
    }
    if !camera.is_follow_mode {
        if request_orientation(world_platform().orientation) {
            camera.is_follow_mode = true;
        }
    } else {
        camera.is_follow_mode = false;
        camera.reset_azimuth();
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
        "set_active_tool" => {
            let Some(tool) = text_of(&action, "tool").and_then(|t| active_tool_of(&t)) else { return false };
            ui_store().active_tool = tool;
        }
        "set_object_size" => {
            let Some(size) = f64_of(&action, "size") else { return false };
            ui_store().object_size = size;
        }
        "select_cube" => {
            let Some(cube) = f64_of(&action, "num_id").and_then(|n| placeable_cube_of(n as u32)) else {
                return false;
            };
            ui_store().select_cube(cube);
        }
        "select_decor" => {
            let Some(id) = f64_of(&action, "num_id").and_then(|n| u8::try_from(n as u32).ok()).and_then(DecorId::of) else {
                return false;
            };
            ui_store().select_decor(id);
        }
        "append_chat_line" => {
            let Some(text) = text_of(&action, "text") else { return false };
            ui_store().append_chat_line(&text);
        }
        "set_is_follow_mode" => {
            let Some(on) = bool_of(&action, "on") else { return false };
            camera_store().is_follow_mode = on;
        }
        "reset_dist" => camera_store().reset_dist(),
        "compass_reset" => compass_reset(),
        "go_to" => {
            // A search result: its point, grounded the way every map_center
            // setter that respects ground_snap is.
            let (Some(lon), Some(lat)) = (f64_of(&action, "lon"), f64_of(&action, "lat")) else { return false };
            let mut target = PVF3::default();
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
            // Orientation first: Safari consumes the gesture's activation, so a
            // permission request reached later can be refused as outside it.
            let platform = world_platform();
            request_orientation(platform.orientation);
            start_watching(platform.geolocation);
        }
        "sensors_subscribe" => {
            // Sent before the first frame, which is what tells the kernel the
            // platform otherwise: the action carries it.
            let platform = Platform {
                geolocation: bool_of(&action, "geolocation").unwrap_or(false),
                orientation: OrientationPlatform {
                    supported: bool_of(&action, "orientation").unwrap_or(false),
                    needs_permission: bool_of(&action, "needs_permission").unwrap_or(false),
                },
            };
            set_world_platform(platform);
            geolocation_subscribe(platform.geolocation);
            orientation_subscribe(platform.orientation);
        }
        "sensors_unsubscribe" => {
            geolocation_unsubscribe();
            orientation_unsubscribe();
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
