// pxl-kernel/door/apply.rs

//! The commands, applied: what each one the shell submits does to the kernel,
//! and the ledger read in place for the two that ask for it.

use super::{post_shell, State};
use crate::abi::cmd::{self as abi, Command};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::base::{off_of, off_of_slice};

/// A command's packet, refused when the command needs one and the message
/// has none.
fn packet<'a>(pkt: &Option<&'a [u8]>) -> Result<&'a [u8], MarshalError> {
    pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))
}

/// A command from the page's UI, applied: a turn asked for, and what it asks
/// of the host raised before the call returns — the shell rings for these at
/// once, so a sensor's close sent after the strobe has stopped is still heard.
fn applied(s: &mut State) -> Result<Option<i32>, MarshalError> {
    crate::world::change::request_world_tick();
    post_shell(s);
    Ok(Some(0))
}

/// Applies one submission: its reply's value, or none for a command nobody
/// numbers or a signal submitted as a command, which replies ENOSYS; a packet
/// that does not read as the command's replies with its errno.
pub(super) fn apply(s: &mut State, number: u32, pkt: &Option<&[u8]>) -> Result<Option<i32>, MarshalError> {
    use crate::control::camera::input::{camera_input_rect, camera_pointer_down, camera_pointer_move, camera_pointer_out, camera_pointer_up, camera_wheel, Holding, Rect};
    use crate::control::sensor::{device_orientation, geolocation};
    use crate::sched::ledger::log::{ledger_set_logging, ledger_window_start, ledger_window_stop};
    use crate::sched::turn::world_platform;
    use crate::shell::setting::store::{settings_load, settings_store};
    use crate::shell::shell as ui;
    use crate::world::change::request_world_tick;

    let Some(cmd) = Command::of(number) else { return Ok(None) };
    match cmd {
        Command::NOP => {
            let value = match pkt {
                Some(pkt) => abi::Nop::read(pkt)?.value,
                None => 0,
            };
            Ok(Some(value as i32))
        }
        Command::WORLD_UPDATE => {
            let p = abi::WorldUpdate::read(packet(pkt)?)?;
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
        Command::WORLD_CREATE => {
            let p = abi::WorldCreate::read(packet(pkt)?)?;
            crate::host::set_development(p.development);
            crate::host::set_handheld(p.handheld);
            crate::sched::turn::set_world_platform(crate::sched::turn::Platform {
                geolocation: p.geolocation,
                orientation: device_orientation::OrientationPlatform {
                    supported: p.orientation,
                    needs_permission: p.orientation_permission,
                },
            });
            settings_load(p.settings.iter().copied());
            if p.objects != 0 {
                crate::layer::object::store::load(p.objects, p.objects_bytes);
            }
            Ok(Some(0))
        }
        Command::INPUT_UPDATE => {
            let p = abi::InputUpdate::read(packet(pkt)?)?;
            camera_input_rect(Rect {
                left: p.left,
                top: p.top,
                width: p.width,
                height: p.height,
            });
            Ok(Some(0))
        }
        Command::INPUT_POINTER => {
            use pxl_spec::cell::object::placeable_cube;
            use pxl_spec::cell::DecorId;
            let p = abi::InputPointer::read(packet(pkt)?)?;
            let mut holding = Holding {
                active_tool: p.tool,
                object_size: crate::layer::object::size::object_size_of(p.tier),
                ..Holding::default()
            };
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
            match p.phase {
                abi::PointerPhase::Down => camera_pointer_down(p.kind, p.id, p.button, p.x, p.y, holding),
                abi::PointerPhase::Move => camera_pointer_move(p.kind, p.id, p.x, p.y, holding),
                abi::PointerPhase::Up => camera_pointer_up(p.kind, p.id, holding),
                abi::PointerPhase::Out => camera_pointer_out(),
            }
            Ok(Some(0))
        }
        Command::INPUT_WHEEL => {
            camera_wheel(abi::InputWheel::read(packet(pkt)?)?.delta_y);
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
        Command::INPUT_RESET_DIST => {
            crate::control::camera::store::camera_store().reset_dist();
            applied(s)
        }
        Command::INPUT_COMPASS_RESET => {
            crate::shell::ui::compass::compass_reset();
            applied(s)
        }
        Command::INPUT_GO_TO => {
            let p = abi::InputGoTo::read(packet(pkt)?)?;
            ui::go_to_degrees(p.lon, p.lat);
            applied(s)
        }
        Command::SENSOR_UPDATE => {
            let p = abi::SensorUpdate::read(packet(pkt)?)?;
            geolocation::geolocation_fix(p.lon, p.lat, p.altitude, p.accuracy, p.heading);
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
        Command::SENSOR_PERMISSION => {
            let p = abi::SensorPermission::read(packet(pkt)?)?;
            match p.sensor {
                abi::Sensor::Geolocation => {
                    if p.answer == abi::PermissionAnswer::Granted {
                        geolocation::geolocation_permission_granted(world_platform().geolocation);
                    }
                }
                abi::Sensor::Orientation => device_orientation::orientation_permission(p.answer),
            }
            Ok(Some(0))
        }
        Command::SENSOR_VISIBILITY => {
            let visible = abi::SensorVisibility::read(packet(pkt)?)?.visible;
            geolocation::geolocation_visibility(visible, world_platform().geolocation);
            if visible {
                device_orientation::orientation_visible();
            } else {
                crate::layer::object::store::flush_save();
            }
            Ok(Some(0))
        }
        Command::SENSOR_ENABLE => {
            ui::sensors_enable();
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
        Command::FETCH_UPDATE => {
            crate::io::fetch::fetch_queued(abi::FetchUpdate::read(packet(pkt)?)?);
            Ok(Some(0))
        }
        Command::LOAD_UPDATE => {
            let p = abi::LoadUpdate::read(packet(pkt)?)?;
            // A model's bytes are far past a slot: the host holds them, and
            // the models' task reads them on a turn's quota.
            crate::io::load::gltf_arrived(p.key, p.body, p.bytes);
            Ok(Some(0))
        }
        Command::LOAD_IMAGE => {
            let p = abi::LoadImage::read(packet(pkt)?)?;
            crate::io::load::image_arrived(p.key, p.handle);
            Ok(Some(0))
        }
        Command::LOAD_DELETE => {
            crate::io::load::load_rejected(abi::LoadDelete::read(packet(pkt)?)?.key);
            Ok(Some(0))
        }
        Command::GPU_CREATE => {
            let p = abi::GpuCreate::read(packet(pkt)?)?;
            pxl_render::gpu::renderer::gpu_created(p.min_uniform_offset_alignment, p.timestamps);
            request_world_tick();
            Ok(Some(0))
        }
        Command::GPU_DELETE => {
            pxl_render::gpu::renderer::gpu_lost();
            Ok(Some(0))
        }
        Command::SURFACE_CREATE => {
            let handle = pxl_render::gpu::surface::surface_create(abi::SurfaceCreate::read(packet(pkt)?)?.role);
            if handle != 0 {
                request_world_tick();
            }
            Ok(Some(handle as i32))
        }
        Command::SURFACE_UPDATE => {
            let p = abi::SurfaceUpdate::read(packet(pkt)?)?;
            if pxl_render::gpu::surface::surface_update(p.surface, p.width, p.height, p.device_pixel_ratio) {
                request_world_tick();
            }
            Ok(Some(0))
        }
        Command::SURFACE_DELETE => {
            if let Some(closed) = pxl_render::gpu::surface::surface_delete(abi::SurfaceDelete::read(packet(pkt)?)?.surface) {
                pxl_render::gpu::renderer::surface_closed(closed);
            }
            Ok(Some(0))
        }
        Command::SETTING_UPDATE => {
            let p = abi::SettingUpdate::read(packet(pkt)?)?;
            settings_store().set_setting(p.key, p.value);
            applied(s)
        }
        Command::SETTING_TOGGLE => {
            settings_store().toggle_setting(abi::SettingToggle::read(packet(pkt)?)?.key);
            applied(s)
        }
        Command::CHAT_CREATE => {
            crate::layer::character::chat_bubble::append_chat_line(abi::ChatCreate::read(packet(pkt)?)?.text);
            applied(s)
        }
        Command::PREVIEW_CREATE => {
            let p = abi::PreviewCreate::read(packet(pkt)?)?;
            let number = u8::try_from(p.number).map_err(|_| MarshalError::new(Errno::EINVAL))?;
            if !crate::layer::character::preview::preview_open(p.surface, p.group, number) {
                return Err(MarshalError::new(Errno::EINVAL));
            }
            applied(s)
        }
        Command::PREVIEW_DELETE => {
            crate::layer::character::preview::preview_close(abi::PreviewDelete::read(packet(pkt)?)?.surface);
            applied(s)
        }
        Command::LEDGER_START => {
            ledger_window_start();
            applied(s)
        }
        Command::LEDGER_STOP => {
            ledger_window_stop();
            applied(s)
        }
        Command::LEDGER_LOG => {
            ledger_set_logging(abi::LedgerLog::read(packet(pkt)?)?.on);
            applied(s)
        }
        // An address in a memory of at most a gibibyte, which an i32 holds.
        Command::LEDGER_READ => Ok(Some(ledger_read(abi::LedgerRead::read(packet(pkt)?)?.window_ms) as i32)),
        Command::LEDGER_TAKE => Ok(Some(ledger_take(s) as i32)),
        Command::WORLD_SNAPSHOT | Command::SENSOR_CREATE | Command::SENSOR_DELETE | Command::FETCH_CREATE | Command::FETCH_DELETE | Command::LOAD_CREATE => Ok(None),
    }
}

/// The ledger, where it lies: the address of READ's words (ledger.json) —
/// the summary block over the last `window_ms`, an infinite window being the
/// whole recording, and the frame, task and worst-frame rings, each an address
/// and a record count.
///
/// Read in place rather than sent: this is instrumentation, and
/// instrumentation does not go out on the control ring, whose failure policy
/// is that `Outbox::post` asserts when the completion queue and its spill are
/// both full. Nothing is built for the read either — the rings are the ledger's
/// own lanes and the summary a block written where it stays — so a reader
/// allocates nothing from the heap the ledger measures. The addresses are good
/// until the next turn.
fn ledger_read(window_ms: f64) -> u32 {
    use crate::sched::ledger::record::{ledger_frames, ledger_tasks, ledger_worst};
    use crate::sched::ledger::spec::READ;
    // On the heap rather than in a static: the shell reads it in the memory,
    // and a native build's statics are not in it.
    static WORDS: crate::global::Global<Box<[u32; READ::WORDS as usize]>> = crate::global::Global::new();
    let summary = crate::sched::ledger::summary::ledger_summarize(window_ms);
    let (frames, tasks, worst) = (ledger_frames(), ledger_tasks(), ledger_worst());
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

/// What the live stream kept since the shell last cleared it: the address of
/// LOG's words (ledger.json), the task rows and the frames, each an address
/// and a count. Zero counts when nothing is waiting, which a shell polling is
/// asking about most of the time. The rows are let go on the next turn.
fn ledger_take(s: &mut State) -> u32 {
    use crate::sched::ledger::spec::LOG;
    // On the heap, as READ's words are.
    static WORDS: crate::global::Global<Box<[u32; LOG::WORDS as usize]>> = crate::global::Global::new();
    let (tasks, frames) = crate::sched::ledger::log::ledger_log_rows();
    let w = WORDS.get(|| Box::new([0; LOG::WORDS as usize]));
    w[LOG::tasks_at as usize] = off_of_slice(&raw const *tasks) as u32;
    w[LOG::tasks_count as usize] = tasks.len() as u32;
    w[LOG::frames_at as usize] = off_of_slice(&raw const *frames) as u32;
    w[LOG::frames_count as usize] = frames.len() as u32;
    s.ledger_taken = true;
    off_of(w.as_ptr()) as u32
}
