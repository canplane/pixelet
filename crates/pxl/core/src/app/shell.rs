// pxl_core/app/shell.rs

//! What the page's UI reads of the kernel and what it asks of it. The UI is an
//! interrupt source and a display: it renders one snapshot of the state it
//! shows — a JSON document the kernel writes after each turn, which the shell
//! compares whole and hands to React only when it changed — and every control
//! it has is an action by name, applied here to the stores.

use pxl_shared::spec::cell::decor_ids;
use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::app::setting::schema::{SettingValue, SETTING_KEYS};
use crate::app::setting::store::use_settings_store;
use crate::app::store::ui_store::{use_ui_store, ActiveTool, RecentPick};
use crate::global::Global;
use crate::json::{parse, Json};
use crate::module::camera::store::{use_camera_store, CameraStatus};
use crate::module::device::pose::store::{use_device_location_store, use_device_orientation_store, DevicePoseStatus};
use crate::module::device::pose::use_device_orientation::{orientation_subscribe, orientation_unsubscribe, request_orientation, OrientationPlatform};
use crate::module::device::pose::use_geolocation::{geolocation_subscribe, geolocation_unsubscribe, start_watching};
use crate::module::map::navigation::use_map_navigation::use_map_navigation;
use crate::module::map::store::use_map_store;
use crate::module::map::ui::control::compass::{compass_reset, show_compass};
use crate::module::map::ui::control::scale_bar::scale_bar;
use crate::module::map::ui::overlay::use_reverse_geocode::use_reverse_geocode;
use crate::module::world::character::store::use_companion_load_store;
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

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

fn opt_str(s: Option<&str>) -> String {
    s.map_or("null".to_string(), quote)
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

/// The state the UI shows, as one JSON document.
pub fn ui_snapshot_json() -> String {
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

    let recent: Vec<String> = ui
        .recent_picks
        .iter()
        .map(|p| match p {
            RecentPick::Object(id) => format!("{{\"kind\":\"object\",\"id\":{}}}", quote(id)),
            RecentPick::Decor(id) => format!("{{\"kind\":\"decor\",\"id\":{}}}", quote(id)),
        })
        .collect();
    let chat: Vec<String> = ui.chat_lines.iter().map(|l| format!("{{\"id\":{},\"text\":{},\"ts\":{}}}", l.id, quote(&l.text), num(l.ts))).collect();
    let setting_values: Vec<String> = SETTING_KEYS()
        .into_iter()
        .filter_map(|key| {
            settings.get(key).map(|value| {
                let v = match value {
                    SettingValue::Toggle(b) => b.to_string(),
                    SettingValue::Text(t) => quote(&t),
                };
                format!("{}:{v}", quote(key))
            })
        })
        .collect();

    let mut out = String::with_capacity(2048);
    out.push('{');
    out.push_str(&format!(
        "\"map\":{{\"has_center\":{},\"center_lonlat\":{},\"status\":{},\"is_off_center\":{}}},",
        map.map_center.is_some(),
        map.map_center.map_or("null".to_string(), |c| {
            let mut g = [0.0; 3];
            epsg4326::unproject(&mut g, c.i, c.f);
            format!("[{},{}]", num(g[0]), num(g[1]))
        }),
        quote(map.map_status.as_str()),
        nav.is_off_center
    ));
    out.push_str(&format!(
        "\"location\":{{\"status\":{},\"error_key\":{},\"has_location\":{}}},",
        quote(pose_status_str(location.status)),
        opt_str(location.error_key.as_deref()),
        location.location.is_some()
    ));
    out.push_str(&format!(
        "\"orientation\":{{\"status\":{},\"heading\":{}}},",
        quote(pose_status_str(orientation.status)),
        orientation.orientation.and_then(|o| o.heading).map_or("null".to_string(), num)
    ));
    out.push_str(&format!(
        "\"view\":{{\"dist\":{},\"azimuth\":{},\"polar\":{}}},\"is_follow_mode\":{},",
        num(camera.view_state.dist),
        num(camera.view_state.azimuth),
        num(camera.view_state.polar),
        camera.is_follow_mode
    ));
    out.push_str(&format!(
        "\"ui\":{{\"active_tool\":{},\"object_color_id\":{},\"object_size\":{},\"decor_id\":{},\"recent_picks\":[{}],\"chat_lines\":[{}]}},",
        quote(active_tool_str(ui.active_tool)),
        quote(&ui.object_color_id),
        num(ui.object_size),
        quote(ui.decor_id),
        recent.join(","),
        chat.join(",")
    ));
    out.push_str(&format!("\"settings\":{{{}}},", setting_values.join(",")));
    out.push_str(&format!("\"companion\":{{\"person_ready\":{},\"pet_ready\":{}}},", companion.person_ready, companion.pet_ready));
    out.push_str(&format!("\"address\":{{\"label\":{},\"loading\":{}}},", opt_str(address.label.as_deref()), address.loading));
    out.push_str(&format!(
        "\"scale_bar\":{{\"total_meters\":{},\"bar_width_px\":{},\"is_default_zoom\":{},\"label\":{}}},",
        num(bar.total_meters),
        num(bar.bar_width_px),
        bar.is_default_zoom,
        quote(&bar.label)
    ));
    out.push_str(&format!("\"compass\":{{\"show\":{}}},", show_compass()));
    out.push_str(&format!("\"camera_busy\":{},\"fog\":{},", camera.camera_status != CameraStatus::Idle, quote(themes(use_theme_key()).fog)));
    out.push_str(&format!("\"report\":{}", opt_str(shell().report.take().as_deref())));
    out.push('}');
    out
}

fn str_of<'a>(action: &'a Json, key: &str) -> Option<&'a str> {
    action.get(key).and_then(Json::as_str)
}

fn f64_of(action: &Json, key: &str) -> Option<f64> {
    action.get(key).and_then(Json::as_f64)
}

fn bool_of(action: &Json, key: &str) -> Option<bool> {
    match action.get(key) {
        Some(Json::Bool(b)) => Some(*b),
        _ => None,
    }
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
    let Some(action) = parse(text) else { return false };
    let Some(kind) = str_of(&action, "type") else { return false };
    match kind {
        "set_setting" => {
            let (Some(key), Some(value)) = (str_of(&action, "key"), action.get("value")) else { return false };
            let value = match value {
                Json::Bool(b) => SettingValue::Toggle(*b),
                Json::String(s) => SettingValue::Text(s.clone()),
                _ => return false,
            };
            use_settings_store().set_setting(key, value);
        }
        "toggle_setting" => {
            let Some(key) = str_of(&action, "key") else { return false };
            use_settings_store().toggle_setting(key);
        }
        "set_active_tool" => {
            let Some(tool) = str_of(&action, "tool").and_then(active_tool_of) else { return false };
            use_ui_store().set_active_tool(tool);
        }
        "set_object_size" => {
            let Some(size) = f64_of(&action, "size") else { return false };
            use_ui_store().set_object_size(size);
        }
        "select_cube" => {
            let Some(id) = str_of(&action, "id") else { return false };
            use_ui_store().select_cube(id);
        }
        "select_decor" => {
            let Some(id) = str_of(&action, "id").and_then(|id| decor_ids.iter().copied().find(|d| *d == id)) else { return false };
            use_ui_store().select_decor(id);
        }
        "append_chat_line" => {
            let Some(text) = str_of(&action, "text") else { return false };
            use_ui_store().append_chat_line(text);
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
            let Some(lang) = str_of(&action, "lang") else { return false };
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
