// pxl-kernel/door/apply/surface.rs

//! SURFACE: a surface's: opened, resized, closed, and the frame drawn into the surfaces.

use super::State;
use super::{given, or_refuse, refuse_unless};
use crate::domain::control::camera::input::camera_view_resize;
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::control::sensor;
use crate::domain::view::frame::resolution;
use crate::sys::sched::clock;
use crate::sys::sched::edge::request_world_tick;
use crate::turn;
use crate::turn::world_platform;
use crate::turn::HostFrame;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    _s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SURFACE_FRAME => {
            let p = abi::SurfaceFrame::read(given(pkt)?)?;
            refuse_unless(
                p.refresh_ms >= 0.0 && p.paced_ms >= 0.0 && p.unix_ms.is_finite(),
                Errno::EINVAL,
            )?;
            // The step is the one clock's change since the last frame: none on
            // the first, and none where the clock was set back.
            let last = clock::wall_now();
            let delta = if last > 0.0 {
                ((p.unix_ms - last) / 1000.0).max(0.0)
            } else {
                0.0
            };
            turn::on_world_frame_asked(HostFrame {
                delta,
                refresh_ms: p.refresh_ms,
                paced_ms: p.paced_ms,
                interactive: p.interactive,
                credits: p.credits,
            });
            clock::set_wall_now(p.unix_ms);
            Ok(Some(0))
        }
        Command::SURFACE_OPEN => {
            let role = abi::SurfaceOpen::read(given(pkt)?)?.role;
            let handle = surface::surface_open(role);
            if handle != 0 {
                // The sensors serve the world's surface: watched while it is open.
                if role == surface_role::WORLD {
                    let platform = world_platform();
                    sensor::sensors_open(platform.geolocation, platform.orient);
                }
                request_world_tick();
            }
            Ok(Some(handle as i32))
        }
        Command::SURFACE_RESIZE => {
            let p = abi::SurfaceResize::read(given(pkt)?)?;
            refuse_unless(p.device_pixel_ratio > 0.0, Errno::EINVAL)?;
            let role = surface::surface(p.surface).map_or(0, |s| s.role);
            let ratio = resolution::surface_ratio(role, [p.width, p.height], p.device_pixel_ratio);
            refuse_unless(
                surface::surface_resize(p.surface, p.width, p.height, p.device_pixel_ratio, ratio),
                Errno::EBADF,
            )?;
            // The world's view is its surface's layout box, in CSS pixels.
            if role == surface_role::WORLD {
                camera_view_resize(
                    f64::from(p.width) / p.device_pixel_ratio,
                    f64::from(p.height) / p.device_pixel_ratio,
                );
            }
            request_world_tick();
            Ok(Some(0))
        }
        Command::SURFACE_CLOSE => {
            let surface = abi::SurfaceClose::read(given(pkt)?)?.surface;
            let role = or_refuse(surface::surface(surface).map(|s| s.role), Errno::EBADF)?;
            super::super::close_surface(surface);
            if role == surface_role::WORLD {
                sensor::sensors_close();
            }
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
