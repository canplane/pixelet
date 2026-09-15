// pxl_core/app/shell.rs

//! What the page's UI reads of the kernel and what it asks of it. The UI is an
//! interrupt source and a display: it renders one snapshot of the state it
//! shows — a JSON document the kernel writes after each turn, which the shell
//! compares whole and hands to React only when it changed — and every control
//! it has is an action by name, applied here to the stores.

use core::fmt::{self, Write};

use pxl_shared::spec::cell::decor_ids;
use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;
use xpute_core::kit::text::Quoted;

use crate::app::setting::schema::{SettingRef, SettingValue, SETTING_KEYS};
use crate::app::setting::store::use_settings_store;
use crate::app::store::ui_store::{use_ui_store, ActiveTool, RecentPick};
use crate::global::Global;
use xpute_core::kit::json::{read_json, JsonValue};
use xpute_core::kit::text::FixStr;
use crate::module::camera::store::{use_camera_store, CameraStatus};
use crate::module::device::pose::store::{use_device_location_store, use_device_orientation_store, DevicePoseStatus};
use crate::module::device::pose::use_device_orientation::{orientation_subscribe, orientation_unsubscribe, request_orientation, OrientationPlatform};
use crate::module::device::pose::use_geolocation::{geolocation_subscribe, geolocation_unsubscribe, start_watching};
use crate::module::map::navigation::use_map_navigation::use_map_navigation;
use crate::module::map::store::use_map_store;
use crate::module::map::ui::control::compass::{compass_reset, show_compass};
use crate::module::map::ui::control::scale_bar::scale_bar;
use crate::module::map::ui::overlay::search_modal::{search_close, search_query, search_rows, SearchRow};
use crate::module::map::ui::overlay::use_reverse_geocode::use_reverse_geocode;
use crate::module::world::character::character_preview::{preview_close, preview_open, preview_rows};
use crate::module::world::character::store::use_companion_load_store;
use crate::module::world::object::store::OBJECT_STORAGE_VERSION;
use crate::module::world::change::request_world_tick;
use crate::module::world::frame::view::view;
use crate::module::world::sched::log::{ledger_report_text, ledger_set_logging, ledger_window_start};
use crate::module::world::terrain::elevation::{get_ele, ground_or_raw};
use crate::module::world::environment::theme::themes;
use crate::module::world::environment::theme_clock::use_theme_key;
use crate::module::world::world::{set_world_platform, world_platform, Platform};

struct Shell {
    language: String,
    /// A search jump that landed on a page with no terrain yet: re-grounded
    /// the moment that page's tile arrives, while the map is still there.
    go_to: Option<PVF3>,
    /// A report asked for, handed over with the next snapshot.
    report: Option<String>,
}

static SHELL: Global<Shell> = Global::new();

fn shell() -> &'static mut Shell {
    SHELL.get(|| Shell {
        language: "en".to_string(),
        go_to: None,
        report: None,
    })
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

