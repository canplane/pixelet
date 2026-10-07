// pxl-kernel/door/apply.rs

//! The shell's commands, applied.

use super::{post_shell, State};
use crate::abi::cmd::{self as abi, Command};
use pxl_spec::space::coord::zoom::Zoom;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_guest::abi::cmd as xpute_cmd;
use xpute_guest::mem::base::{off_of, off_of_slice};

fn packet<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    or_refuse(*pkt, Errno::EBADMSG)
}

/// What the decoder cannot see, a value out of the range the kernel takes or a
/// handle it does not hold, is refused here: past the door nothing checks it again.
fn or_refuse<T>(v: Option<T>, errno: Errno) -> Result<T, MarshalError> {
    v.ok_or_else(|| MarshalError::new(errno))
}

fn refuse_unless(valid: bool, errno: Errno) -> Result<(), MarshalError> {
    or_refuse(valid.then_some(()), errno)
}

fn on_earth(lon: f64, lat: f64) -> bool {
    (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat)
}

/// A bearing in degrees, as a compass or a course gives it.
fn bearing(deg: Option<f64>) -> bool {
    deg.is_none_or(|d| (0.0..=360.0).contains(&d))
}

/// A UI command: asks for a turn and raises what it asks of the host before
/// returning, so a sensor close sent after the strobe stopped is still heard.
fn applied(s: &mut State) -> Result<Option<i32>, MarshalError> {
    crate::sched::edge::request_world_tick();
    post_shell(s);
    Ok(Some(0))
}

