// pxl-kernel/door/apply.rs

//! The shell's commands, applied.

use super::State;
use crate::domain::control::cmd::{self as abi, Command};
use xpute_guest::abi::cmd as xpute_cmd;
use xpute_guest::mem::{off_of, off_of_slice};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

fn packet<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    or_refuse(*pkt, Errno::EBADMSG)
}

/// What the decoder cannot see, a value out of the range the kernel takes or a
/// handle it does not hold, is refused here: past the door nothing checks it again.
fn or_refuse<T>(v: Option<T>, errnum: Errno) -> Result<T, MarshalError> {
    v.ok_or_else(|| MarshalError::new(errnum))
}

fn refuse_unless(valid: bool, errnum: Errno) -> Result<(), MarshalError> {
    or_refuse(valid.then_some(()), errnum)
}

fn on_earth(lon: f64, lat: f64) -> bool {
    (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat)
}

/// A bearing in degrees, as a compass or a course gives it.
fn bearing(deg: Option<f64>) -> bool {
    deg.is_none_or(|d| (0.0..=360.0).contains(&d))
}

/// A UI command: asks for a turn, and has this one raise what it asks of the
/// host at its falling edge, so a sensor close sent after the strobe stopped is
/// still heard.
fn applied(s: &mut State) -> Result<Option<i32>, MarshalError> {
    crate::sys::sched::edge::request_world_tick();
    s.asked = true;
    Ok(Some(0))
}

