// pxl-kernel/door/apply.rs

//! The shell's commands, applied.

use super::{post_shell, State};
use crate::abi::cmd::{self as abi, Command};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_runtime::abi::cmd as xpute_cmd;
use xpute_runtime::mem::base::{off_of, off_of_slice};

fn packet<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))
}

/// A UI command: asks for a turn and raises what it asks of the host before
/// returning, so a sensor close sent after the strobe stopped is still heard.
fn applied(s: &mut State) -> Result<Option<i32>, MarshalError> {
    crate::world::change::request_world_tick();
    post_shell(s);
    Ok(Some(0))
}

/// Applies one submission. None (replied as ENOSYS) for an unknown command
/// or a signal submitted as one.
pub(super) fn apply(s: &mut State, number: u32, pkt: &Option<&[u8]>) -> Result<Option<i32>, MarshalError> {
    use crate::control::camera::input::{camera_input_rect, camera_keys, camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up, camera_rehover, camera_wheel, Rect};
    use crate::control::interaction::{hold, holding};
    use crate::control::sensor::{device_orientation, geolocation};
    use crate::sched::profiler::log::{profiler_set_logging, profiler_window_start, profiler_window_stop};
    use crate::sched::turn::world_platform;
    use crate::shell::setting::store::{settings_load, settings_store};
    use crate::shell::shell as ui;
    use crate::world::change::request_world_tick;

    if let Some(cmd) = xpute_cmd::Command::of(number) {
        return apply_xpute(cmd, pkt);
    }
    let Some(cmd) = Command::of(number) else { return Ok(None) };
    match cmd {
        Command::WORLD_FRAME => {
            let p = abi::WorldFrame::read(packet(pkt)?)?;
            crate::sched::turn::world_frame_asked(crate::sched::turn::HostFrame {
                width: p.width,
                height: p.height,
                dpr: p.dpr,
                delta: p.delta_s,
                frame_ms: p.frame_ms,
                interactive: p.interactive,
                credits: p.credits,
            });
            crate::host::set_local_time(crate::host::LocalTime {
                unix_ms: p.unix_ms,
                hours: p.hours,
                minutes: p.minutes,
                seconds: p.seconds,
                day_start_ms: p.day_start_ms,
            });
            Ok(Some(0))
        }
        Command::WORLD_BOOT => {
            let p = abi::WorldBoot::read(packet(pkt)?)?;
            super::host_attached(s);
            crate::host::set_development(p.development);
            crate::host::set_handheld(p.handheld);
            crate::sched::turn::set_world_platform(crate::sched::turn::Platform {
                geolocation: p.geolocation,
                orientation: device_orientation::OrientationPlatform {
                    supported: p.orientation,
                    needs_permission: p.orientation_permission,
                    silence_is_loss: p.orientation_silence_is_loss,
                },
            });
            settings_load(p.settings);
            crate::layer::object::store::load();
            Ok(Some(0))
        }
        Command::WORLD_SOURCE => {
            crate::world::paging::floor::declare(abi::WorldSource::read(packet(pkt)?)?.top_level);
            Ok(Some(0))
        }
        Command::INPUT_RECT => {
            let p = abi::InputRect::read(packet(pkt)?)?;
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
                abi::PointerPhase::Up => camera_pointer_up(p.kind, p.id),
                abi::PointerPhase::Out => camera_pointer_out(),
            }
            Ok(Some(0))
        }
        // A cube or decor that is not in the catalog leaves the one held.
        Command::INPUT_HOLD => {
            use pxl_spec::cell::object::placeable_cube;
            use pxl_spec::cell::DecorId;
            let p = abi::InputHold::read(packet(pkt)?)?;
            let mut holding = holding();
            holding.active_tool = p.tool;
            holding.object_tier = p.tier;
            match p.tool {
                abi::ActiveTool::Object => {
                    if let Some(cube) = placeable_cube(p.channel, p.value) {
                        holding.object_cube = cube;
                    }
                }
                abi::ActiveTool::Decor => {
                    if let Some(decor) = u8::try_from(p.value).ok().and_then(DecorId::of) {
                        holding.decor_id = decor;
                    }
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
            if !crate::control::camera::store::camera_store().is_follow_mode {
                crate::control::camera::controller::reset_azimuth();
            }
            applied(s)
        }
        Command::INPUT_GO_TO => {
            let p = abi::InputGoTo::read(packet(pkt)?)?;
            ui::go_to_degrees(p.lon, p.lat);
            applied(s)
        }
        Command::INPUT_POSE => {
            let p = abi::InputPose::read(packet(pkt)?)?;
            crate::control::camera::controller::tracked_pose([p.x, p.y, p.z], [p.qx, p.qy, p.qz, p.qw]);
            applied(s)
        }
        Command::INPUT_POSE_END => {
            crate::control::camera::controller::tracked_end();
            applied(s)
        }
        Command::SENSOR_POSITION => {
            let p = abi::SensorPosition::read(packet(pkt)?)?;
            geolocation::geolocation_fix(p.lon, p.lat, p.altitude, p.accuracy, p.course);
            Ok(Some(0))
        }
        Command::SENSOR_ERROR => {
            geolocation::geolocation_error(abi::SensorError::read(packet(pkt)?)?.code);
            Ok(Some(0))
        }
        Command::SENSOR_ORIENTATION => {
            let p = abi::SensorOrientation::read(packet(pkt)?)?;
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
                crate::layer::object::store::flush_save();
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
            crate::io::load::image_decoded(p.image, p.res);
            Ok(Some(0))
        }
        Command::GPU_READY => {
            let p = abi::GpuReady::read(packet(pkt)?)?;
            pxl_render::gpu::renderer::gpu_created(p.min_uniform_offset_alignment, p.timestamps);
            request_world_tick();
            Ok(Some(0))
        }
        Command::GPU_LOST => {
            pxl_render::gpu::renderer::gpu_lost();
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
            if pxl_render::gpu::surface::surface_resize(p.surface, p.width, p.height, p.device_pixel_ratio) {
                request_world_tick();
            }
            Ok(Some(0))
        }
        Command::SURFACE_CLOSE => {
            if let Some(closed) = pxl_render::gpu::surface::surface_close(abi::SurfaceClose::read(packet(pkt)?)?.surface) {
                pxl_render::gpu::renderer::surface_closed(closed);
            }
            Ok(Some(0))
        }
        Command::SETTING_SET => {
            let p = abi::SettingSet::read(packet(pkt)?)?;
            settings_store().set_setting(p.key, p.value);
            applied(s)
        }
        Command::CHAT_SEND => {
            crate::layer::character::chat_bubble::append_chat_line(abi::ChatSend::read(packet(pkt)?)?.text);
            applied(s)
        }
        Command::PREVIEW_OPEN => {
            let p = abi::PreviewOpen::read(packet(pkt)?)?;
            let number = u8::try_from(p.number).map_err(|_| MarshalError::new(Errno::EINVAL))?;
            if !crate::layer::character::preview::preview_open(p.surface, p.group, number) {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            applied(s)
        }
        Command::PREVIEW_CLOSE => {
            crate::layer::character::preview::preview_close(abi::PreviewClose::read(packet(pkt)?)?.surface);
            applied(s)
        }
        // Records only while the profiler setting is on: a window opened with the
        // panel off would record with nothing to show or stop it.
        Command::PROFILER_START => {
            if settings_store().debug_overlay {
                profiler_window_start();
            }
            applied(s)
        }
        Command::PROFILER_STOP => {
            profiler_window_stop();
            applied(s)
        }
        Command::PROFILER_LOG => {
            let on = abi::ProfilerLog::read(packet(pkt)?)?.on;
            profiler_set_logging(on && settings_store().debug_overlay);
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
fn profiler_read(window_ms: f64) -> u32 {
    use crate::sched::profiler::record::{profiler_frames, profiler_tasks, profiler_worst};
    use crate::sched::profiler::spec::READ;
    // On the heap, not a static: a native build's statics are outside the shell-readable memory.
    static WORDS: xpute_runtime::global::Global<Box<[u32; READ::WORDS as usize]>> = xpute_runtime::global::Global::new();
    let summary = crate::sched::profiler::summary::profiler_summarize(window_ms);
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
    use crate::sched::profiler::spec::LOG;
    // On the heap, as READ's words are.
    static WORDS: xpute_runtime::global::Global<Box<[u32; LOG::WORDS as usize]>> = xpute_runtime::global::Global::new();
    let (tasks, frames) = crate::sched::profiler::log::profiler_log_rows();
    let w = WORDS.get(|| Box::new([0; LOG::WORDS as usize]));
    w[LOG::tasks_at as usize] = off_of_slice(&raw const *tasks) as u32;
    w[LOG::tasks_count as usize] = tasks.len() as u32;
    w[LOG::frames_at as usize] = off_of_slice(&raw const *frames) as u32;
    w[LOG::frames_count as usize] = frames.len() as u32;
    s.profiler_taken = true;
    off_of(w.as_ptr()) as u32
}