/// Applies one submission. None (replied as ENOSYS) for an unknown command
/// or a signal submitted as one.
pub(super) fn apply(s: &mut State, number: u32, pkt: &Option<&[u8]>) -> Result<Option<i32>, MarshalError> {
    use crate::control::camera::input::{camera_input_rect, camera_keys, camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up, camera_rehover, camera_wheel, Rect};
    use crate::control::interaction::{hold, holding};
    use crate::control::sensor::{device_orientation, geolocation};
    use crate::profiler::log::{profiler_follow, profiler_set_logging, profiler_window_start, profiler_window_stop};
    use crate::sched::edge::request_world_tick;
    use crate::setting::store::{settings_load, settings_store, SettingKey};
    use crate::shell as ui;
    use crate::turn::world_platform;

    if let Some(cmd) = xpute_cmd::Command::of(number) {
        return apply_xpute(cmd, pkt);
    }
    let Some(cmd) = Command::of(number) else { return Ok(None) };
    match cmd {
        Command::WORLD_FRAME => {
            let p = abi::WorldFrame::read(packet(pkt)?)?;
            refuse_unless(p.dpr > 0.0 && p.delta_s >= 0.0 && p.frame_ms >= 0.0, Errno::EINVAL)?;
            crate::turn::world_frame_asked(crate::turn::HostFrame {
                width: p.width,
                height: p.height,
                dpr: p.dpr,
                delta: p.delta_s,
                frame_ms: p.frame_ms,
                interactive: p.interactive,
                credits: p.credits,
            });
            crate::io::host::set_wall_now(p.unix_ms);
            Ok(Some(0))
        }
        Command::WORLD_BOOT => {
            let p = abi::WorldBoot::read(packet(pkt)?)?;
            super::host_attached(s);
            crate::io::host::set_development(p.development);
            crate::io::host::set_handheld(p.handheld);
            crate::turn::set_world_platform(crate::turn::Platform {
                geolocation: p.geolocation,
                orientation: device_orientation::OrientationPlatform {
                    supported: p.orientation,
                    needs_permission: p.orientation_permission,
                    silence_is_loss: p.orientation_silence_is_loss,
                },
            });
            settings_load(p.settings);
            profiler_follow(settings_store().debug_overlay, crate::profiler::summary::counters());
            crate::layer::object::repository::open();
            // The shell holds the kernel's state from its first render on.
            applied(s)
        }
        Command::WORLD_SOURCE => {
            let p = abi::WorldSource::read(packet(pkt)?)?;
            refuse_unless(
                p.top_z >= u32::from(pxl_spec::space::layer::lattice::GROUND.top()) && p.top_z <= u32::from(pxl_spec::space::layer::tile::LEAF_ZOOM),
                Errno::EINVAL,
            )?;
            crate::layer::residency::floor::declare(p.top_z as Zoom);
            Ok(Some(0))
        }
        Command::INPUT_RECT => {
            let p = abi::InputRect::read(packet(pkt)?)?;
            refuse_unless(p.width >= 0.0 && p.height >= 0.0, Errno::EINVAL)?;
            camera_input_rect(Rect {
                left: p.left,
                top: p.top,
                width: p.width,
                height: p.height,
            });
            Ok(Some(0))
        }
        Command::INPUT_POINTER => {
            let p = abi::InputPointer::read(packet(pkt)?)?;
            match p.phase {
                abi::PointerPhase::Down => camera_pointer_down(p.kind, p.id, p.button, p.x, p.y),
                abi::PointerPhase::Move => camera_pointer_move(p.kind, p.id, p.x, p.y),
                abi::PointerPhase::Up => {
                    if let Some([x, y]) = camera_pointer_up(p.kind, p.id) {
                        tap_at(x, y);
                    }
                }
                abi::PointerPhase::Out => camera_pointer_out(),
            }
            Ok(Some(0))
        }
        Command::INPUT_HOLD => {
            use pxl_spec::cell::object::placeable_cube_of;
            use pxl_spec::cell::DecorId;
            let p = abi::InputHold::read(packet(pkt)?)?;
            let mut holding = holding();
            holding.active_tool = p.tool;
            holding.object_tier = p.tier;
            match p.tool {
                abi::ActiveTool::Object => {
                    holding.object_cube = or_refuse(placeable_cube_of(p.value), Errno::EINVAL)?;
                }
                abi::ActiveTool::Decor => {
                    holding.decor_id = or_refuse(u16::try_from(p.value).ok().and_then(DecorId::of), Errno::EINVAL)?;
                }
                abi::ActiveTool::Pointer | abi::ActiveTool::Eraser => {}
            }
            hold(holding);
            camera_rehover();
            Ok(Some(0))
        }
        Command::INPUT_WHEEL => {
            camera_wheel(abi::InputWheel::read(packet(pkt)?)?.delta_y);
            Ok(Some(0))
        }
        Command::INPUT_KEYS => {
            let p = abi::InputKeys::read(packet(pkt)?)?;
            camera_keys(p.x, p.y);
            Ok(Some(0))
        }
        Command::INPUT_TRACKING => {
            ui::tracking_action(abi::InputTracking::read(packet(pkt)?)?.tracking);
            request_world_tick();
            Ok(Some(0))
        }
        Command::INPUT_VIEW => {
            let p = abi::InputView::read(packet(pkt)?)?;
            ui::set_view(p.dist, p.polar);
            applied(s)
        }
        Command::INPUT_COMPASS_RESET => {
            // Following the heading would turn it straight back.
            if !crate::control::navigation::follow_mode::following() {
                crate::control::camera::controller::reset_azimuth();
            }
            applied(s)
        }
        Command::INPUT_GO_TO => {
            let p = abi::InputGoTo::read(packet(pkt)?)?;
            refuse_unless(on_earth(p.lon, p.lat), Errno::EINVAL)?;
            ui::go_to_degrees(p.lon, p.lat);
            applied(s)
        }
        Command::INPUT_POSE => {
            let p = abi::InputPose::read(packet(pkt)?)?;
            // A unit quaternion by the protocol; one of no length turns nothing.
            refuse_unless(p.qx * p.qx + p.qy * p.qy + p.qz * p.qz + p.qw * p.qw > 0.0, Errno::EINVAL)?;
            crate::control::camera::controller::tracked_pose([p.x, p.y, p.z], [p.qx, p.qy, p.qz, p.qw]);
            applied(s)
        }
        Command::INPUT_POSE_END => {
            crate::control::camera::controller::tracked_end();
            applied(s)
        }
        Command::SENSOR_POSITION => {
            let p = abi::SensorPosition::read(packet(pkt)?)?;
            refuse_unless(on_earth(p.lon, p.lat) && p.accuracy.is_none_or(|a| a >= 0.0) && bearing(p.course), Errno::EINVAL)?;
            // An accuracy the device does not give is no accuracy at all.
            geolocation::geolocation_fix(p.lon, p.lat, p.altitude, p.accuracy.unwrap_or(f64::INFINITY), p.course);
            Ok(Some(0))
        }
        Command::SENSOR_ERROR => {
            geolocation::geolocation_error(abi::SensorError::read(packet(pkt)?)?.code);
            Ok(Some(0))
        }
        Command::SENSOR_ORIENTATION => {
            let p = abi::SensorOrientation::read(packet(pkt)?)?;
            // DeviceOrientationEvent's ranges, closed: a reading at an end is still one.
            let within = |v: Option<f64>, half: f64| v.is_none_or(|v| (-half..=half).contains(&v));
            refuse_unless(bearing(p.webkit_compass_heading) && bearing(p.alpha) && within(p.beta, 180.0) && within(p.gamma, 90.0), Errno::EINVAL)?;
            device_orientation::orientation_event(p.webkit_compass_heading, p.alpha, p.beta, p.gamma);
            Ok(Some(0))
        }
        Command::SENSOR_ORIENTATION_LOST => {
            device_orientation::orientation_lost();
            Ok(Some(0))
        }
        Command::SENSOR_PERMISSION => {
            let p = abi::SensorPermission::read(packet(pkt)?)?;
            match p.sensor {
                abi::Sensor::Geolocation => {
                    if p.answer == abi::PermissionAnswer::Granted {
                        geolocation::geolocation_permission_granted(world_platform().geolocation);
                    }
                }
                abi::Sensor::Orientation => device_orientation::orientation_permission(p.answer, world_platform().orientation),
            }
            Ok(Some(0))
        }
        Command::WORLD_VISIBILITY => {
            let visible = abi::WorldVisibility::read(packet(pkt)?)?.visible;
            geolocation::geolocation_visibility(visible, world_platform().geolocation);
            if visible {
                device_orientation::orientation_visible();
            } else {
                crate::layer::object::repository::flush();
            }
            Ok(Some(0))
        }
        Command::SENSOR_ENABLE_LOCATION => {
            ui::location_enable();
            applied(s)
        }
        Command::SENSOR_OPEN => {
            ui::sensors_open();
            applied(s)
        }
        Command::SENSOR_CLOSE => {
            ui::sensors_close();
            applied(s)
        }
        Command::IMAGE_DECODED => {
            let p = abi::ImageDecoded::read(packet(pkt)?)?;
            crate::io::load::image_decoded(p.image, crate::io::host::answered(p.res));
            Ok(Some(0))
        }
        Command::GPU_READY => {
            let p = abi::GpuReady::read(packet(pkt)?)?;
            let align = p.min_uniform_offset_alignment;
            refuse_unless(align.is_power_of_two() && align <= pxl_render::gpu::renderer::UNIFORM_ALIGN_MAX, Errno::EINVAL)?;
            pxl_render::gpu::renderer::gpu_created(p.min_uniform_offset_alignment, p.timestamps);
            request_world_tick();
            Ok(Some(0))
        }
        Command::GPU_LOST => {
            pxl_render::gpu::renderer::gpu_lost();
            crate::io::load::images_lost();
            Ok(Some(0))
        }
        Command::SURFACE_OPEN => {
            let handle = pxl_render::gpu::surface::surface_open(abi::SurfaceOpen::read(packet(pkt)?)?.role);
            if handle != 0 {
                request_world_tick();
            }
            Ok(Some(handle as i32))
        }
        Command::SURFACE_RESIZE => {
            let p = abi::SurfaceResize::read(packet(pkt)?)?;
            refuse_unless(p.device_pixel_ratio > 0.0, Errno::EINVAL)?;
            let role = pxl_render::gpu::surface::surface(p.surface).map_or(0, |s| s.role);
            let ratio = crate::view::frame::resolution::surface_ratio(role, [p.width, p.height], p.device_pixel_ratio);
            refuse_unless(pxl_render::gpu::surface::surface_resize(p.surface, p.width, p.height, p.device_pixel_ratio, ratio), Errno::EBADF)?;
            request_world_tick();
            Ok(Some(0))
        }
        Command::SURFACE_CLOSE => {
            let surface = abi::SurfaceClose::read(packet(pkt)?)?.surface;
            refuse_unless(pxl_render::gpu::surface::surface(surface).is_some(), Errno::EBADF)?;
            super::close_surface(surface);
            Ok(Some(0))
        }
        Command::SETTING_SET => {
            let p = abi::SettingSet::read(packet(pkt)?)?;
            let changed = settings_store().set_setting(p.key, p.value);
            refuse_unless(changed.is_some(), Errno::EINVAL)?;
            if changed == Some(true) {
                if p.key.sources_chunks() {
                    crate::chunk::source::chunk_sources_changed();
                }
                if p.key == SettingKey::debug_overlay {
                    profiler_follow(p.value == 1, crate::profiler::summary::counters());
                }
            }
            applied(s)
        }
        Command::CHAT_SEND => {
            crate::scene::character::chat_bubble::append_chat_line(abi::ChatSend::read(packet(pkt)?)?.text);
            applied(s)
        }
        Command::PREVIEW_OPEN => {
            let p = abi::PreviewOpen::read(packet(pkt)?)?;
            refuse_unless(pxl_render::gpu::surface::surface(p.surface).is_some(), Errno::EBADF)?;
            refuse_unless(crate::scene::character::preview::preview_open(p.surface, p.group, p.number), Errno::EINVAL)?;
            applied(s)
        }
        Command::PREVIEW_CLOSE => {
            refuse_unless(crate::scene::character::preview::preview_close(abi::PreviewClose::read(packet(pkt)?)?.surface), Errno::EBADF)?;
            applied(s)
        }
        // Records only while the profiler setting is on: a window opened with the
        // panel off would record with nothing to show or stop it.
        Command::PROFILER_START => {
            refuse_unless(settings_store().debug_overlay, Errno::EPERM)?;
            profiler_window_start(crate::profiler::summary::counters());
            applied(s)
        }
        Command::PROFILER_STOP => {
            profiler_window_stop();
            applied(s)
        }
        Command::PROFILER_LOG => {
            let on = abi::ProfilerLog::read(packet(pkt)?)?.on;
            refuse_unless(!on || settings_store().debug_overlay, Errno::EPERM)?;
            profiler_set_logging(on);
            applied(s)
        }
        // Memory is at most 1 GiB, so an address fits an i32.
        Command::PROFILER_READ => Ok(Some(profiler_read(abi::ProfilerRead::read(packet(pkt)?)?.window_ms) as i32)),
        Command::PROFILER_TAKE => Ok(Some(profiler_take(s) as i32)),
        Command::WORLD_SNAPSHOT | Command::SENSOR_START | Command::SENSOR_STOP => Ok(None),
    }
}