/// Applies one submission. None (replied as ENOSYS) for an unknown command
/// or a signal submitted as one.
pub(super) fn apply(
    s: &mut State,
    number: u32,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    use crate::domain::control::camera::input::{
        camera_input_rect, camera_keys, camera_pointer_down, camera_pointer_move,
        camera_pointer_out, camera_pointer_up, camera_rehover, camera_wheel,
    };
    use crate::domain::control::interaction::{hold, holding};
    use crate::domain::control::sensor::{device_orientation, geolocation};
    use crate::domain::view::setting::store::{
        settings_loaded, settings_open, settings_store, SettingKey,
    };
    use crate::profiler::window::{profiler_follow, profiler_window_start, profiler_window_stop};
    use crate::shell as ui;
    use crate::sys::sched::edge::request_world_tick;
    use crate::turn::world_platform;

    if let Some(cmd) = xpute_cmd::Command::of(number) {
        return apply_xpute(cmd, pkt);
    }
    let Some(cmd) = Command::of(number) else {
        return Ok(None);
    };
    match cmd {
        Command::WORLD_FRAME => {
            let p = abi::WorldFrame::read(packet(pkt)?)?;
            refuse_unless(
                p.refresh_ms >= 0.0 && p.paced_ms >= 0.0 && p.unix_ms.is_finite(),
                Errno::EINVAL,
            )?;
            // The step is the one clock's change since the last frame: none on
            // the first, and none where the clock was set back.
            let last = crate::sys::sched::clock::wall_now();
            let delta = if last > 0.0 {
                ((p.unix_ms - last) / 1000.0).max(0.0)
            } else {
                0.0
            };
            crate::turn::world_frame_asked(crate::turn::HostFrame {
                width: p.width,
                height: p.height,
                delta,
                refresh_ms: p.refresh_ms,
                paced_ms: p.paced_ms,
                interactive: p.interactive,
                credits: p.credits,
            });
            crate::sys::sched::clock::set_wall_now(p.unix_ms);
            Ok(Some(0))
        }
        Command::WORLD_BOOT => {
            let p = abi::WorldBoot::read(packet(pkt)?)?;
            super::host_attached(s);
            crate::domain::view::setting::store::set_development(p.development);
            crate::turn::set_world_platform(crate::turn::Platform {
                geolocation: p.geolocation,
                orientation: device_orientation::OrientationPlatform {
                    supported: p.orientation,
                    needs_permission: p.orientation_permission,
                    silence_is_loss: p.orientation_silence_is_loss,
                },
            });
            // Both kept slots, read from a turn on: what waits on the settings,
            // the profiler and the snapshot's, starts once they are in (turn.rs).
            settings_open();
            crate::domain::layer::object::repository::open();
            // The shell holds the kernel's state from its first render on.
            applied(s)
        }
        Command::WORLD_SOURCE => {
            let p = abi::WorldSource::read(packet(pkt)?)?;
            refuse_unless(
                crate::sys::io::source::declare(p.top_zoom, p.format, p.shard.iter()).is_ok(),
                Errno::EINVAL,
            )?;
            Ok(Some(0))
        }
        Command::WORLD_KEPT_EMPTIED => {
            use crate::sys::io::file::{self, Kept};
            let p = abi::WorldKeptEmptied::read(packet(pkt)?)?;
            let kept = or_refuse(
                Kept::of_slot(p.slot).filter(|&k| file::refused(k)),
                Errno::EINVAL,
            )?;
            file::emptied(kept);
            // The settings write what they hold from the next keep on; the
            // objects are read again, empty, and saved from then on.
            match kept {
                Kept::Settings => crate::domain::view::setting::store::settings_emptied(),
                Kept::Objects => crate::domain::layer::object::repository::open(),
            }
            Ok(Some(0))
        }
        Command::INPUT_RECT => {
            let p = abi::InputRect::read(packet(pkt)?)?;
            refuse_unless(p.width >= 0.0 && p.height >= 0.0, Errno::EINVAL)?;
            camera_input_rect(p);
            Ok(Some(0))
        }
        Command::INPUT_POINTER => {
            let p = abi::InputPointer::read(packet(pkt)?)?;
            match p.phase {
                abi::PointerPhase::Down => {
                    camera_pointer_down(p.pointer_type, p.id, p.button, p.x, p.y)
                }
                abi::PointerPhase::Move => camera_pointer_move(p.pointer_type, p.id, p.x, p.y),
                abi::PointerPhase::Up => {
                    if let Some([x, y]) = camera_pointer_up(p.pointer_type, p.id) {
                        crate::pointer::tap(x, y);
                    }
                }
                abi::PointerPhase::Out => camera_pointer_out(),
                abi::PointerPhase::Cancel => {
                    camera_pointer_up(p.pointer_type, p.id);
                }
            }
            Ok(Some(0))
        }
        Command::INPUT_HOLD => {
            use pxl_spec::asset::cube::cube_of;
            use pxl_spec::asset::DecorId;
            let p = abi::InputHold::read(packet(pkt)?)?;
            let mut holding = holding();
            holding.active_tool = p.tool;
            holding.object_tier = p.tier;
            match p.tool {
                abi::ActiveTool::Object => {
                    holding.object_cube = or_refuse(cube_of(p.value), Errno::EINVAL)?;
                }
                abi::ActiveTool::Decor => {
                    holding.decor_id = or_refuse(
                        u16::try_from(p.value).ok().and_then(DecorId::of),
                        Errno::EINVAL,
                    )?;
                }
                abi::ActiveTool::Pointer | abi::ActiveTool::Eraser | abi::ActiveTool::Marker => {}
            }
            hold(holding);
            camera_rehover();
            Ok(Some(0))
        }
        Command::INPUT_MARKER_WORDS => {
            let text = abi::InputMarkerWords::read(packet(pkt)?)?.text;
            or_refuse(
                crate::domain::control::interaction::edit::draft_words(text).ok(),
                Errno::EMSGSIZE,
            )?;
            Ok(Some(0))
        }
        Command::INPUT_MARKER_PUT => {
            crate::domain::control::interaction::edit::put_marker();
            Ok(Some(0))
        }
        Command::INPUT_MARKER_CANCEL => {
            crate::domain::control::interaction::edit::cancel_marker();
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
        Command::INPUT_COMPASS_RESET => {
            // Following the heading would turn it straight back.
            if !crate::domain::control::navigation::follow_mode::following() {
                crate::domain::control::camera::controller::reset_azimuth();
            }
            applied(s)
        }
        Command::INPUT_GO_TO => {
            let p = abi::InputGoTo::read(packet(pkt)?)?;
            refuse_unless(on_earth(p.lon, p.lat), Errno::EINVAL)?;
            ui::go_to_degrees(
                p.lon,
                p.lat,
                crate::domain::control::camera::state::IntentPatch {
                    dist: p.dist,
                    azimuth: p.azimuth,
                    polar: p.polar,
                },
            );
            applied(s)
        }
        Command::INPUT_POSE => {
            let p = abi::InputPose::read(packet(pkt)?)?;
            // A unit quaternion by the protocol; one of no length turns nothing.
            refuse_unless(
                p.qx * p.qx + p.qy * p.qy + p.qz * p.qz + p.qw * p.qw > 0.0,
                Errno::EINVAL,
            )?;
            crate::domain::control::camera::controller::tracked_pose(
                [p.x, p.y, p.z],
                [p.qx, p.qy, p.qz, p.qw],
            );
            applied(s)
        }
        Command::INPUT_POSE_END => {
            crate::domain::control::camera::controller::tracked_end();
            applied(s)
        }
        Command::SENSOR_POSITION => {
            let p = abi::SensorPosition::read(packet(pkt)?)?;
            refuse_unless(
                on_earth(p.lon, p.lat) && p.accuracy.is_none_or(|a| a >= 0.0) && bearing(p.course),
                Errno::EINVAL,
            )?;
            // An accuracy the device does not give is no accuracy at all.
            geolocation::geolocation_fix(
                p.lon,
                p.lat,
                p.altitude,
                p.accuracy.unwrap_or(f64::INFINITY),
                p.course,
            );
            Ok(Some(0))
        }
        Command::SENSOR_FAILED => {
            let p = abi::SensorFailed::read(packet(pkt)?)?;
            match p.sensor {
                // A failure the platform named no cause for is Other's.
                abi::Sensor::Geolocation => geolocation::geolocation_error(
                    p.code.unwrap_or(abi::GeolocationErrorCode::Other),
                ),
                abi::Sensor::Orientation => device_orientation::orientation_lost(),
            }
            Ok(Some(0))
        }
        Command::SENSOR_ORIENTATION => {
            let p = abi::SensorOrientation::read(packet(pkt)?)?;
            // DeviceOrientationEvent's ranges, closed: a reading at an end is still one.
            let within = |v: Option<f64>, half: f64| v.is_none_or(|v| (-half..=half).contains(&v));
            refuse_unless(
                bearing(p.webkit_compass_heading)
                    && bearing(p.alpha)
                    && within(p.beta, 180.0)
                    && within(p.gamma, 90.0),
                Errno::EINVAL,
            )?;
            device_orientation::orientation_event(
                p.webkit_compass_heading,
                p.alpha,
                p.beta,
                p.gamma,
            );
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
                abi::Sensor::Orientation => device_orientation::orientation_permission(
                    p.answer,
                    world_platform().orientation,
                ),
            }
            Ok(Some(0))
        }
        Command::WORLD_VISIBILITY => {
            let visible = abi::WorldVisibility::read(packet(pkt)?)?.visible;
            geolocation::geolocation_visibility(visible, world_platform().geolocation);
            if visible {
                device_orientation::orientation_visible();
            } else {
                crate::domain::layer::object::repository::flush();
            }
            Ok(Some(0))
        }
        Command::INPUT_LOCATE => {
            ui::location_enable();
            applied(s)
        }
        Command::WORLD_VIEW => {
            if abi::WorldView::read(packet(pkt)?)?.shown {
                ui::sensors_open();
            } else {
                ui::sensors_close();
            }
            applied(s)
        }
        Command::GPU_READY => {
            pxl_render::gpu::api::device_answered();
            request_world_tick();
            Ok(Some(0))
        }
        Command::SURFACE_OPEN => {
            let handle =
                pxl_render::gpu::surface::surface_open(abi::SurfaceOpen::read(packet(pkt)?)?.role);
            if handle != 0 {
                request_world_tick();
            }
            Ok(Some(handle as i32))
        }
        Command::SURFACE_RESIZE => {
            let p = abi::SurfaceResize::read(packet(pkt)?)?;
            refuse_unless(p.device_pixel_ratio > 0.0, Errno::EINVAL)?;
            let role = pxl_render::gpu::surface::surface(p.surface).map_or(0, |s| s.role);
            let ratio = crate::domain::view::frame::resolution::surface_ratio(
                role,
                [p.width, p.height],
                p.device_pixel_ratio,
            );
            refuse_unless(
                pxl_render::gpu::surface::surface_resize(
                    p.surface,
                    p.width,
                    p.height,
                    p.device_pixel_ratio,
                    ratio,
                ),
                Errno::EBADF,
            )?;
            request_world_tick();
            Ok(Some(0))
        }
        Command::SURFACE_CLOSE => {
            let surface = abi::SurfaceClose::read(packet(pkt)?)?.surface;
            refuse_unless(
                pxl_render::gpu::surface::surface(surface).is_some(),
                Errno::EBADF,
            )?;
            super::close_surface(surface);
            Ok(Some(0))
        }
        Command::SETTING_SET => {
            let p = abi::SettingSet::read(packet(pkt)?)?;
            // A change before the slot is read would be read over.
            refuse_unless(settings_loaded(), Errno::EAGAIN)?;
            let changed = settings_store().set_setting(p.key, p.value);
            refuse_unless(changed.is_some(), Errno::EINVAL)?;
            if changed == Some(true) && p.key == SettingKey::profiler {
                profiler_follow(p.value == 1);
            }
            applied(s)
        }
        Command::CHAT_SEND => {
            crate::domain::scene::marker::append_chat_line(abi::ChatSend::read(packet(pkt)?)?.text);
            applied(s)
        }
        Command::PREVIEW_OPEN => {
            let p = abi::PreviewOpen::read(packet(pkt)?)?;
            refuse_unless(
                pxl_render::gpu::surface::surface(p.surface).is_some(),
                Errno::EBADF,
            )?;
            refuse_unless(
                crate::domain::scene::character::preview::preview_open(
                    p.surface, p.group, p.number,
                ),
                Errno::EINVAL,
            )?;
            applied(s)
        }
        Command::PREVIEW_CLOSE => {
            refuse_unless(
                crate::domain::scene::character::preview::preview_close(
                    abi::PreviewClose::read(packet(pkt)?)?.surface,
                ),
                Errno::EBADF,
            )?;
            applied(s)
        }
        // Records only while the profiler setting is on: a window opened with the
        // panel off would record with nothing to show or stop it.
        Command::PROFILER_START => {
            refuse_unless(settings_store().profiler, Errno::EPERM)?;
            refuse_unless(profiler_window_start(), Errno::ENOMEM)?;
            applied(s)
        }
        Command::PROFILER_STOP => {
            profiler_window_stop();
            applied(s)
        }
        // Memory is at most 1 GiB, so an address fits an i32.
        Command::PROFILER_READ => Ok(Some(profiler_read(
            abi::ProfilerRead::read(packet(pkt)?)?.window_ms,
        ) as i32)),
        Command::PROFILER_RINGS => Ok(Some(profiler_rings() as i32)),
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
            crate::sys::io::file::opened(p.fd, p.res, p.detail);
            Ok(Some(0))
        }
    }
}

