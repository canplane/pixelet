// pxl_core/module/world/world.rs

//! The world as the kernel runs it: every layer's turn, in one place.
//!
//! Structure:
//! 1. Environment: terrain, POI
//! 2. Content: placed voxels and user objects
//! 3. Interaction: highlighters and guide markers
//!
//! Coordinate model: every layer places what it has in the root frame
//! (module/world/frame/view.rs), the map's azimuth and the floating origin's
//! translation, which the view's damping steps write.
//!
//! Two kinds of turn. The tick's steps (sched/tick.rs) run inside the frame, by
//! phase and under one budget; `world_boot` registers them once. The effects —
//! the fetchers, the stores' followers, the markers — run once per turn before
//! the tick (`world_turn`), each comparing what it last saw, the way their
//! React effects compared their dependencies.

use pxl_shared::spec::space::indexer::{L1_IDX_IN_L4, L3_IDX};
use pxl_shared::spec::space::pvf3::projection;
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::sched::quantum::Grant;

use crate::app::setting::store::use_settings_store;
use crate::global::{now, Global};
use crate::lib::env::config::APP_ENV;
use crate::module::camera::input::camera_input_step;
use crate::module::camera::use_camera::camera_step;
use crate::module::device::pose::use_device_orientation::{orientation_watchdog_step, OrientationPlatform};
use crate::module::device::pose::use_geolocation::geolocation_step;
use crate::module::map::environment::map_environment_step;
use crate::module::map::navigation::use_follow_mode::follow_mode_step;
use crate::module::map::navigation::use_map_bootstrap::map_bootstrap_step;
use crate::module::map::navigation::use_map_navigation::use_map_navigation;
use crate::module::map::store::use_map_store;
use crate::module::map::ui::overlay::use_reverse_geocode::reverse_geocode_step;
use crate::module::map::ui::toast::toast_controller::toast_controller_step;
use crate::module::world::character::companion::{companion_step, CompanionProps};
use crate::module::world::chunk_node::chunk_nodes_step;
use crate::module::world::core_tick::{chunk_mounts_step, core_tick_report_step, core_tick_visible_step, set_host_task_ms};
use crate::module::world::decor::asset_store::decor_asset_step;
use crate::module::world::environment::theme_clock::theme_clock_step;
use crate::module::world::frame::coord::get_local_world_origin;
use crate::module::world::frame::use_view_frame::{view_rotation_step, view_translation_step};
use crate::module::world::marker::debug_gizmos::debug_gizmos_step;
use crate::module::world::marker::hover_cursor::hover_cursor_step;
use crate::module::world::marker::location_puck::location_puck_step;
use crate::module::world::object::teach::teach_objects;
use crate::module::world::poi::poi_layer::{poi_index_step, poi_layer_watch_step, poi_place_tick_step};
use crate::module::world::poi::use_poi_fetcher::poi_fetcher_step;
use crate::module::world::sched::fetch_scheduler::{fetch_pump, set_fetch_credits};
use crate::module::world::sched::tick::{on_tick, run_tick, TickContext, TickReport, TickState, CONTENT, COSMETIC, INTERACTION, REPORT, VISIBLE};
use crate::module::world::change::take_tick_request;
use crate::module::world::terrain::teach::teach_terrain;
use crate::module::world::terrain::use_ground_snap_sync::ground_snap_sync_step;
use crate::module::world::terrain::use_tile_fetcher::tile_fetcher_step;

/// What the platform offers the sensors, as the shell found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub geolocation: bool,
    pub orientation: OrientationPlatform,
}

static BOOTED: Global<bool> = Global::new();

/// The companion's place, from map_center in the local floating-origin frame —
/// wherever the camera actually is, not the L4 tile's centre up to 2048u away.
fn companion_props() -> Option<CompanionProps> {
    let map_center = use_map_store().map_center?;
    let map_origin = get_local_world_origin(Some(&map_center));
    let mut local = [0.0; 3];
    projection::unproject(&mut local, L1_IDX_IN_L4(map_center.i), map_center.f);
    Some(CompanionProps {
        origin_page_idx_xy: TO_XY(L3_IDX(map_origin)),
        map_origin,
        center_x: local[0],
        center_y: local[1],
        center_z: local[2],
    })
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_props());
}

fn debug_gizmos_tick(_: &mut TickContext) {
    let map_center = use_map_store().map_center;
    let show = APP_ENV() != "production" && use_settings_store().show_debug_objects && map_center.is_some();
    debug_gizmos_step(show, map_center, get_local_world_origin(map_center.as_ref()));
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step();
}