fn apply_xpute(cmd: xpute_cmd::Command, pkt: &Option<&[u8]>) -> Result<Option<i32>, MarshalError> {
    match cmd {
        xpute_cmd::Command::NOP => {
            let value = match pkt {
                Some(pkt) => xpute_cmd::Nop::read(pkt)?.value,
                None => 0,
            };
            Ok(Some(value as i32))
        }
        xpute_cmd::Command::SYS_OPENED => {
            let p = xpute_cmd::SysOpened::read(packet(pkt)?)?;
            crate::io::file::opened(p.fd, p.res, p.detail);
            Ok(Some(0))
        }
    }
}

/// The address of READ's words (profiler.json): the summary over
/// `window_ms` and the three rings. Read in place rather than posted:
/// `Door::post` asserts on a full completion ring, and reading allocates
/// nothing in the heap being measured. Valid until the next turn.
fn profiler_read(window_ms: Option<f64>) -> u32 {
    use crate::profiler::record::{profiler_frames, profiler_tasks, profiler_worst};
    use crate::profiler::spec::READ;
    // On the heap, not a static: a native build's statics are outside the shell-readable memory.
    static WORDS: xpute_guest::global::Global<Box<[u32; READ::WORDS as usize]>> = xpute_guest::global::Global::new();
    let summary = crate::profiler::summary::profiler_summarize(window_ms);
    let (frames, tasks, worst) = (profiler_frames(), profiler_tasks(), profiler_worst());
    let w = WORDS.get(|| Box::new([0; READ::WORDS as usize]));
    w[READ::summary_at as usize] = off_of_slice(&raw const *summary) as u32;
    w[READ::summary_words as usize] = summary.len() as u32;
    w[READ::frames_at as usize] = off_of_slice(&raw const *frames) as u32;
    w[READ::frames_count as usize] = frames.len() as u32;
    w[READ::tasks_at as usize] = off_of_slice(&raw const *tasks) as u32;
    w[READ::tasks_count as usize] = tasks.len() as u32;
    w[READ::worst_at as usize] = off_of_slice(&raw const *worst) as u32;
    w[READ::worst_count as usize] = worst.len() as u32;
    off_of(w.as_ptr()) as u32
}