/// The address of READ's words (profiler.json): the summary over
/// `window_ms`. Read in place rather than posted: `Door::post` asserts on a
/// full completion ring, and reading allocates nothing in the heap being
/// measured. Valid until the next turn.
fn profiler_read(window_ms: Option<f64>) -> u32 {
    use crate::profiler::spec::READ;
    // On the heap, not a static: a native build's statics are outside the shell-readable memory.
    static WORDS: xpute_guest::global::Global<Box<[u32; READ::WORDS as usize]>> =
        xpute_guest::global::Global::new();
    let summary = crate::profiler::summary::profiler_summarize(window_ms);
    let w = WORDS.get(|| Box::new([0; READ::WORDS as usize]));
    w[READ::summary_at as usize] = off_of_slice(&raw const *summary) as u32;
    w[READ::summary_words as usize] = summary.len() as u32;
    off_of(w.as_ptr()) as u32
}

/// The address of RINGS's words: the three rings as they lie, which a
/// reader of every record asks without the summary it would not read.
fn profiler_rings() -> u32 {
    use crate::profiler::record::{profiler_frames, profiler_tasks, profiler_worst};
    use crate::profiler::spec::RINGS;
    // On the heap, as READ's words are.
    static WORDS: xpute_guest::global::Global<Box<[u32; RINGS::WORDS as usize]>> =
        xpute_guest::global::Global::new();
    let (frames, tasks, worst) = (profiler_frames(), profiler_tasks(), profiler_worst());
    let w = WORDS.get(|| Box::new([0; RINGS::WORDS as usize]));
    w[RINGS::frames_at as usize] = off_of_slice(&raw const *frames) as u32;
    w[RINGS::frames_count as usize] = frames.len() as u32;
    w[RINGS::tasks_at as usize] = off_of_slice(&raw const *tasks) as u32;
    w[RINGS::tasks_count as usize] = tasks.len() as u32;
    w[RINGS::worst_at as usize] = off_of_slice(&raw const *worst) as u32;
    w[RINGS::worst_count as usize] = worst.len() as u32;
    off_of(w.as_ptr()) as u32
}

#[cfg(test)]
#[path = "apply.test.rs"]
mod test;