pub fn active_tool_str(tool: ActiveTool) -> &'static str {
    match tool {
        ActiveTool::Pointer => "pointer",
        ActiveTool::Object => "object",
        ActiveTool::Decor => "decor",
        ActiveTool::Eraser => "eraser",
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

/// One search result as the snapshot's JSON object, a comma before all but the first.
fn search_row_json<W: Write>(out: &mut W, first: bool, r: &SearchRow) -> fmt::Result {
    if !first {
        out.write_char(',')?;
    }
    write!(
        out,
        "{{\"gers_id\":{},\"kind\":{},\"name\":{},\"subtitle\":{},\"distance\":{},\"icon\":{},\"color\":{},\"lon\":{},\"lat\":{}}}",
        Quoted(r.gers_id),
        Quoted(r.kind),
        Quoted(r.name),
        Quoted(r.subtitle),
        Or(r.distance.map(Quoted)),
        Quoted(r.icon),
        Quoted(r.color),
        Num(r.lon),
        Num(r.lat)
    )
}

/// The state the UI shows, as one JSON document written into `out` — the
/// lane the shell hands over, so a turn builds no string of it.
pub fn ui_snapshot_json<W: Write>(out: &mut W) -> fmt::Result {
    let map = use_map_store();
    let nav = use_map_navigation();
    let location = use_device_location_store();
    let orientation = use_device_orientation_store();
    let camera = use_camera_store();
    let ui = use_ui_store();
    let settings = use_settings_store();
    let companion = use_companion_load_store();
    let address = use_reverse_geocode(&shell().language);
    let bar = scale_bar(view().height.max(1.0));

    write!(out, "{{\"map\":{{\"has_center\":{},\"center_lonlat\":", map.map_center.is_some())?;
    match map.map_center {
        Some(c) => {
            let mut g = [0.0; 3];
            epsg4326::unproject(&mut g, c.i, c.f);
            write!(out, "[{},{}]", Num(g[0]), Num(g[1]))?;
        }
        None => out.write_str("null")?,
    }
    write!(out, ",\"status\":{},\"is_off_center\":{}}},", Quoted(map.map_status.as_str()), nav.is_off_center)?;
    write!(
        out,
        "\"location\":{{\"status\":{},\"error_key\":{},\"has_location\":{}}},",
        Quoted(pose_status_str(location.status)),
        Or(location.error_key.as_deref().map(Quoted)),
        location.location.is_some()
    )?;
    write!(
        out,
        "\"orientation\":{{\"status\":{},\"heading\":{}}},",
        Quoted(pose_status_str(orientation.status)),
        Or(orientation.orientation.and_then(|o| o.heading).map(Num))
    )?;
    write!(
        out,
        "\"view\":{{\"dist\":{},\"azimuth\":{},\"polar\":{}}},\"is_follow_mode\":{},",
        Num(camera.view_state.dist),
        Num(camera.view_state.azimuth),
        Num(camera.view_state.polar),
        camera.is_follow_mode
    )?;
    write!(
        out,
        "\"ui\":{{\"active_tool\":{},\"object_color_id\":{},\"object_size\":{},\"decor_id\":{},\"recent_picks\":[",
        Quoted(active_tool_str(ui.active_tool)),
        Quoted(&ui.object_color_id),
        Num(ui.object_size),
        Quoted(ui.decor_id)
    )?;
    for (i, p) in ui.recent_picks.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        match p {
            RecentPick::Object(id) => write!(out, "{{\"kind\":\"object\",\"id\":{}}}", Quoted(id))?,
            RecentPick::Decor(id) => write!(out, "{{\"kind\":\"decor\",\"id\":{}}}", Quoted(id))?,
        }
    }
    out.write_str("],\"chat_lines\":[")?;
    for (i, l) in ui.chat_lines.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        write!(out, "{{\"id\":{},\"text\":{},\"ts\":{}}}", l.id, Quoted(&l.text), Num(l.ts))?;
    }
    out.write_str("]},\"settings\":{")?;
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
        }
    }
    write!(out, "}},\"companion\":{{\"person_ready\":{},\"pet_ready\":{}}},", companion.person_ready, companion.pet_ready)?;
    write!(out, "\"address\":{{\"label\":{},\"loading\":{}}},", Or(address.label.map(Quoted)), address.loading)?;
    write!(
        out,
        "\"scale_bar\":{{\"total_meters\":{},\"bar_width_px\":{},\"is_default_zoom\":{},\"label\":{}}},",
        Num(bar.total_meters),
        Num(bar.bar_width_px),
        bar.is_default_zoom,
        Quoted(bar.label)
    )?;
    write!(out, "\"compass\":{{\"show\":{}}},", show_compass())?;
    write!(out, "\"camera_busy\":{},\"fog\":{},", camera.camera_status != CameraStatus::Idle, Quoted(themes(use_theme_key()).fog))?;
    out.write_str("\"previews\":[")?;
    for (i, (id, model, skin)) in preview_rows().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        write!(out, "{{\"id\":{id},\"model\":{model},\"skin\":{skin}}}")?;
    }
    write!(out, "],\"object_storage_version\":{OBJECT_STORAGE_VERSION},\"search\":{{\"results\":[")?;
    let (mut rows, mut first) = (Ok(()), true);
    let loading = search_rows(&shell().language, |r| {
        if rows.is_ok() {
            rows = search_row_json(&mut *out, first, &r);
            first = false;
        }
    });
    rows?;
    write!(out, "],\"loading\":{loading}}},")?;
    write!(out, "\"report\":{}}}", Or(shell().report.take().as_deref().map(Quoted)))
}

/// A string field of an action, its escapes decoded into a string of `N`
/// bytes on the stack; none where it is missing, not a string, or longer.
fn text_of<const N: usize>(action: &JsonValue, key: &str) -> Option<FixStr<N>> {
    let s = action.get(key)?.as_str()?;
    let mut out = FixStr::new();
    write!(out, "{s}").ok()?;
    Some(out)
}

fn f64_of(action: &JsonValue, key: &str) -> Option<f64> {
    action.get(key)?.as_f64()
}

fn bool_of(action: &JsonValue, key: &str) -> Option<bool> {
    action.get(key)?.as_bool()
}
/// The tracking button: back to the device when the map is off it, else
/// follow mode on — the compass sensor asked for first — or off again.
fn tracking_action() {
    let location = use_device_location_store();
    let is_enabled = location.status == DevicePoseStatus::Ready && location.location.is_some();
    let is_loading = location.status == DevicePoseStatus::Loading;
    if !is_enabled || is_loading {
        return;
    }
    let camera = use_camera_store();
    let nav = use_map_navigation();
    if nav.is_off_center && !camera.is_follow_mode {
        if let Some(device) = nav.device_coord {
            // device_coord's Z is the device's raw GPS altitude, not the map's
            // baked terrain height — ground_or_raw applies the same terrain
            // sample a pan uses, so this recenter lands where panning would.
            let target = if use_settings_store().ground_snap { ground_or_raw(&device) } else { device };
            use_map_store().set_map_center(target);
        }
        camera.set_is_follow_mode(false);
        return;
    }
    if !camera.is_follow_mode {
        if request_orientation(world_platform().orientation) {
            camera.set_is_follow_mode(true);
        }
    } else {
        camera.set_is_follow_mode(false);
        camera.reset_azimuth();
    }
}