/// The address of LOG's words: rows the live stream kept since the last
/// take, let go next turn.
fn profiler_take(s: &mut State) -> u32 {
    use crate::profiler::spec::LOG;
    // On the heap, as READ's words are.
    static WORDS: xpute_guest::global::Global<Box<[u32; LOG::WORDS as usize]>> = xpute_guest::global::Global::new();
    let (tasks, frames) = crate::profiler::log::profiler_log_rows();
    let w = WORDS.get(|| Box::new([0; LOG::WORDS as usize]));
    w[LOG::tasks_at as usize] = off_of_slice(&raw const *tasks) as u32;
    w[LOG::tasks_count as usize] = tasks.len() as u32;
    w[LOG::frames_at as usize] = off_of_slice(&raw const *frames) as u32;
    w[LOG::frames_count as usize] = frames.len() as u32;
    s.profiler_taken = true;
    off_of(w.as_ptr()) as u32
}

#[cfg(test)]
#[path = "apply.test.rs"]
mod test;

/// A badge takes a tap before the ground under it, with the pointer tool only.
fn tap_at(x: f64, y: f64) {
    use crate::chunk::place::Went;
    use crate::control::navigation::destination::{go_into_place, go_to_place};
    match crate::control::interaction::taps_badges().then(|| crate::chunk::place::tap(x, y)).flatten() {
        Some(Went::To(at)) => go_to_place(at),
        Some(Went::Into(at)) => go_into_place(at),
        None => crate::control::interaction::tap(x, y),
    }
}