fn environment_tick(_: &mut TickContext) {
    map_environment_step();
}

/// Teaches the build its tables — before the first page is told — and registers
/// the tick's steps. Once.
pub fn world_boot() {
    let booted = BOOTED.get(|| false);
    if *booted {
        return;
    }
    *booted = true;
    let (a, t) = (crate::probe::mark(), crate::probe::top());
    teach_terrain();
    let (a, t) = (crate::probe::add(27, a), crate::probe::add_bytes(27, t));
    teach_objects();
    crate::probe::add(28, a);
    crate::probe::add_bytes(28, t);

    on_tick(INTERACTION, camera_input_tick, "input");
    on_tick(INTERACTION, camera_step, "camera");
    on_tick(INTERACTION, environment_tick, "environment");
    on_tick(INTERACTION, view_rotation_step, "view_rotation");
    on_tick(INTERACTION, view_translation_step, "view_translation");
    on_tick(INTERACTION, companion_tick, "companion");
    on_tick(INTERACTION, crate::module::world::character::character_preview::preview_step, "preview");
    on_tick(VISIBLE, core_tick_visible_step, "chunks");
    on_tick(CONTENT, poi_index_step, "poi_index");
    on_tick(CONTENT, poi_place_tick_step, "poi_place");
    on_tick(COSMETIC, debug_gizmos_tick, "debug_gizmos");
    on_tick(REPORT, core_tick_report_step, "ledger");
}

/// The effects' turn, before the tick.
pub fn world_turn(platform: Platform) {
    world_boot();
    map_bootstrap_step();
    crate::app::shell::shell_step();
    geolocation_step(platform.geolocation);
    orientation_watchdog_step(platform.orientation);
    let a = crate::probe::mark();
    tile_fetcher_step();
    let a = crate::probe::add(20, a);
    poi_fetcher_step();
    crate::probe::add(21, a);
    let (a, t) = (crate::probe::mark(), crate::probe::top());
    ground_snap_sync_step();
    follow_mode_step();
    theme_clock_step();
    poi_layer_watch_step();
    let (a, t) = (crate::probe::add(30, a), crate::probe::add_bytes(30, t));
    decor_asset_step();
    crate::probe::add(29, a);
    crate::probe::add_bytes(29, t);
    let a = crate::probe::mark();
    chunk_mounts_step();
    let a = crate::probe::add(22, a);
    chunk_nodes_step();
    crate::probe::add(23, a);
    hover_cursor_step();
    // device_coord is deliberately not gated the way map_center is — the puck
    // holds its own last-known coordinate across a transient null.
    location_puck_step(use_map_navigation().device_coord);
    reverse_geocode_step();
    crate::module::map::ui::overlay::search_modal::search_step();
    toast_controller_step();
    crate::module::world::object::store::flush_save_if_due();
    fetch_pump();
}

static TICK_STATE: Global<TickState> = Global::new();
static PLATFORM: Global<Option<Platform>> = Global::new();

/// Tells the kernel the platform before its first frame.
pub fn set_world_platform(platform: Platform) {
    *PLATFORM.get(|| None) = Some(platform);
}

/// The platform as the last frame was told it.
pub fn world_platform() -> Platform {
    PLATFORM.get(|| None).unwrap_or(Platform {
        geolocation: false,
        orientation: OrientationPlatform {
            supported: false,
            needs_permission: false,
        },
    })
}

/// The turn the central scheduler strobed: the effects, then the tick's phases
/// on its grant, and the fetches they asked for within its credits. What the
/// tick spent, and whether a turn after this one was asked for.
pub fn world_frame(width: f64, height: f64, dpr: f64, delta: f64, platform: Platform, grant: Grant, credits: u32) -> TickReport {
    let t0 = now();
    *PLATFORM.get(|| None) = Some(platform);
    set_fetch_credits(credits);
    world_turn(platform);
    let state = TICK_STATE.get(TickState::default);
    state.width = width;
    state.height = height;
    state.dpr = dpr;
    state.elapsed_s += delta;
    let (a, t) = (crate::probe::mark(), crate::probe::top());
    let mut report = run_tick(state, delta, grant);
    crate::probe::add(31, a);
    crate::probe::add_bytes(31, t);
    fetch_pump();
    crate::render::gpu::stage::stage_write();
    crate::render::gpu::renderer::render_frame();
    set_host_task_ms(now() - t0);
    report.invalidated |= take_tick_request();
    report
}