/// Applies one UI action, a JSON object with a `type`. False for one that does
/// not read.
pub fn ui_action(text: &str) -> bool {
    let Some(action) = read_json(text) else { return false };
    let Some(kind) = text_of::<64>(&action, "type") else { return false };
    match kind.as_str() {
        "set_setting" => {
            let (Some(key), Some(value)) = (text_of::<64>(&action, "key"), action.get("value")) else { return false };
            let value = match (value.as_bool(), value.as_str()) {
                (Some(b), _) => SettingValue::Toggle(b),
                (_, Some(s)) => SettingValue::Text(s.to_string()),
                _ => return false,
            };
            use_settings_store().set_setting(&key, value);
        }
        "toggle_setting" => {
            let Some(key) = text_of::<64>(&action, "key") else { return false };
            use_settings_store().toggle_setting(&key);
        }
        "set_active_tool" => {
            let Some(tool) = text_of::<64>(&action, "tool").and_then(|t| active_tool_of(&t)) else { return false };
            use_ui_store().set_active_tool(tool);
        }
        "set_object_size" => {
            let Some(size) = f64_of(&action, "size") else { return false };
            use_ui_store().set_object_size(size);
        }
        "select_cube" => {
            let Some(id) = text_of::<64>(&action, "id") else { return false };
            use_ui_store().select_cube(&id);
        }
        "select_decor" => {
            let Some(id) = text_of::<64>(&action, "id").and_then(|id| decor_ids.iter().copied().find(|d| *d == id.as_str())) else { return false };
            use_ui_store().select_decor(id);
        }
        "append_chat_line" => {
            let Some(text) = text_of::<1024>(&action, "text") else { return false };
            use_ui_store().append_chat_line(&text);
        }
        "set_is_follow_mode" => {
            let Some(on) = bool_of(&action, "on") else { return false };
            use_camera_store().set_is_follow_mode(on);
        }
        "reset_azimuth" => use_camera_store().reset_azimuth(),
        "reset_dist" => use_camera_store().reset_dist(),
        "compass_reset" => compass_reset(),
        "tracking" => tracking_action(),
        "go_to" => {
            // A search result: its point, grounded the way every map_center
            // setter that respects ground_snap is.
            let (Some(lon), Some(lat)) = (f64_of(&action, "lon"), f64_of(&action, "lat")) else { return false };
            let mut target = PVF3::default();
            epsg4326::project(&mut target, &[lon, lat, 0.0]);
            // The user drove the camera: GPS stops overriding it.
            use_camera_store().set_is_follow_mode(false);
            let grounded = ground_or_raw(&target);
            use_map_store().set_map_center(grounded);
            // A jump usually lands on a page nothing has loaded yet, where the
            // ground falls back to the raw coordinate: grounded again once the
            // page's tile arrives, rather than left under the terrain.
            shell().go_to = (grounded == target).then_some(target);
        }
        "set_language" => {
            let Some(lang) = text_of::<64>(&action, "lang") else { return false };
            shell().language = lang.to_string();
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
            let (Some(id), Some(key)) = (f64_of(&action, "id"), text_of::<64>(&action, "key")) else { return false };
            if !preview_open(id as u32, &key) {
                return false;
            }
        }
        "preview_close" => {
            let Some(id) = f64_of(&action, "id") else { return false };
            preview_close(id as u32);
        }
        "search_query" => {
            let Some(query) = text_of::<1024>(&action, "query") else { return false };
            search_query(&query);
        }
        "search_close" => search_close(),
        "ledger_report" => shell().report = Some(ledger_report_text()),
        "ledger_logging" => {
            let Some(on) = bool_of(&action, "on") else { return false };
            ledger_set_logging(on);
        }
        "ledger_window_start" => ledger_window_start(),
        _ => return false,
    }
    request_world_tick();
    true
}

/// The shell's turn: a pending search jump re-grounded once its page's terrain
/// has arrived, if the map is still on that page.
pub fn shell_step() {
    let Some(target) = shell().go_to else { return };
    if get_ele(target.i, target.f).is_none() {
        return;
    }
    shell().go_to = None;
    if use_map_store().map_center.is_some_and(|c| L3_IDX(c.i) == L3_IDX(target.i)) {
        use_map_store().set_map_center(ground_or_raw(&target));
    }
}
