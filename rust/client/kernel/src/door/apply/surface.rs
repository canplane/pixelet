// pxl-kernel/door/apply/surface.rs

//! SURFACE: a surface's: opened, resized and closed.

use super::{or_refuse, refuse_unless};
use crate::domain::control::camera::input::camera_view_resize;
use crate::domain::control::cmd as abi;
use crate::domain::control::sensor;
use crate::domain::view::frame::resolution;
use crate::sys::sched::edge::request_world_tick;
use crate::turn::world_platform;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_guest::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

/// The surface's handle, or 0 where none was opened. It draws nothing until
/// SCENE_DRAW (scene.rs `draw`).
pub(super) fn open() -> Outcome {
    let handle = surface::surface_open();
    if handle != 0 {
        request_world_tick();
    }
    Ok(Some(handle as i32))
}

/// The surface `handle` made to draw the world: its sensors watched while it
/// is open, and its view sized where its box is already known. False where it
/// is not open.
pub(super) fn draw_world(handle: u32) -> bool {
    let Some(s) = surface::surface(handle) else {
        return false;
    };
    s.role = surface_role::WORLD;
    let platform = world_platform();
    sensor::sensors_open(platform.geolocation, platform.orient);
    if s.px_w != 0 {
        s.ratio = resolution::surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
        camera_view_resize(f64::from(s.px_w) / s.dpr, f64::from(s.px_h) / s.dpr);
    }
    request_world_tick();
    true
}

pub(super) fn resize(c: abi::SurfaceResize) -> Outcome {
    refuse_unless(c.device_pixel_ratio > 0.0, Errno::EINVAL)?;
    let role = surface::surface(c.surface).map_or(0, |s| s.role);
    let ratio = resolution::surface_ratio(role, [c.width, c.height], c.device_pixel_ratio);
    refuse_unless(
        surface::surface_resize(c.surface, c.width, c.height, c.device_pixel_ratio, ratio),
        Errno::EBADF,
    )?;
    // The world's view is its surface's layout box, in CSS pixels.
    if role == surface_role::WORLD {
        camera_view_resize(
            f64::from(c.width) / c.device_pixel_ratio,
            f64::from(c.height) / c.device_pixel_ratio,
        );
    }
    request_world_tick();
    Ok(Some(0))
}

pub(super) fn close(c: abi::SurfaceClose) -> Outcome {
    let role = or_refuse(surface::surface(c.surface).map(|s| s.role), Errno::EBADF)?;
    super::super::close_surface(c.surface);
    if role == surface_role::WORLD {
        sensor::sensors_close();
    }
    Ok(Some(0))
}
